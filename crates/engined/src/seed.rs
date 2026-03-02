//! Realistic market seed data.
//!
//! All commands use fixed, stable `command_id` strings so the seed is
//! **fully idempotent** — running it on every startup is safe.  On the first
//! boot each command is `inserted=true`; on subsequent boots the command_log
//! returns the original event record without touching the order book.
//!
//! # Scenario
//!
//! Two market makers post a two-sided quote around mid 101.5:
//!
//!   Ask 102  qty 10  (order #1, MM user #10)
//!   Ask 104  qty 10  (order #2, MM user #10)
//!   Bid  99  qty 10  (order #3, MM user #20)
//!   Bid  97  qty 10  (order #4, MM user #20)
//!
//! A taker hits the ask side:
//!   Bid 105  qty 15  (order #5, Taker user #30)
//!   → fills ask@102 fully (10 lots), ask@104 partially (5 lots)
//!   → ask@104 now has 5 lots remaining
//!
//! A second taker lifts the bid:
//!   Ask  98  qty  5  (order #6, Taker user #40)
//!   → fills bid@99 partially (5 lots)
//!   → bid@99 now has 5 lots remaining
//!
//! A market-maker cancel:
//!   Cancel order #4 (bid@97 qty 10)
//!
//! Final resting book:  Ask 104 qty 5  |  Bid 99 qty 5  |  spread = 5 ticks
//! Events written:       10  (6× OrderAccepted, 3× TradeExecuted, 1× OrderCanceled)

use command_handler::{Command, CommandHandler, CommandResult};
use event_log::PostgresEventLog;
use matching_engine::{LimitOrder, Side};
use tracing::info;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub async fn seed_market(handler: &mut CommandHandler<PostgresEventLog>) -> Result<()> {
    info!("seeding market data (idempotent — safe to re-run)");

    // ── Market maker quotes ───────────────────────────────────────────────────
    place(handler, "seed-mm-ask-102", LimitOrder {
        order_id: 1, user_id: 10, side: Side::Ask,
        price: 102, quantity: 10, ts_nanos: 1_000,
    }).await?;

    place(handler, "seed-mm-ask-104", LimitOrder {
        order_id: 2, user_id: 10, side: Side::Ask,
        price: 104, quantity: 10, ts_nanos: 2_000,
    }).await?;

    place(handler, "seed-mm-bid-99", LimitOrder {
        order_id: 3, user_id: 20, side: Side::Bid,
        price: 99, quantity: 10, ts_nanos: 3_000,
    }).await?;

    place(handler, "seed-mm-bid-97", LimitOrder {
        order_id: 4, user_id: 20, side: Side::Bid,
        price: 97, quantity: 10, ts_nanos: 4_000,
    }).await?;

    // ── Taker hits the ask side ───────────────────────────────────────────────
    // Bid@105 qty 15 crosses ask@102(10) and partial ask@104(5).
    place(handler, "seed-taker-bid-105", LimitOrder {
        order_id: 5, user_id: 30, side: Side::Bid,
        price: 105, quantity: 15, ts_nanos: 5_000,
    }).await?;

    // ── Taker lifts the bid side ──────────────────────────────────────────────
    // Ask@98 qty 5 crosses bid@99(5).
    place(handler, "seed-taker-ask-98", LimitOrder {
        order_id: 6, user_id: 40, side: Side::Ask,
        price: 98, quantity: 5, ts_nanos: 6_000,
    }).await?;

    // ── Market maker cancel ───────────────────────────────────────────────────
    cancel(handler, "seed-cancel-bid-97", 4).await?;

    info!("seed complete — final book: ask@104 qty 5 | bid@99 qty 5 | spread 5");
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn place(
    handler: &mut CommandHandler<PostgresEventLog>,
    command_id: &str,
    order: LimitOrder,
) -> Result<()> {
    let side_str  = match order.side { Side::Ask => "ask", Side::Bid => "bid" };
    let order_price = order.price;
    let order_qty   = order.quantity;

    let result = handler.handle(Command::PlaceOrder {
        command_id: command_id.into(),
        order,
    }).await?;

    if let CommandResult::OrderPlaced(r) = result {
        info!(
            command_id,
            seq     = r.order_accepted.seq,
            side    = side_str,
            price   = order_price,
            qty     = order_qty,
            fills   = r.trades.len(),
            resting = r.remaining_quantity,
            new     = r.inserted,
            "place"
        );
    }
    Ok(())
}

async fn cancel(
    handler: &mut CommandHandler<PostgresEventLog>,
    command_id: &str,
    order_id: u64,
) -> Result<()> {
    let result = handler.handle(Command::CancelOrder {
        command_id: command_id.into(),
        order_id,
    }).await?;

    if let CommandResult::OrderCanceled(r) = result {
        info!(
            command_id,
            seq     = r.event.seq,
            order_id,
            new     = r.inserted,
            "cancel"
        );
    }
    Ok(())
}
