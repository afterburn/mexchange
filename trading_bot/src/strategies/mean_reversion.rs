use rand::Rng;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use super::Strategy;
use crate::indicators::z_score;
use crate::types::{MarketState, OrderRequest, OrderType, Side};

const MAX_POSITION: Decimal = dec!(500);

pub struct MeanReversion {
    position: Decimal,
}

impl MeanReversion {
    pub fn new() -> Self {
        Self {
            position: Decimal::ZERO,
        }
    }

    fn round_price(price: Decimal) -> Decimal {
        price.round_dp(2)
    }

    fn round_quantity(qty: Decimal) -> Decimal {
        qty.round_dp(2).max(dec!(0.01))
    }
}

impl Strategy for MeanReversion {
    fn name(&self) -> &'static str {
        "MeanReversion"
    }

    fn interval_ms(&self) -> u64 {
        600
    }

    fn generate_orders(&mut self, market: &MarketState, symbol: &str) -> Vec<OrderRequest> {
        if market.price_history.len() < 10 {
            return vec![];
        }

        let mid = match market.mid_price() {
            Some(p) => p,
            None => return vec![],
        };

        let z = z_score(&market.price_history).unwrap_or(Decimal::ZERO);
        let z_abs = z.abs();

        let mut orders = Vec::new();
        let mut rng = rand::thread_rng();

        // Price returned to mean - take profit on 30% of position
        if z_abs < dec!(0.5) && self.position.abs() > dec!(10) {
            let close_size = Self::round_quantity(self.position.abs() * dec!(0.3));
            let side = if self.position > Decimal::ZERO {
                Side::Ask
            } else {
                Side::Bid
            };

            orders.push(OrderRequest {
                symbol: symbol.to_string(),
                side,
                order_type: OrderType::Market,
                price: None,
                quantity: close_size,
            });

            if side == Side::Ask {
                self.position -= close_size;
            } else {
                self.position += close_size;
            }
            return orders;
        }

        // Determine if overbought or oversold
        let is_overbought = z > dec!(1.5);
        let is_oversold = z < dec!(-1.5);
        let is_extreme = z_abs > dec!(2.0);

        if !is_overbought && !is_oversold {
            return vec![];
        }

        // Size scales with deviation (capped at 3 std devs)
        let base_size = dec!(15) + Decimal::from(rng.gen_range(0u32..10u32));
        let z_multiplier = z_abs.min(dec!(3));
        let total_size = Self::round_quantity(base_size * z_multiplier);

        // Fade the move: sell when overbought, buy when oversold
        let side = if is_overbought { Side::Ask } else { Side::Bid };

        if is_extreme {
            // 30% chance of market order for immediate fill
            if rng.gen_range(0..100) < 30 {
                let market_size = Self::round_quantity(total_size * dec!(0.3));
                orders.push(OrderRequest {
                    symbol: symbol.to_string(),
                    side,
                    order_type: OrderType::Market,
                    price: None,
                    quantity: market_size,
                });
            }

            // 2-4 limit orders around current price
            let num_limits = rng.gen_range(2..=4);
            let limit_size_each = Self::round_quantity(total_size / Decimal::from(num_limits));

            for i in 0..num_limits {
                let offset = mid * dec!(0.0005) * Decimal::from(i + 1);
                let price = if is_overbought {
                    Self::round_price(mid + offset)
                } else {
                    Self::round_price(mid - offset)
                };

                if limit_size_each > dec!(0.01) && price > Decimal::ZERO {
                    orders.push(OrderRequest {
                        symbol: symbol.to_string(),
                        side,
                        order_type: OrderType::Limit,
                        price: Some(price),
                        quantity: limit_size_each,
                    });
                }
            }
        } else {
            // Mild conditions: 1-3 limit orders away from market
            let num_limits = rng.gen_range(1..=3);
            let limit_size_each = Self::round_quantity(total_size / Decimal::from(num_limits));

            for i in 0..num_limits {
                let offset = mid * dec!(0.001) * Decimal::from(i + 1);
                let price = if is_overbought {
                    Self::round_price(mid + offset)
                } else {
                    Self::round_price(mid - offset)
                };

                if limit_size_each > dec!(0.01) && price > Decimal::ZERO {
                    orders.push(OrderRequest {
                        symbol: symbol.to_string(),
                        side,
                        order_type: OrderType::Limit,
                        price: Some(price),
                        quantity: limit_size_each,
                    });
                }
            }
        }

        // Update position tracking
        let position_delta = if is_overbought { -total_size } else { total_size };
        self.position = (self.position + position_delta).clamp(-MAX_POSITION, MAX_POSITION);

        orders
    }
}

impl Default for MeanReversion {
    fn default() -> Self {
        Self::new()
    }
}
