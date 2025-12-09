use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use tracing::error;

const INTERVALS: &[(&str, i64)] = &[
    ("1m", 60),
    ("5m", 300),
    ("15m", 900),
    ("1h", 3600),
    ("4h", 14400),
    ("1d", 86400),
];

pub struct OhlcvAggregator {
    pool: PgPool,
}

impl OhlcvAggregator {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn process_trade(
        &self,
        symbol: &str,
        price: Decimal,
        quantity: Decimal,
        timestamp_ms: u64,
    ) -> anyhow::Result<()> {
        let timestamp = DateTime::from_timestamp_millis(timestamp_ms as i64)
            .unwrap_or_else(Utc::now);

        for (interval_name, interval_secs) in INTERVALS {
            let open_time = Self::truncate_to_interval(timestamp, *interval_secs);

            if let Err(e) = self.upsert_ohlcv(
                symbol,
                interval_name,
                open_time,
                price,
                quantity,
            ).await {
                error!("Failed to upsert OHLCV for {}/{}: {}", symbol, interval_name, e);
            }
        }

        Ok(())
    }

    fn truncate_to_interval(timestamp: DateTime<Utc>, interval_secs: i64) -> DateTime<Utc> {
        let ts_secs = timestamp.timestamp();
        let truncated_secs = (ts_secs / interval_secs) * interval_secs;
        DateTime::from_timestamp(truncated_secs, 0).unwrap_or(timestamp)
    }

    async fn upsert_ohlcv(
        &self,
        symbol: &str,
        interval: &str,
        open_time: DateTime<Utc>,
        price: Decimal,
        volume: Decimal,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO ohlcv (symbol, interval, open_time, open, high, low, close, volume, trade_count)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 1)
            ON CONFLICT (symbol, interval, open_time) DO UPDATE SET
                high = GREATEST(ohlcv.high, EXCLUDED.high),
                low = LEAST(ohlcv.low, EXCLUDED.low),
                close = EXCLUDED.close,
                volume = ohlcv.volume + EXCLUDED.volume,
                trade_count = ohlcv.trade_count + 1
            "#,
        )
        .bind(symbol)
        .bind(interval)
        .bind(open_time)
        .bind(price) // open
        .bind(price) // high
        .bind(price) // low
        .bind(price) // close
        .bind(volume)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
