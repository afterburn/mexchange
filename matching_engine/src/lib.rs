use rust_decimal::Decimal;
use std::collections::{BTreeMap, HashMap, VecDeque};
pub use uuid::Uuid;

pub type OrderId = Uuid;

// Decimal supports high precision needed for cryptocurrency (e.g., Bitcoin 0.00000001)
pub type Price = Decimal;
pub type Quantity = Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Bid,
    Ask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Limit,
    Market,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    pub id: OrderId,
    pub side: Side,
    pub order_type: OrderType,
    pub price: Option<Price>,
    pub quantity: Quantity,
    pub remaining_quantity: Quantity,
}

impl Order {
    pub fn new_limit(id: OrderId, side: Side, price: Price, quantity: Quantity) -> Self {
        Self {
            id,
            side,
            order_type: OrderType::Limit,
            price: Some(price),
            quantity,
            remaining_quantity: quantity,
        }
    }

    pub fn new_market(id: OrderId, side: Side, quantity: Quantity) -> Self {
        Self {
            id,
            side,
            order_type: OrderType::Market,
            price: None,
            quantity,
            remaining_quantity: quantity,
        }
    }

    pub fn is_filled(&self) -> bool {
        self.remaining_quantity.is_zero()
    }
}

#[derive(Debug, Clone)]
struct PriceLevel {
    #[allow(dead_code)]
    price: Price,
    orders: VecDeque<Order>,
    total_quantity: Quantity,
}

impl PriceLevel {
    fn new(price: Price) -> Self {
        Self {
            price,
            orders: VecDeque::new(),
            total_quantity: Decimal::ZERO,
        }
    }

    fn add_order(&mut self, order: Order) {
        self.total_quantity += order.remaining_quantity;
        self.orders.push_back(order);
    }

    fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fill {
    pub buy_order_id: OrderId,
    pub sell_order_id: OrderId,
    pub price: Price,
    pub quantity: Quantity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderResult {
    pub order_id: OrderId,
    pub fills: Vec<Fill>,
    /// Order IDs that were fully filled as a result of this order (includes the order itself if fully filled)
    pub completed_orders: Vec<OrderId>,
}

pub struct OrderBook {
    // BTreeMap for price levels - sorted by price
    // For bids: higher prices first (descending)
    // For asks: lower prices first (ascending)
    bids: BTreeMap<Price, PriceLevel>,
    asks: BTreeMap<Price, PriceLevel>,
    orders: HashMap<OrderId, Order>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            orders: HashMap::new(),
        }
    }

    pub fn add_limit_order(
        &mut self,
        order_id: OrderId,
        side: Side,
        price: Price,
        quantity: Quantity,
    ) -> OrderResult {
        let mut order = Order::new_limit(order_id, side, price, quantity);
        let mut fills = Vec::new();
        let mut completed_orders = Vec::new();

        self.match_order(&mut order, &mut fills, &mut completed_orders);

        if order.is_filled() {
            completed_orders.push(order_id);
        } else {
            self.add_order_to_book(order.clone());
        }

        OrderResult { order_id, fills, completed_orders }
    }

    pub fn add_market_order(
        &mut self,
        order_id: OrderId,
        side: Side,
        quantity: Quantity,
    ) -> OrderResult {
        let mut order = Order::new_market(order_id, side, quantity);
        let mut fills = Vec::new();
        let mut completed_orders = Vec::new();

        self.match_order(&mut order, &mut fills, &mut completed_orders);

        // Market orders are always "complete" (either filled or cancelled)
        if order.is_filled() {
            completed_orders.push(order_id);
        }

        OrderResult { order_id, fills, completed_orders }
    }

    /// Get a reference to an order by ID (for snapshotting before matching)
    pub fn get_order(&self, order_id: OrderId) -> Option<&Order> {
        self.orders.get(&order_id)
    }

    /// Restore an order to the book (for rollback after failed settlement)
    /// This re-adds the order with its current remaining_quantity
    pub fn restore_order(&mut self, order: Order) {
        if order.order_type == OrderType::Market || order.is_filled() {
            return; // Can't restore market orders or fully filled orders
        }
        self.add_order_to_book(order);
    }

    pub fn cancel_order(&mut self, order_id: OrderId) -> bool {
        let Some(order) = self.orders.remove(&order_id) else {
            return false;
        };

        self.remove_order_from_book(&order);
        true
    }

    /// Check if an order exists in the book (i.e., has remaining unfilled quantity)
    pub fn order_exists(&self, order_id: OrderId) -> bool {
        self.orders.contains_key(&order_id)
    }

    pub fn best_bid(&self) -> Option<Price> {
        self.bids.keys().next_back().copied()
    }

    pub fn best_ask(&self) -> Option<Price> {
        self.asks.keys().next().copied()
    }

    pub fn spread(&self) -> Option<Price> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) if ask > bid => Some(ask - bid),
            _ => None,
        }
    }

    pub fn quantity_at_price(&self, side: Side, price: Price) -> Quantity {
        let book = match side {
            Side::Bid => &self.bids,
            Side::Ask => &self.asks,
        };
        book.get(&price).map_or(Decimal::ZERO, |level| level.total_quantity)
    }

    /// Returns bid price levels sorted by price descending (best bid first)
    pub fn get_bids(&self, max_levels: usize) -> Vec<(Price, Quantity)> {
        self.bids
            .iter()
            .rev() // BTreeMap is ascending, we want descending for bids
            .take(max_levels)
            .map(|(price, level)| (*price, level.total_quantity))
            .collect()
    }

    /// Returns ask price levels sorted by price ascending (best ask first)
    pub fn get_asks(&self, max_levels: usize) -> Vec<(Price, Quantity)> {
        self.asks
            .iter()
            .take(max_levels)
            .map(|(price, level)| (*price, level.total_quantity))
            .collect()
    }

    fn match_order(&mut self, order: &mut Order, fills: &mut Vec<Fill>, completed_orders: &mut Vec<OrderId>) {
        let opposite_book = match order.side {
            Side::Bid => &mut self.asks,
            Side::Ask => &mut self.bids,
        };

        // Bids match against lowest asks first, asks match against highest bids first
        let prices_to_match: Vec<Price> = if order.side == Side::Ask {
            opposite_book.keys().copied().rev().collect()
        } else {
            opposite_book.keys().copied().collect()
        };

        let mut prices_to_remove = Vec::new();

        for price in prices_to_match {
            if order.is_filled() {
                break;
            }

            let can_match = match order.order_type {
                OrderType::Market => true,
                OrderType::Limit => {
                    let order_price = order.price.unwrap();
                    match order.side {
                        Side::Bid => price <= order_price,
                        Side::Ask => price >= order_price,
                    }
                }
            };

            if !can_match {
                break;
            }

            let Some(level) = opposite_book.get_mut(&price) else {
                continue;
            };

            while !level.orders.is_empty() && !order.is_filled() {
                let mut opposite_order = level.orders.pop_front().unwrap();
                let fill_quantity = order.remaining_quantity.min(opposite_order.remaining_quantity);

                order.remaining_quantity -= fill_quantity;
                opposite_order.remaining_quantity -= fill_quantity;
                level.total_quantity -= fill_quantity;

                let fill = match order.side {
                    Side::Bid => Fill {
                        buy_order_id: order.id,
                        sell_order_id: opposite_order.id,
                        price,
                        quantity: fill_quantity,
                    },
                    Side::Ask => Fill {
                        buy_order_id: opposite_order.id,
                        sell_order_id: order.id,
                        price,
                        quantity: fill_quantity,
                    },
                };
                fills.push(fill);

                if opposite_order.is_filled() {
                    // Track that this resting order was fully filled
                    completed_orders.push(opposite_order.id);
                    self.orders.remove(&opposite_order.id);
                } else {
                    level.orders.push_front(opposite_order.clone());
                    self.orders.insert(opposite_order.id, opposite_order);
                }
            }

            if level.is_empty() {
                prices_to_remove.push(price);
            }
        }

        for price in prices_to_remove {
            opposite_book.remove(&price);
        }
    }

    fn add_order_to_book(&mut self, order: Order) {
        let price = order.price.expect("Limit order must have a price");
        let book = match order.side {
            Side::Bid => &mut self.bids,
            Side::Ask => &mut self.asks,
        };

        self.orders.insert(order.id, order.clone());

        book.entry(price)
            .or_insert_with(|| PriceLevel::new(price))
            .add_order(order);
    }

    fn remove_order_from_book(&mut self, order: &Order) {
        let Some(price) = order.price else {
            return;
        };

        let book = match order.side {
            Side::Bid => &mut self.bids,
            Side::Ask => &mut self.asks,
        };

        let Some(level) = book.get_mut(&price) else {
            return;
        };

        let mut removed_quantity = Decimal::ZERO;
        level.orders.retain(|o| {
            if o.id == order.id {
                removed_quantity = o.remaining_quantity;
                false
            } else {
                true
            }
        });
        level.total_quantity -= removed_quantity;

        if level.is_empty() {
            book.remove(&price);
        }
    }
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_id() -> OrderId {
        Uuid::new_v4()
    }

    #[test]
    fn test_create_orderbook() {
        let ob = OrderBook::new();
        assert_eq!(ob.best_bid(), None);
        assert_eq!(ob.best_ask(), None);
    }

    #[test]
    fn test_add_limit_bid() {
        let mut ob = OrderBook::new();
        let order_id = new_id();
        let result = ob.add_limit_order(order_id, Side::Bid, Decimal::from(100), Decimal::from(10));

        assert_eq!(result.order_id, order_id);
        assert_eq!(result.fills.len(), 0);

        assert_eq!(ob.best_bid(), Some(Decimal::from(100)));
    }

    #[test]
    fn test_add_limit_ask() {
        let mut ob = OrderBook::new();
        let order_id = new_id();
        let result = ob.add_limit_order(order_id, Side::Ask, Decimal::from(100), Decimal::from(10));

        assert_eq!(result.order_id, order_id);
        assert_eq!(result.fills.len(), 0);

        assert_eq!(ob.best_ask(), Some(Decimal::from(100)));
    }

    #[test]
    fn test_simple_match() {
        let mut ob = OrderBook::new();

        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(100), Decimal::from(10));

        let result = ob.add_limit_order(new_id(), Side::Bid, Decimal::from(100), Decimal::from(10));

        assert_eq!(result.fills.len(), 1);
        assert_eq!(result.fills[0].price, Decimal::from(100));
        assert_eq!(result.fills[0].quantity, Decimal::from(10));

        assert_eq!(ob.best_bid(), None);
        assert_eq!(ob.best_ask(), None);
    }

    #[test]
    fn test_partial_fill() {
        let mut ob = OrderBook::new();

        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(100), Decimal::from(5));

        let result = ob.add_limit_order(new_id(), Side::Bid, Decimal::from(100), Decimal::from(10));

        assert_eq!(result.fills.len(), 1);
        assert_eq!(result.fills[0].quantity, Decimal::from(5));

        assert_eq!(ob.best_bid(), Some(Decimal::from(100)));
        assert_eq!(ob.quantity_at_price(Side::Bid, Decimal::from(100)), Decimal::from(5));
    }

    #[test]
    fn test_partial_fill_across_levels() {
        let mut ob = OrderBook::new();

        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(100), Decimal::from(5));
        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(101), Decimal::from(5));
        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(102), Decimal::from(5));

        let result = ob.add_limit_order(new_id(), Side::Bid, Decimal::from(102), Decimal::from(12));

        assert_eq!(result.fills.len(), 3);

        assert_eq!(result.fills[0].price, Decimal::from(100));
        assert_eq!(result.fills[0].quantity, Decimal::from(5));

        assert_eq!(result.fills[1].price, Decimal::from(101));
        assert_eq!(result.fills[1].quantity, Decimal::from(5));

        assert_eq!(result.fills[2].price, Decimal::from(102));
        assert_eq!(result.fills[2].quantity, Decimal::from(2));

        assert_eq!(ob.best_ask(), Some(Decimal::from(102)));
        assert_eq!(ob.quantity_at_price(Side::Ask, Decimal::from(102)), Decimal::from(3));
    }

    #[test]
    fn test_market_order_buy() {
        let mut ob = OrderBook::new();

        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(100), Decimal::from(5));
        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(101), Decimal::from(5));

        let result = ob.add_market_order(new_id(), Side::Bid, Decimal::from(7));

        assert_eq!(result.fills.len(), 2);
        assert_eq!(result.fills[0].price, Decimal::from(100));
        assert_eq!(result.fills[0].quantity, Decimal::from(5));
        assert_eq!(result.fills[1].price, Decimal::from(101));
        assert_eq!(result.fills[1].quantity, Decimal::from(2));

        assert_eq!(ob.best_ask(), Some(Decimal::from(101)));
        assert_eq!(ob.quantity_at_price(Side::Ask, Decimal::from(101)), Decimal::from(3));
    }

    #[test]
    fn test_market_order_sell() {
        let mut ob = OrderBook::new();

        ob.add_limit_order(new_id(), Side::Bid, Decimal::from(100), Decimal::from(5));
        ob.add_limit_order(new_id(), Side::Bid, Decimal::from(99), Decimal::from(5));

        let result = ob.add_market_order(new_id(), Side::Ask, Decimal::from(7));

        assert_eq!(result.fills.len(), 2);
        assert_eq!(result.fills[0].price, Decimal::from(100));
        assert_eq!(result.fills[0].quantity, Decimal::from(5));
        assert_eq!(result.fills[1].price, Decimal::from(99));
        assert_eq!(result.fills[1].quantity, Decimal::from(2));

        assert_eq!(ob.best_bid(), Some(Decimal::from(99)));
        assert_eq!(ob.quantity_at_price(Side::Bid, Decimal::from(99)), Decimal::from(3));
    }

    #[test]
    fn test_cancel_order() {
        let mut ob = OrderBook::new();

        let order_id = new_id();
        let result = ob.add_limit_order(order_id, Side::Bid, Decimal::from(100), Decimal::from(10));

        assert_eq!(ob.best_bid(), Some(Decimal::from(100)));

        assert!(ob.cancel_order(result.order_id));
        assert_eq!(ob.best_bid(), None);

        assert!(!ob.cancel_order(order_id));
    }

    #[test]
    fn test_spread() {
        let mut ob = OrderBook::new();

        assert_eq!(ob.spread(), None);

        ob.add_limit_order(new_id(), Side::Bid, Decimal::from(99), Decimal::from(10));
        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(101), Decimal::from(10));

        assert_eq!(ob.spread(), Some(Decimal::from(2)));
    }

    #[test]
    fn test_price_priority() {
        let mut ob = OrderBook::new();

        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(102), Decimal::from(5));
        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(100), Decimal::from(5));
        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(101), Decimal::from(5));

        assert_eq!(ob.best_ask(), Some(Decimal::from(100)));

        let result = ob.add_market_order(new_id(), Side::Bid, Decimal::from(12));

        assert_eq!(result.fills[0].price, Decimal::from(100));
        assert_eq!(result.fills[1].price, Decimal::from(101));
        assert_eq!(result.fills[2].price, Decimal::from(102));
    }

    #[test]
    fn test_time_priority() {
        let mut ob = OrderBook::new();

        let order_id_1 = new_id();
        let result1 = ob.add_limit_order(order_id_1, Side::Ask, Decimal::from(100), Decimal::from(5));

        let order_id_2 = new_id();
        let result2 = ob.add_limit_order(order_id_2, Side::Ask, Decimal::from(100), Decimal::from(5));

        let result = ob.add_limit_order(new_id(), Side::Bid, Decimal::from(100), Decimal::from(5));

        assert_eq!(result.fills.len(), 1);
        assert_eq!(result.fills[0].sell_order_id, result1.order_id);

        assert_eq!(ob.quantity_at_price(Side::Ask, Decimal::from(100)), Decimal::from(5));

        let result = ob.add_limit_order(new_id(), Side::Bid, Decimal::from(100), Decimal::from(5));
        assert_eq!(result.fills[0].sell_order_id, result2.order_id);
    }

    #[test]
    fn test_no_match_when_prices_dont_cross() {
        let mut ob = OrderBook::new();

        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(101), Decimal::from(10));

        let result = ob.add_limit_order(new_id(), Side::Bid, Decimal::from(99), Decimal::from(10));

        assert_eq!(result.fills.len(), 0);

        assert_eq!(ob.best_bid(), Some(Decimal::from(99)));
        assert_eq!(ob.best_ask(), Some(Decimal::from(101)));
    }

    #[test]
    fn test_market_order_with_empty_book() {
        let mut ob = OrderBook::new();

        let result = ob.add_market_order(new_id(), Side::Bid, Decimal::from(10));

        assert_eq!(result.fills.len(), 0);
    }

    #[test]
    fn test_complex_scenario() {
        let mut ob = OrderBook::new();

        ob.add_limit_order(new_id(), Side::Bid, Decimal::from(95), Decimal::from(10));
        ob.add_limit_order(new_id(), Side::Bid, Decimal::from(96), Decimal::from(15));
        ob.add_limit_order(new_id(), Side::Bid, Decimal::from(97), Decimal::from(20));

        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(103), Decimal::from(10));
        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(102), Decimal::from(15));
        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(101), Decimal::from(20));

        assert_eq!(ob.best_bid(), Some(Decimal::from(97)));
        assert_eq!(ob.best_ask(), Some(Decimal::from(101)));
        assert_eq!(ob.spread(), Some(Decimal::from(4)));

        let result = ob.add_market_order(new_id(), Side::Ask, Decimal::from(40));

        assert_eq!(result.fills.len(), 3);
        assert_eq!(result.fills[0].price, Decimal::from(97));
        assert_eq!(result.fills[0].quantity, Decimal::from(20));
        assert_eq!(result.fills[1].price, Decimal::from(96));
        assert_eq!(result.fills[1].quantity, Decimal::from(15));
        assert_eq!(result.fills[2].price, Decimal::from(95));
        assert_eq!(result.fills[2].quantity, Decimal::from(5));

        assert_eq!(ob.best_bid(), Some(Decimal::from(95)));
        assert_eq!(ob.quantity_at_price(Side::Bid, Decimal::from(95)), Decimal::from(5));
        assert_eq!(ob.best_ask(), Some(Decimal::from(101)));
    }

    #[test]
    fn test_multiple_orders_same_level() {
        let mut ob = OrderBook::new();

        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(100), Decimal::from(3));
        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(100), Decimal::from(4));
        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(100), Decimal::from(5));

        assert_eq!(ob.quantity_at_price(Side::Ask, Decimal::from(100)), Decimal::from(12));

        let result = ob.add_limit_order(new_id(), Side::Bid, Decimal::from(100), Decimal::from(8));

        assert_eq!(result.fills.len(), 3);
        assert_eq!(result.fills[0].quantity, Decimal::from(3));
        assert_eq!(result.fills[1].quantity, Decimal::from(4));
        assert_eq!(result.fills[2].quantity, Decimal::from(1));

        assert_eq!(ob.quantity_at_price(Side::Ask, Decimal::from(100)), Decimal::from(4));
    }

    #[test]
    fn test_bid_matching_multiple_asks() {
        let mut ob = OrderBook::new();

        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(100), Decimal::from(10));
        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(101), Decimal::from(10));
        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(102), Decimal::from(10));
        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(103), Decimal::from(10));

        let result = ob.add_limit_order(new_id(), Side::Bid, Decimal::from(102), Decimal::from(25));

        assert_eq!(result.fills.len(), 3);
        assert_eq!(result.fills[0].price, Decimal::from(100));
        assert_eq!(result.fills[0].quantity, Decimal::from(10));
        assert_eq!(result.fills[1].price, Decimal::from(101));
        assert_eq!(result.fills[1].quantity, Decimal::from(10));
        assert_eq!(result.fills[2].price, Decimal::from(102));
        assert_eq!(result.fills[2].quantity, Decimal::from(5));

        assert_eq!(ob.best_ask(), Some(Decimal::from(102)));
        assert_eq!(ob.quantity_at_price(Side::Ask, Decimal::from(102)), Decimal::from(5));
    }

    #[test]
    fn test_ask_matching_multiple_bids() {
        let mut ob = OrderBook::new();

        ob.add_limit_order(new_id(), Side::Bid, Decimal::from(103), Decimal::from(10));
        ob.add_limit_order(new_id(), Side::Bid, Decimal::from(102), Decimal::from(10));
        ob.add_limit_order(new_id(), Side::Bid, Decimal::from(101), Decimal::from(10));
        ob.add_limit_order(new_id(), Side::Bid, Decimal::from(100), Decimal::from(10));

        let result = ob.add_limit_order(new_id(), Side::Ask, Decimal::from(101), Decimal::from(25));

        assert_eq!(result.fills.len(), 3);
        assert_eq!(result.fills[0].price, Decimal::from(103));
        assert_eq!(result.fills[0].quantity, Decimal::from(10));
        assert_eq!(result.fills[1].price, Decimal::from(102));
        assert_eq!(result.fills[1].quantity, Decimal::from(10));
        assert_eq!(result.fills[2].price, Decimal::from(101));
        assert_eq!(result.fills[2].quantity, Decimal::from(5));

        assert_eq!(ob.best_bid(), Some(Decimal::from(101)));
        assert_eq!(ob.quantity_at_price(Side::Bid, Decimal::from(101)), Decimal::from(5));
    }

    #[test]
    fn test_cancel_partial_filled_order() {
        let mut ob = OrderBook::new();

        let order_id = new_id();
        let result = ob.add_limit_order(order_id, Side::Bid, Decimal::from(100), Decimal::from(20));

        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(100), Decimal::from(5));

        assert_eq!(ob.quantity_at_price(Side::Bid, Decimal::from(100)), Decimal::from(15));

        assert!(ob.cancel_order(result.order_id));
        assert_eq!(ob.best_bid(), None);
    }

    #[test]
    fn test_large_orderbook_scenario() {
        let mut ob = OrderBook::new();

        for i in 90..=99 {
            ob.add_limit_order(new_id(), Side::Bid, Decimal::from(i as i64), Decimal::from(10));
        }

        for i in 101..=110 {
            ob.add_limit_order(new_id(), Side::Ask, Decimal::from(i as i64), Decimal::from(10));
        }

        assert_eq!(ob.best_bid(), Some(Decimal::from(99)));
        assert_eq!(ob.best_ask(), Some(Decimal::from(101)));
        assert_eq!(ob.spread(), Some(Decimal::from(2)));

        let result = ob.add_market_order(new_id(), Side::Ask, Decimal::from(95));

        assert_eq!(result.fills.len(), 10);

        for i in 0..9 {
            assert_eq!(result.fills[i].price, Decimal::from((99 - i) as i64));
            assert_eq!(result.fills[i].quantity, Decimal::from(10));
        }
        assert_eq!(result.fills[9].price, Decimal::from(90));
        assert_eq!(result.fills[9].quantity, Decimal::from(5));

        assert_eq!(ob.best_bid(), Some(Decimal::from(90)));
        assert_eq!(ob.quantity_at_price(Side::Bid, Decimal::from(90)), Decimal::from(5));
    }

    #[test]
    fn test_interleaved_orders() {
        let mut ob = OrderBook::new();

        ob.add_limit_order(new_id(), Side::Bid, Decimal::from(98), Decimal::from(10));
        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(102), Decimal::from(10));
        ob.add_limit_order(new_id(), Side::Bid, Decimal::from(99), Decimal::from(10));
        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(101), Decimal::from(10));
        ob.add_limit_order(new_id(), Side::Bid, Decimal::from(97), Decimal::from(10));
        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(103), Decimal::from(10));

        assert_eq!(ob.best_bid(), Some(Decimal::from(99)));
        assert_eq!(ob.best_ask(), Some(Decimal::from(101)));

        let result = ob.add_limit_order(new_id(), Side::Bid, Decimal::from(102), Decimal::from(15));

        assert_eq!(result.fills.len(), 2);
        assert_eq!(result.fills[0].price, Decimal::from(101));
        assert_eq!(result.fills[0].quantity, Decimal::from(10));
        assert_eq!(result.fills[1].price, Decimal::from(102));
        assert_eq!(result.fills[1].quantity, Decimal::from(5));

        assert_eq!(ob.best_ask(), Some(Decimal::from(102)));
        assert_eq!(ob.quantity_at_price(Side::Ask, Decimal::from(102)), Decimal::from(5));
    }

    #[test]
    fn test_order_id_preserved() {
        let mut ob = OrderBook::new();

        let id1 = new_id();
        let id2 = new_id();
        let id3 = new_id();

        let r1 = ob.add_limit_order(id1, Side::Bid, Decimal::from(100), Decimal::from(10));
        let r2 = ob.add_limit_order(id2, Side::Ask, Decimal::from(101), Decimal::from(10));
        let r3 = ob.add_limit_order(id3, Side::Bid, Decimal::from(99), Decimal::from(10));

        assert_eq!(r1.order_id, id1);
        assert_eq!(r2.order_id, id2);
        assert_eq!(r3.order_id, id3);
    }

    #[test]
    fn test_complete_fill_across_many_levels() {
        let mut ob = OrderBook::new();

        for i in 100..110 {
            ob.add_limit_order(new_id(), Side::Ask, Decimal::from(i as i64), Decimal::from(10));
        }

        let result = ob.add_limit_order(new_id(), Side::Bid, Decimal::from(109), Decimal::from(100));

        assert_eq!(result.fills.len(), 10);

        let total_filled: Decimal = result.fills.iter().map(|f| f.quantity).sum();
        assert_eq!(total_filled, Decimal::from(100));

        assert_eq!(ob.best_ask(), None);
    }

    #[test]
    fn test_market_order_partial_liquidity() {
        let mut ob = OrderBook::new();

        ob.add_limit_order(new_id(), Side::Ask, Decimal::from(100), Decimal::from(10));

        let result = ob.add_market_order(new_id(), Side::Bid, Decimal::from(20));

        assert_eq!(result.fills.len(), 1);
        assert_eq!(result.fills[0].quantity, Decimal::from(10));

        assert_eq!(ob.best_ask(), None);
    }

    #[test]
    fn test_self_matching_prevented() {
        let mut ob = OrderBook::new();

        ob.add_limit_order(new_id(), Side::Bid, Decimal::from(100), Decimal::from(10));

        let result = ob.add_limit_order(new_id(), Side::Ask, Decimal::from(101), Decimal::from(10));

        assert_eq!(result.fills.len(), 0);

        assert_eq!(ob.best_bid(), Some(Decimal::from(100)));
        assert_eq!(ob.best_ask(), Some(Decimal::from(101)));
    }

    #[test]
    fn test_empty_book_operations() {
        let ob = OrderBook::new();

        assert_eq!(ob.best_bid(), None);
        assert_eq!(ob.best_ask(), None);
        assert_eq!(ob.spread(), None);
        assert_eq!(ob.quantity_at_price(Side::Bid, Decimal::from(100)), Decimal::from(0));
        assert_eq!(ob.quantity_at_price(Side::Ask, Decimal::from(100)), Decimal::from(0));
    }

    #[test]
    fn test_cancel_nonexistent_order() {
        let mut ob = OrderBook::new();

        assert!(!ob.cancel_order(new_id()));
    }

    #[test]
    fn test_multiple_cancellations() {
        let mut ob = OrderBook::new();

        let id1 = new_id();
        let id2 = new_id();
        let id3 = new_id();

        ob.add_limit_order(id1, Side::Bid, Decimal::from(100), Decimal::from(10));
        ob.add_limit_order(id2, Side::Bid, Decimal::from(99), Decimal::from(10));
        ob.add_limit_order(id3, Side::Bid, Decimal::from(98), Decimal::from(10));

        assert_eq!(ob.best_bid(), Some(Decimal::from(100)));

        assert!(ob.cancel_order(id1));
        assert_eq!(ob.best_bid(), Some(Decimal::from(99)));

        assert!(ob.cancel_order(id3));
        assert_eq!(ob.best_bid(), Some(Decimal::from(99)));

        assert!(ob.cancel_order(id2));
        assert_eq!(ob.best_bid(), None);
    }

    #[test]
    fn test_get_order() {
        let mut ob = OrderBook::new();

        let order_id = new_id();
        ob.add_limit_order(order_id, Side::Bid, Decimal::from(100), Decimal::from(10));

        let order = ob.get_order(order_id);
        assert!(order.is_some());
        let order = order.unwrap();
        assert_eq!(order.id, order_id);
        assert_eq!(order.side, Side::Bid);
        assert_eq!(order.remaining_quantity, Decimal::from(10));

        // Non-existent order
        assert!(ob.get_order(new_id()).is_none());
    }

    #[test]
    fn test_restore_order() {
        let mut ob = OrderBook::new();

        // Add and then cancel an order
        let order_id = new_id();
        ob.add_limit_order(order_id, Side::Bid, Decimal::from(100), Decimal::from(10));

        let order = ob.get_order(order_id).unwrap().clone();
        assert!(ob.cancel_order(order_id));
        assert_eq!(ob.best_bid(), None);

        // Restore the order
        ob.restore_order(order);
        assert_eq!(ob.best_bid(), Some(Decimal::from(100)));
        assert_eq!(ob.quantity_at_price(Side::Bid, Decimal::from(100)), Decimal::from(10));
    }
}
