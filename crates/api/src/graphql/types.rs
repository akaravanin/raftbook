use async_graphql::*;
use event_log::Event;

// ── GraphQL scalars ───────────────────────────────────────────────────────────
//
// GraphQL Int is 32-bit signed.  We use ID (string) for order/user IDs and
// i64 for quantities and prices to avoid precision loss in JSON transport.

/// Mirrors `event_log::EventRecord` with fully typed variants.
#[derive(SimpleObject, Clone)]
pub struct GqlEventRecord {
    pub seq: i64,
    pub event: GqlEvent,
}

/// Union of all domain event payloads.
#[derive(Union, Clone)]
pub enum GqlEvent {
    OrderAccepted(GqlOrderAccepted),
    TradeExecuted(GqlTradeExecuted),
    OrderCanceled(GqlOrderCanceled),
}

#[derive(SimpleObject, Clone)]
pub struct GqlOrderAccepted {
    pub order_id: ID,
}

#[derive(SimpleObject, Clone)]
pub struct GqlTradeExecuted {
    pub maker_order_id: ID,
    pub taker_order_id: ID,
    pub price: i64,
    pub quantity: i64,
}

#[derive(SimpleObject, Clone)]
pub struct GqlOrderCanceled {
    pub order_id: ID,
}

/// Result of a PlaceOrder mutation.
#[derive(SimpleObject)]
pub struct PlaceOrderResult {
    pub accepted_seq: i64,
    pub fills: Vec<GqlFill>,
    pub remaining_qty: i64,
    /// `false` when this was an idempotent retry of an already-processed command.
    pub inserted: bool,
}

/// A single fill produced by a PlaceOrder.
#[derive(SimpleObject, Clone)]
pub struct GqlFill {
    pub maker_order_id: ID,
    pub taker_order_id: ID,
    pub price: i64,
    pub quantity: i64,
}

/// Result of a CancelOrder mutation.
#[derive(SimpleObject)]
pub struct CancelOrderResult {
    pub event_seq: i64,
    pub inserted: bool,
}

// ── Conversions ───────────────────────────────────────────────────────────────

impl From<event_log::EventRecord> for GqlEventRecord {
    fn from(r: event_log::EventRecord) -> Self {
        let event = match r.event {
            Event::OrderAccepted { order_id } => {
                GqlEvent::OrderAccepted(GqlOrderAccepted {
                    order_id: ID::from(order_id.to_string()),
                })
            }
            Event::TradeExecuted { trade } => GqlEvent::TradeExecuted(GqlTradeExecuted {
                maker_order_id: ID::from(trade.maker_order_id.to_string()),
                taker_order_id: ID::from(trade.taker_order_id.to_string()),
                price: trade.price as i64,
                quantity: trade.quantity as i64,
            }),
            Event::OrderCanceled { order_id } => {
                GqlEvent::OrderCanceled(GqlOrderCanceled {
                    order_id: ID::from(order_id.to_string()),
                })
            }
        };
        GqlEventRecord {
            seq: r.seq as i64,
            event,
        }
    }
}
