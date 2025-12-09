use rand::Rng;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use super::Strategy;
use crate::indicators::{rsi, trend};
use crate::types::{MarketState, OrderRequest, OrderType, Side};

const MAX_POSITION: Decimal = dec!(1000);
const TREND_THRESHOLD: Decimal = dec!(0.3);

pub struct Aggressive {
    position: Decimal,
}

impl Aggressive {
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

impl Strategy for Aggressive {
    fn name(&self) -> &'static str {
        "Aggressive"
    }

    fn interval_ms(&self) -> u64 {
        800
    }

    fn generate_orders(&mut self, market: &MarketState, symbol: &str) -> Vec<OrderRequest> {
        if market.price_history.len() < 10 {
            return vec![];
        }

        let mid = match market.mid_price() {
            Some(p) => p,
            None => return vec![],
        };

        let mut orders = Vec::new();
        let mut rng = rand::thread_rng();

        // Calculate trend (5 recent vs 5 older candles)
        let current_trend = trend(&market.price_history, 5, 5).unwrap_or(Decimal::ZERO);
        let current_rsi = rsi(&market.price_history).unwrap_or(dec!(50));

        let trend_abs = current_trend.abs();

        // Only trade if trend exceeds threshold
        if trend_abs < TREND_THRESHOLD {
            // Trend disappeared - close 20% of position at market
            if self.position.abs() > dec!(10) {
                let close_size = Self::round_quantity(self.position.abs() * dec!(0.2));
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
            }
            return orders;
        }

        let is_bullish = current_trend > Decimal::ZERO;

        // RSI confirmation - don't buy overbought, don't sell oversold
        if is_bullish && current_rsi > dec!(75) {
            return vec![];
        }
        if !is_bullish && current_rsi < dec!(25) {
            return vec![];
        }

        // Position sizing scales with trend strength
        let base_size = dec!(20) + Decimal::from(rng.gen_range(0u32..30u32));
        let trend_multiplier = dec!(1) + (trend_abs / dec!(100)) * dec!(15);
        let total_size = Self::round_quantity(base_size * trend_multiplier);

        let side = if is_bullish { Side::Bid } else { Side::Ask };

        // 40% market orders to chase
        let market_size = Self::round_quantity(total_size * dec!(0.4));
        if market_size > dec!(0.01) {
            orders.push(OrderRequest {
                symbol: symbol.to_string(),
                side,
                order_type: OrderType::Market,
                price: None,
                quantity: market_size,
            });
        }

        // Multiple limit orders just past the market
        let num_limits = rng.gen_range(2..=4);
        let limit_size_each = Self::round_quantity((total_size - market_size) / Decimal::from(num_limits));

        for i in 0..num_limits {
            let offset = mid * dec!(0.001) * Decimal::from(i + 1);
            let price = if is_bullish {
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

        // Update position tracking
        let position_delta = if is_bullish { total_size } else { -total_size };
        self.position = (self.position + position_delta).clamp(-MAX_POSITION, MAX_POSITION);

        orders
    }
}

impl Default for Aggressive {
    fn default() -> Self {
        Self::new()
    }
}
