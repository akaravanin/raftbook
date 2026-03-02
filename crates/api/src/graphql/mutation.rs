use async_graphql::{Context, Enum, Object, Result, ID};
use command_handler::{Command, CommandResult};
use event_log::Event;
use matching_engine::{LimitOrder, Side};

use crate::{
    graphql::types::{CancelOrderResult, GqlFill, PlaceOrderResult},
    AppState,
};

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum GqlSide {
    Bid,
    Ask,
}

pub struct Mutation;

#[Object]
impl Mutation {
    /// Submit a limit order.  `commandId` must be a caller-generated UUID;
    /// retrying with the same `commandId` is safe and returns the original result.
    async fn place_order(
        &self,
        ctx: &Context<'_>,
        command_id: String,
        order_id: ID,
        user_id: ID,
        side: GqlSide,
        price: i64,
        quantity: i64,
    ) -> Result<PlaceOrderResult> {
        if command_id.is_empty() {
            return Err("command_id is required".into());
        }
        if price <= 0 {
            return Err("price must be > 0".into());
        }
        if quantity <= 0 {
            return Err("quantity must be > 0".into());
        }

        let matching_side = match side {
            GqlSide::Bid => Side::Bid,
            GqlSide::Ask => Side::Ask,
        };

        let order = LimitOrder {
            order_id: order_id.parse::<u64>()?,
            user_id: user_id.parse::<u64>()?,
            side: matching_side,
            price: price as u64,
            quantity: quantity as u64,
            ts_nanos: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
        };

        let state = ctx.data::<AppState>()?;
        let result = state
            .handler
            .lock()
            .await
            .handle(Command::PlaceOrder { command_id, order })
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        let CommandResult::OrderPlaced(placed) = result else {
            return Err("unexpected result variant".into());
        };

        // Broadcast to streaming subscribers.
        let _ = state.event_tx.send(placed.order_accepted.clone());
        for trade_rec in &placed.trades {
            let _ = state.event_tx.send(trade_rec.clone());
        }

        let fills = placed
            .trades
            .iter()
            .filter_map(|rec| {
                if let Event::TradeExecuted { trade } = &rec.event {
                    Some(GqlFill {
                        maker_order_id: ID::from(trade.maker_order_id.to_string()),
                        taker_order_id: ID::from(trade.taker_order_id.to_string()),
                        price: trade.price as i64,
                        quantity: trade.quantity as i64,
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(PlaceOrderResult {
            accepted_seq: placed.order_accepted.seq as i64,
            fills,
            remaining_qty: placed.remaining_quantity as i64,
            inserted: placed.inserted,
        })
    }

    /// Cancel a resting limit order.  Idempotent via `commandId`.
    async fn cancel_order(
        &self,
        ctx: &Context<'_>,
        command_id: String,
        order_id: ID,
    ) -> Result<CancelOrderResult> {
        if command_id.is_empty() {
            return Err("command_id is required".into());
        }

        let state = ctx.data::<AppState>()?;
        let result = state
            .handler
            .lock()
            .await
            .handle(Command::CancelOrder {
                command_id,
                order_id: order_id.parse::<u64>()?,
            })
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        let CommandResult::OrderCanceled(canceled) = result else {
            return Err("unexpected result variant".into());
        };

        let _ = state.event_tx.send(canceled.event.clone());

        Ok(CancelOrderResult {
            event_seq: canceled.event.seq as i64,
            inserted: canceled.inserted,
        })
    }
}
