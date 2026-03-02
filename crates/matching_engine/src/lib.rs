use std::collections::{BTreeMap, HashMap, VecDeque};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type OrderId = u64;
pub type UserId = u64;
pub type Price = u64;
pub type Quantity = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Bid,
    Ask,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitOrder {
    pub order_id: OrderId,
    pub user_id: UserId,
    pub side: Side,
    pub price: Price,
    pub quantity: Quantity,
    pub ts_nanos: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trade {
    pub maker_order_id: OrderId,
    pub taker_order_id: OrderId,
    pub price: Price,
    pub quantity: Quantity,
    pub ts_nanos: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchOutcome {
    pub accepted: bool,
    pub remaining_quantity: Quantity,
    pub trades: Vec<Trade>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EngineError {
    #[error("duplicate order id: {0}")]
    DuplicateOrderId(OrderId),
    #[error("invalid order quantity: {0}")]
    InvalidQuantity(Quantity),
    #[error("invalid order price: {0}")]
    InvalidPrice(Price),
    #[error("order not found: {0}")]
    OrderNotFound(OrderId),
}

#[derive(Debug, Default)]
pub struct OrderBook {
    bids: BTreeMap<Price, VecDeque<LimitOrder>>,
    asks: BTreeMap<Price, VecDeque<LimitOrder>>,
    order_locator: HashMap<OrderId, (Side, Price)>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn place_limit(&mut self, mut incoming: LimitOrder) -> Result<MatchOutcome, EngineError> {
        if incoming.quantity == 0 {
            return Err(EngineError::InvalidQuantity(incoming.quantity));
        }
        if incoming.price == 0 {
            return Err(EngineError::InvalidPrice(incoming.price));
        }
        if self.order_locator.contains_key(&incoming.order_id) {
            return Err(EngineError::DuplicateOrderId(incoming.order_id));
        }
        let mut trades = Vec::new();

        match incoming.side {
            Side::Bid => {
                while incoming.quantity > 0 {
                    let Some(best_ask) = self.best_ask_price() else {
                        break;
                    };
                    if best_ask > incoming.price {
                        break;
                    }
                    self.match_with_level(best_ask, &mut incoming, &mut trades);
                }
            }
            Side::Ask => {
                while incoming.quantity > 0 {
                    let Some(best_bid) = self.best_bid_price() else {
                        break;
                    };
                    if best_bid < incoming.price {
                        break;
                    }
                    self.match_with_level(best_bid, &mut incoming, &mut trades);
                }
            }
        }

        if incoming.quantity > 0 {
            self.add_resting_order(incoming.clone());
        }

        Ok(MatchOutcome {
            accepted: true,
            remaining_quantity: incoming.quantity,
            trades,
        })
    }

    pub fn cancel(&mut self, order_id: OrderId) -> Result<(), EngineError> {
        let Some((side, price)) = self.order_locator.remove(&order_id) else {
            return Err(EngineError::OrderNotFound(order_id));
        };

        let book = match side {
            Side::Bid => &mut self.bids,
            Side::Ask => &mut self.asks,
        };

        let mut level = match book.remove(&price) {
            Some(level) => level,
            None => return Err(EngineError::OrderNotFound(order_id)),
        };

        if let Some(pos) = level.iter().position(|o| o.order_id == order_id) {
            level.remove(pos);
        }

        if !level.is_empty() {
            book.insert(price, level);
        }

        Ok(())
    }

    pub fn total_resting_orders(&self) -> usize {
        self.order_locator.len()
    }

    fn best_bid_price(&self) -> Option<Price> {
        self.bids.last_key_value().map(|(p, _)| *p)
    }

    fn best_ask_price(&self) -> Option<Price> {
        self.asks.first_key_value().map(|(p, _)| *p)
    }

    fn add_resting_order(&mut self, order: LimitOrder) {
        let side = order.side;
        let price = order.price;
        let id = order.order_id;
        let book = match side {
            Side::Bid => &mut self.bids,
            Side::Ask => &mut self.asks,
        };

        book.entry(price).or_default().push_back(order);
        self.order_locator.insert(id, (side, price));
    }

    fn match_with_level(
        &mut self,
        level_price: Price,
        incoming: &mut LimitOrder,
        trades: &mut Vec<Trade>,
    ) {
        let book = match incoming.side {
            Side::Bid => &mut self.asks,
            Side::Ask => &mut self.bids,
        };

        let mut level = match book.remove(&level_price) {
            Some(level) => level,
            None => return,
        };

        while incoming.quantity > 0 {
            let Some(mut maker) = level.pop_front() else {
                break;
            };

            let fill_qty = maker.quantity.min(incoming.quantity);
            maker.quantity -= fill_qty;
            incoming.quantity -= fill_qty;

            trades.push(Trade {
                maker_order_id: maker.order_id,
                taker_order_id: incoming.order_id,
                price: level_price,
                quantity: fill_qty,
                ts_nanos: incoming.ts_nanos,
            });

            if maker.quantity == 0 {
                self.order_locator.remove(&maker.order_id);
            } else {
                level.push_front(maker);
                break;
            }
        }

        if !level.is_empty() {
            book.insert(level_price, level);
        }
    }
}
