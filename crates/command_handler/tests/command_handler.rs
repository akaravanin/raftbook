use command_handler::{Command, CommandError, CommandResult, CommandHandler};
use event_log::InMemoryEventLog;
use matching_engine::{LimitOrder, Side};

fn make_log() -> InMemoryEventLog {
    InMemoryEventLog::new()
}

fn ask(order_id: u64, price: u64, qty: u64) -> LimitOrder {
    LimitOrder { order_id, user_id: 1, side: Side::Ask, price, quantity: qty, ts_nanos: order_id }
}

fn bid(order_id: u64, price: u64, qty: u64) -> LimitOrder {
    LimitOrder { order_id, user_id: 2, side: Side::Bid, price, quantity: qty, ts_nanos: order_id }
}

// ── PlaceOrder: resting order ─────────────────────────────────────────────────

#[tokio::test]
async fn place_non_crossing_order_rests_and_emits_accepted() {
    let mut h = CommandHandler::new(make_log());

    let res = h.handle(Command::PlaceOrder {
        command_id: "cmd-1".into(),
        order: bid(1, 100, 10),
    }).await.unwrap();

    let CommandResult::OrderPlaced(r) = res else { panic!("wrong variant") };
    assert!(r.inserted);
    assert!(r.trades.is_empty());
    assert_eq!(r.remaining_quantity, 10);
    assert_eq!(h.resting_order_count(), 1);
}

// ── PlaceOrder: full fill ──────────────────────────────────────────────────────

#[tokio::test]
async fn place_crossing_order_produces_trade_events() {
    let mut h = CommandHandler::new(make_log());

    h.handle(Command::PlaceOrder {
        command_id: "cmd-ask".into(),
        order: ask(1, 100, 5),
    }).await.unwrap();

    let res = h.handle(Command::PlaceOrder {
        command_id: "cmd-bid".into(),
        order: bid(2, 105, 5),
    }).await.unwrap();

    let CommandResult::OrderPlaced(r) = res else { panic!("wrong variant") };
    assert!(r.inserted);
    assert_eq!(r.trades.len(), 1);
    assert_eq!(r.remaining_quantity, 0);
    assert_eq!(h.resting_order_count(), 0);
}

// ── PlaceOrder: partial fill ───────────────────────────────────────────────────

#[tokio::test]
async fn partial_fill_leaves_remainder_resting() {
    let mut h = CommandHandler::new(make_log());

    h.handle(Command::PlaceOrder {
        command_id: "cmd-ask".into(),
        order: ask(1, 100, 3),
    }).await.unwrap();

    let res = h.handle(Command::PlaceOrder {
        command_id: "cmd-bid".into(),
        order: bid(2, 100, 5),
    }).await.unwrap();

    let CommandResult::OrderPlaced(r) = res else { panic!("wrong variant") };
    assert_eq!(r.trades.len(), 1);
    assert_eq!(r.remaining_quantity, 2);
    assert_eq!(h.resting_order_count(), 1); // bid remainder rests
}

// ── CancelOrder ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn cancel_resting_order_removes_from_book() {
    let mut h = CommandHandler::new(make_log());

    h.handle(Command::PlaceOrder {
        command_id: "cmd-1".into(),
        order: bid(42, 99, 10),
    }).await.unwrap();

    assert_eq!(h.resting_order_count(), 1);

    let res = h.handle(Command::CancelOrder {
        command_id: "cmd-cancel-42".into(),
        order_id: 42,
    }).await.unwrap();

    let CommandResult::OrderCanceled(r) = res else { panic!("wrong variant") };
    assert!(r.inserted);
    assert_eq!(h.resting_order_count(), 0);
}

#[tokio::test]
async fn cancel_unknown_order_returns_engine_error() {
    let mut h = CommandHandler::new(make_log());

    let err = h.handle(Command::CancelOrder {
        command_id: "cmd-cancel-999".into(),
        order_id: 999,
    }).await.unwrap_err();

    assert!(matches!(err, CommandError::Engine(_)));
}

// ── Idempotency ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn duplicate_place_order_command_is_idempotent() {
    let mut h = CommandHandler::new(make_log());

    let first = h.handle(Command::PlaceOrder {
        command_id: "cmd-dup".into(),
        order: bid(1, 100, 5),
    }).await.unwrap();

    let CommandResult::OrderPlaced(r1) = first else { panic!() };
    assert!(r1.inserted);

    // Retry with same command_id — must not add a second order to the book.
    let retry = h.handle(Command::PlaceOrder {
        command_id: "cmd-dup".into(),
        order: bid(1, 100, 5),
    }).await.unwrap();

    let CommandResult::OrderPlaced(r2) = retry else { panic!() };
    assert!(!r2.inserted);
    // The accepted event record is the same sequence number.
    assert_eq!(r1.order_accepted.seq, r2.order_accepted.seq);
    // Book still has exactly one order, not two.
    assert_eq!(h.resting_order_count(), 1);
}

#[tokio::test]
async fn duplicate_cancel_command_is_idempotent() {
    let mut h = CommandHandler::new(make_log());

    h.handle(Command::PlaceOrder {
        command_id: "cmd-place".into(),
        order: bid(7, 50, 3),
    }).await.unwrap();

    h.handle(Command::CancelOrder {
        command_id: "cmd-cancel-7".into(),
        order_id: 7,
    }).await.unwrap();

    // Second cancel retry: should not error even though order is gone.
    let retry = h.handle(Command::CancelOrder {
        command_id: "cmd-cancel-7".into(),
        order_id: 7,
    }).await.unwrap();

    let CommandResult::OrderCanceled(r) = retry else { panic!() };
    assert!(!r.inserted);
}

// ── Event log consistency ─────────────────────────────────────────────────────

#[tokio::test]
async fn event_log_contains_correct_sequence_of_events() {
    use event_log::{AppendOnlyLog, Event, IdempotentEventLog};

    let mut log = make_log();

    // We need to interact with the log directly for this test, so we
    // duplicate a small scenario inline.
    log.append_idempotent("cmd-a1", Event::OrderAccepted { order_id: 1 }).await.unwrap();
    log.append_idempotent("cmd-a2", Event::OrderAccepted { order_id: 2 }).await.unwrap();

    let records = log.read_from(0);
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].seq, 0);
    assert_eq!(records[1].seq, 1);
}
