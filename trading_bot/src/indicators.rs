use rust_decimal::Decimal;
use rust_decimal_macros::dec;

/// Simple Moving Average
pub fn sma(prices: &[Decimal]) -> Option<Decimal> {
    if prices.is_empty() {
        return None;
    }
    let sum: Decimal = prices.iter().sum();
    Some(sum / Decimal::from(prices.len()))
}

/// Standard Deviation
pub fn std_dev(prices: &[Decimal]) -> Option<Decimal> {
    let mean = sma(prices)?;
    if prices.len() < 2 {
        return None;
    }

    let variance: Decimal = prices
        .iter()
        .map(|p| {
            let diff = *p - mean;
            diff * diff
        })
        .sum::<Decimal>()
        / Decimal::from(prices.len());

    // Approximate square root using Newton's method
    sqrt_decimal(variance)
}

/// Approximate square root for Decimal using Newton's method
fn sqrt_decimal(value: Decimal) -> Option<Decimal> {
    if value < Decimal::ZERO {
        return None;
    }
    if value == Decimal::ZERO {
        return Some(Decimal::ZERO);
    }

    let mut guess = value / dec!(2);
    for _ in 0..20 {
        let new_guess = (guess + value / guess) / dec!(2);
        if (new_guess - guess).abs() < dec!(0.0000001) {
            return Some(new_guess);
        }
        guess = new_guess;
    }
    Some(guess)
}

/// Relative Strength Index (0-100)
pub fn rsi(prices: &[Decimal]) -> Option<Decimal> {
    if prices.len() < 2 {
        return None;
    }

    let mut gains = Decimal::ZERO;
    let mut losses = Decimal::ZERO;
    let mut gain_count = 0u32;
    let mut loss_count = 0u32;

    for i in 1..prices.len() {
        let change = prices[i] - prices[i - 1];
        if change > Decimal::ZERO {
            gains += change;
            gain_count += 1;
        } else if change < Decimal::ZERO {
            losses += change.abs();
            loss_count += 1;
        }
    }

    let avg_gain = if gain_count > 0 {
        gains / Decimal::from(gain_count)
    } else {
        Decimal::ZERO
    };

    let avg_loss = if loss_count > 0 {
        losses / Decimal::from(loss_count)
    } else {
        Decimal::ZERO
    };

    if avg_loss == Decimal::ZERO {
        return Some(dec!(100));
    }

    let rs = avg_gain / avg_loss;
    Some(dec!(100) - (dec!(100) / (Decimal::ONE + rs)))
}

/// Z-Score (standard deviations from mean)
pub fn z_score(prices: &[Decimal]) -> Option<Decimal> {
    if prices.is_empty() {
        return None;
    }

    let mean = sma(prices)?;
    let std = std_dev(prices)?;

    if std == Decimal::ZERO {
        return Some(Decimal::ZERO);
    }

    let current = *prices.last()?;
    Some((current - mean) / std)
}

/// Calculate trend as percentage change between recent and older moving averages
pub fn trend(prices: &[Decimal], recent_period: usize, lookback: usize) -> Option<Decimal> {
    if prices.len() < recent_period + lookback {
        return None;
    }

    let recent_start = prices.len() - recent_period;
    let older_start = prices.len() - recent_period - lookback;
    let older_end = prices.len() - recent_period;

    let recent_avg = sma(&prices[recent_start..])?;
    let older_avg = sma(&prices[older_start..older_end])?;

    if older_avg == Decimal::ZERO {
        return None;
    }

    Some((recent_avg - older_avg) / older_avg * dec!(100))
}

/// Volatility as percentage of price (clamped 0.5% - 5%)
pub fn volatility(prices: &[Decimal]) -> Option<Decimal> {
    let std = std_dev(prices)?;
    let mean = sma(prices)?;

    if mean == Decimal::ZERO {
        return None;
    }

    let vol_pct = (std / mean).abs();
    let min_vol = dec!(0.005);
    let max_vol = dec!(0.05);

    Some(vol_pct.max(min_vol).min(max_vol))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sma() {
        let prices = vec![dec!(10), dec!(20), dec!(30)];
        assert_eq!(sma(&prices), Some(dec!(20)));
    }

    #[test]
    fn test_rsi_all_gains() {
        let prices = vec![dec!(10), dec!(11), dec!(12), dec!(13), dec!(14)];
        let result = rsi(&prices).unwrap();
        assert_eq!(result, dec!(100));
    }

    #[test]
    fn test_z_score() {
        let prices = vec![dec!(10), dec!(10), dec!(10), dec!(10), dec!(10)];
        let result = z_score(&prices).unwrap();
        assert_eq!(result, dec!(0));
    }
}
