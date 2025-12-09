use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MarketEvent {
    #[serde(rename = "fill")]
    Fill {
        symbol: String,
        buy_order_id: u64,
        sell_order_id: u64,
        price: Decimal,
        quantity: Decimal,
        timestamp: u64,
    },
    #[serde(other)]
    Unknown,
}
