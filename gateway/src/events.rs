use matching_engine::OrderId;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Bid,
    Ask,
}

impl From<matching_engine::Side> for Side {
    fn from(side: matching_engine::Side) -> Self {
        match side {
            matching_engine::Side::Bid => Side::Bid,
            matching_engine::Side::Ask => Side::Ask,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeltaAction {
    Add,
    Update,
    Remove,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelDelta {
    pub action: DeltaAction,
    pub side: Side,
    pub price: Decimal,
    pub quantity: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MarketEvent {
    #[serde(rename = "fill")]
    Fill {
        symbol: String,
        /// Order ID (UUID) for the buy side
        buy_order_id: OrderId,
        /// Order ID (UUID) for the sell side
        sell_order_id: OrderId,
        price: Decimal,
        quantity: Decimal,
        timestamp: u64,
    },
    /// Full orderbook snapshot - replaces entire state
    #[serde(rename = "orderbook_snapshot")]
    OrderBookSnapshot {
        symbol: String,
        sequence: u64,
        bids: Vec<PriceLevel>,
        asks: Vec<PriceLevel>,
    },
    /// Incremental orderbook update - merge into existing state
    #[serde(rename = "orderbook_delta")]
    OrderBookDelta {
        symbol: String,
        sequence: u64,
        deltas: Vec<LevelDelta>,
    },
    #[serde(rename = "order_accepted")]
    OrderAccepted {
        /// Order ID (UUID)
        order_id: OrderId,
        side: Side,
        order_type: String,
        price: Option<Decimal>,
        quantity: Decimal,
    },
    #[serde(rename = "order_cancelled")]
    OrderCancelled {
        /// Order ID (UUID)
        order_id: OrderId,
        /// How much was filled before cancellation
        filled_quantity: Decimal,
    },
    /// Sent when an order is 100% filled
    #[serde(rename = "order_filled")]
    OrderFilled {
        /// Order ID (UUID)
        order_id: OrderId,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceLevel {
    pub price: Decimal,
    pub quantity: Decimal,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderRequest {
    pub side: Side,
    pub order_type: String,
    pub price: Option<Decimal>,
    pub quantity: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelOrderRequest {
    pub order_id: OrderId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OrderCommand {
    #[serde(rename = "place_order")]
    PlaceOrder {
        /// Order ID (UUID)
        order_id: OrderId,
        side: Side,
        order_type: String,
        price: Option<Decimal>,
        quantity: Decimal,
        user_id: Option<OrderId>,
    },
    #[serde(rename = "cancel_order")]
    CancelOrder {
        /// Order ID (UUID) to cancel
        order_id: OrderId,
        user_id: Option<OrderId>,
    },
}

