use matching_engine::{LimitOrder, OrderBook, Side};

#[test]
fn price_time_priority_is_deterministic() {
    let mut ob = OrderBook::new();

    ob.place_limit(LimitOrder {
        order_id: 1,
        user_id: 10,
        side: Side::Ask,
        price: 100,
        quantity: 5,
        ts_nanos: 1,
    })
    .unwrap();

    ob.place_limit(LimitOrder {
        order_id: 2,
        user_id: 11,
        side: Side::Ask,
        price: 100,
        quantity: 5,
        ts_nanos: 2,
    })
    .unwrap();

    let outcome = ob
        .place_limit(LimitOrder {
            order_id: 3,
            user_id: 12,
            side: Side::Bid,
            price: 105,
            quantity: 7,
            ts_nanos: 3,
        })
        .unwrap();

    assert_eq!(outcome.trades.len(), 2);
    assert_eq!(outcome.trades[0].maker_order_id, 1);
    assert_eq!(outcome.trades[0].quantity, 5);
    assert_eq!(outcome.trades[1].maker_order_id, 2);
    assert_eq!(outcome.trades[1].quantity, 2);
    assert_eq!(ob.total_resting_orders(), 1);
}

#[test]
fn non_crossing_order_rests() {
    let mut ob = OrderBook::new();

    let outcome = ob
        .place_limit(LimitOrder {
            order_id: 42,
            user_id: 1,
            side: Side::Bid,
            price: 99,
            quantity: 10,
            ts_nanos: 1,
        })
        .unwrap();

    assert!(outcome.trades.is_empty());
    assert_eq!(outcome.remaining_quantity, 10);
    assert_eq!(ob.total_resting_orders(), 1);
}
