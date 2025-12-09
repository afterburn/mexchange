use rand::Rng;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use super::Strategy;
use crate::indicators::volatility;
use crate::types::{MarketState, OrderRequest, OrderType, Side};

const NUM_LEVELS: usize = 20;
const BASE_SPREAD_BPS: Decimal = dec!(15);
const MAX_INVENTORY: Decimal = dec!(500);

pub struct MarketMaker {
    inventory: Decimal,
}

impl MarketMaker {
    pub fn new() -> Self {
        Self {
            inventory: Decimal::ZERO,
        }
    }

    fn round_price(price: Decimal) -> Decimal {
        price.round_dp(2)
    }

    fn round_quantity(qty: Decimal) -> Decimal {
        qty.round_dp(2).max(dec!(0.01))
    }
}

impl Strategy for MarketMaker {
    fn name(&self) -> &'static str {
        "MarketMaker"
    }

    fn interval_ms(&self) -> u64 {
        1000
    }

    fn generate_orders(&mut self, market: &MarketState, symbol: &str) -> Vec<OrderRequest> {
        let mid = match market.mid_price() {
            Some(p) => p,
            None => return vec![],
        };

        let mut orders = Vec::with_capacity(NUM_LEVELS * 2);
        let mut rng = rand::thread_rng();

        // Simulate fills (random walk)
        let fill_delta: Decimal = Decimal::from(rng.gen_range(-30i32..=30i32));
        self.inventory = (self.inventory + fill_delta).clamp(-MAX_INVENTORY, MAX_INVENTORY);

        // Calculate volatility-adjusted spread
        let vol = volatility(&market.price_history).unwrap_or(dec!(0.01));
        let vol_adjustment = vol * dec!(100); // bps
        let inventory_risk = (self.inventory.abs() / MAX_INVENTORY) * dec!(10);
        let half_spread = (BASE_SPREAD_BPS + vol_adjustment + inventory_risk) / dec!(10000) * mid;

        // Stoikov-style inventory skew
        let inventory_skew = (self.inventory / MAX_INVENTORY) * half_spread * dec!(0.5);
        let adjusted_mid = mid - inventory_skew;

        // Generate bid levels
        for i in 0..NUM_LEVELS {
            let level_offset = half_spread * Decimal::from(i + 1);
            let price = Self::round_price(adjusted_mid - level_offset);

            // Size decreases with distance from mid
            let base_size = dec!(30) + Decimal::from(rng.gen_range(0u32..20u32));
            let size_factor = dec!(1) - (Decimal::from(i) / Decimal::from(NUM_LEVELS * 2));
            let quantity = Self::round_quantity(base_size * size_factor);

            if price > Decimal::ZERO && quantity > Decimal::ZERO {
                orders.push(OrderRequest {
                    symbol: symbol.to_string(),
                    side: Side::Bid,
                    order_type: OrderType::Limit,
                    price: Some(price),
                    quantity,
                });
            }
        }

        // Generate ask levels
        for i in 0..NUM_LEVELS {
            let level_offset = half_spread * Decimal::from(i + 1);
            let price = Self::round_price(adjusted_mid + level_offset);

            let base_size = dec!(30) + Decimal::from(rng.gen_range(0u32..20u32));
            let size_factor = dec!(1) - (Decimal::from(i) / Decimal::from(NUM_LEVELS * 2));
            let quantity = Self::round_quantity(base_size * size_factor);

            if quantity > Decimal::ZERO {
                orders.push(OrderRequest {
                    symbol: symbol.to_string(),
                    side: Side::Ask,
                    order_type: OrderType::Limit,
                    price: Some(price),
                    quantity,
                });
            }
        }

        orders
    }
}

impl Default for MarketMaker {
    fn default() -> Self {
        Self::new()
    }
}
