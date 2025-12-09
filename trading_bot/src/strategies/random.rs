use rand::Rng;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use super::Strategy;
use crate::indicators::trend;
use crate::types::{MarketState, OrderRequest, OrderType, Side};

pub struct Random;

impl Random {
    pub fn new() -> Self {
        Self
    }

    fn round_price(price: Decimal) -> Decimal {
        price.round_dp(2)
    }

    fn round_quantity(qty: Decimal) -> Decimal {
        qty.round_dp(2).max(dec!(0.01))
    }
}

impl Strategy for Random {
    fn name(&self) -> &'static str {
        "Random"
    }

    fn interval_ms(&self) -> u64 {
        500
    }

    fn generate_orders(&mut self, market: &MarketState, symbol: &str) -> Vec<OrderRequest> {
        let mid = match market.mid_price() {
            Some(p) => p,
            None => return vec![],
        };

        let mut orders = Vec::new();
        let mut rng = rand::thread_rng();

        // 3-8 orders per execution
        let num_orders = rng.gen_range(3..=8);

        // Trend-following bias (FOMO/panic behavior)
        let current_trend = trend(&market.price_history, 5, 5).unwrap_or(Decimal::ZERO);
        let trend_bias = (current_trend / dec!(100)).clamp(dec!(-0.2), dec!(0.2));
        let buy_probability = dec!(0.5) + trend_bias;

        for _ in 0..num_orders {
            // Determine side with trend bias
            let is_buy = Decimal::from(rng.gen_range(0u32..100u32)) / dec!(100) < buy_probability;
            let side = if is_buy { Side::Bid } else { Side::Ask };

            // 85% limit, 15% market
            let is_market = rng.gen_range(0..100) < 15;

            // Retail-sized orders: 1-20 units
            let quantity = Self::round_quantity(Decimal::from(rng.gen_range(1u32..=20u32)));

            if is_market {
                orders.push(OrderRequest {
                    symbol: symbol.to_string(),
                    side,
                    order_type: OrderType::Market,
                    price: None,
                    quantity,
                });
            } else {
                // Limit order placement:
                // Buys: 0.15% below to 0.05% above market
                // Sells: 0.05% below to 0.15% above market
                let offset_pct = if is_buy {
                    Decimal::from(rng.gen_range(-15i32..=5i32)) / dec!(10000)
                } else {
                    Decimal::from(rng.gen_range(-5i32..=15i32)) / dec!(10000)
                };

                let price = Self::round_price(mid * (dec!(1) + offset_pct));

                if price > Decimal::ZERO {
                    orders.push(OrderRequest {
                        symbol: symbol.to_string(),
                        side,
                        order_type: OrderType::Limit,
                        price: Some(price),
                        quantity,
                    });
                }
            }
        }

        orders
    }
}

impl Default for Random {
    fn default() -> Self {
        Self::new()
    }
}
