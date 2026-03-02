use std::pin::Pin;

use async_stream::try_stream;
use command_handler::{Command, CommandResult};
use event_log::{Event, PostgresEventLog};
use matching_engine::{LimitOrder, Side};
use tokio::sync::broadcast;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};
use tracing::warn;

use crate::AppState;

// Pull in the generated protobuf/gRPC types.
pub mod proto {
    tonic::include_proto!("exchange.v1");
}

use proto::{
    exchange_server::{Exchange, ExchangeServer},
    CancelOrderRequest, CancelOrderResponse, EventRecord as ProtoEventRecord, Fill,
    PlaceOrderRequest, PlaceOrderResponse, StreamEventsRequest,
};

// ── Proto ↔ domain conversions ────────────────────────────────────────────────

fn to_proto_record(r: event_log::EventRecord) -> ProtoEventRecord {
    use proto::{event_record::Payload, OrderAccepted, OrderCanceled, TradeExecuted};

    let payload = match r.event {
        Event::OrderAccepted { order_id } => {
            Payload::OrderAccepted(OrderAccepted { order_id })
        }
        Event::TradeExecuted { trade } => Payload::TradeExecuted(TradeExecuted {
            maker_order_id: trade.maker_order_id,
            taker_order_id: trade.taker_order_id,
            price: trade.price,
            quantity: trade.quantity,
        }),
        Event::OrderCanceled { order_id } => {
            Payload::OrderCanceled(OrderCanceled { order_id })
        }
    };

    ProtoEventRecord {
        seq: r.seq,
        payload: Some(payload),
    }
}

fn parse_side(raw: i32) -> Result<Side, Status> {
    // prost strips the enum-name prefix: SIDE_BID → Side::Bid, SIDE_ASK → Side::Ask.
    match proto::Side::try_from(raw) {
        Ok(proto::Side::Bid) => Ok(Side::Bid),
        Ok(proto::Side::Ask) => Ok(Side::Ask),
        _ => Err(Status::invalid_argument("side must be SIDE_BID or SIDE_ASK")),
    }
}

// ── Service struct ─────────────────────────────────────────────────────────────

struct ExchangeService {
    state: AppState,
}

pub fn make_exchange_server(state: AppState) -> ExchangeServer<impl Exchange> {
    ExchangeServer::new(ExchangeService { state })
}

// ── tonic impl ────────────────────────────────────────────────────────────────

type StreamEventsStream =
    Pin<Box<dyn Stream<Item = Result<ProtoEventRecord, Status>> + Send + 'static>>;

#[tonic::async_trait]
impl Exchange for ExchangeService {
    // ── PlaceOrder ──────────────────────────────────────────────────────────

    async fn place_order(
        &self,
        req: Request<PlaceOrderRequest>,
    ) -> Result<Response<PlaceOrderResponse>, Status> {
        let r = req.into_inner();

        if r.command_id.is_empty() {
            return Err(Status::invalid_argument("command_id is required"));
        }
        if r.price == 0 {
            return Err(Status::invalid_argument("price must be > 0"));
        }
        if r.quantity == 0 {
            return Err(Status::invalid_argument("quantity must be > 0"));
        }

        let side = parse_side(r.side)?;
        let order = LimitOrder {
            order_id: r.order_id,
            user_id: r.user_id,
            side,
            price: r.price,
            quantity: r.quantity,
            ts_nanos: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
        };

        let result = self
            .state
            .handler
            .lock()
            .await
            .handle(Command::PlaceOrder {
                command_id: r.command_id,
                order,
            })
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let CommandResult::OrderPlaced(placed) = result else {
            return Err(Status::internal("unexpected result variant"));
        };

        // Broadcast accepted + trade events to streaming subscribers.
        let _ = self.state.event_tx.send(placed.order_accepted.clone());
        for trade_rec in &placed.trades {
            let _ = self.state.event_tx.send(trade_rec.clone());
        }

        let fills = placed
            .trades
            .iter()
            .filter_map(|rec| {
                if let Event::TradeExecuted { trade } = &rec.event {
                    Some(Fill {
                        maker_order_id: trade.maker_order_id,
                        taker_order_id: trade.taker_order_id,
                        price: trade.price,
                        quantity: trade.quantity,
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(Response::new(PlaceOrderResponse {
            accepted_seq: placed.order_accepted.seq,
            fills,
            remaining_qty: placed.remaining_quantity,
            inserted: placed.inserted,
        }))
    }

    // ── CancelOrder ─────────────────────────────────────────────────────────

    async fn cancel_order(
        &self,
        req: Request<CancelOrderRequest>,
    ) -> Result<Response<CancelOrderResponse>, Status> {
        let r = req.into_inner();

        if r.command_id.is_empty() {
            return Err(Status::invalid_argument("command_id is required"));
        }

        let result = self
            .state
            .handler
            .lock()
            .await
            .handle(Command::CancelOrder {
                command_id: r.command_id,
                order_id: r.order_id,
            })
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let CommandResult::OrderCanceled(canceled) = result else {
            return Err(Status::internal("unexpected result variant"));
        };

        let _ = self.state.event_tx.send(canceled.event.clone());

        Ok(Response::new(CancelOrderResponse {
            event_seq: canceled.event.seq,
            inserted: canceled.inserted,
        }))
    }

    // ── StreamEvents ────────────────────────────────────────────────────────
    //
    // Pattern: subscribe to the broadcast channel FIRST, then replay history.
    // This ensures no events are missed in the window between replay and live.

    type StreamEventsStream = StreamEventsStream;

    async fn stream_events(
        &self,
        req: Request<StreamEventsRequest>,
    ) -> Result<Response<Self::StreamEventsStream>, Status> {
        let from_seq = req.into_inner().from_seq;
        let pool = self.state.pool.clone();

        // Subscribe before replay to avoid the replay→live gap.
        let mut rx: broadcast::Receiver<event_log::EventRecord> =
            self.state.event_tx.subscribe();

        let stream = try_stream! {
            // 1. Replay persisted events.
            let log = PostgresEventLog::from_pool(pool);
            let history = log
                .read_from(from_seq)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;

            let mut cursor = from_seq;
            for record in history {
                cursor = record.seq + 1;
                yield to_proto_record(record);
            }

            // 2. Tail live events from the broadcast channel.
            loop {
                match rx.recv().await {
                    Ok(record) => {
                        if record.seq >= cursor {
                            cursor = record.seq + 1;
                            yield to_proto_record(record);
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!(skipped = n, "StreamEvents receiver lagged; some events may be missing");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        };

        Ok(Response::new(Box::pin(stream)))
    }
}
