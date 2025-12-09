use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// Cached balance derived from ledger.
/// This is a read-only view - all modifications go through LedgerEntry::append()
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Balance {
    pub id: Uuid,
    pub user_id: Uuid,
    pub asset: String,
    pub available: Decimal,
    pub locked: Decimal,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Balance {
    pub async fn get_for_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>("SELECT * FROM balances WHERE user_id = $1 ORDER BY asset")
            .bind(user_id)
            .fetch_all(pool)
            .await
    }

    pub async fn get(pool: &PgPool, user_id: Uuid, asset: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            "SELECT * FROM balances WHERE user_id = $1 AND asset = $2"
        )
        .bind(user_id)
        .bind(asset)
        .fetch_optional(pool)
        .await
    }

    /// Get balance or return zero balance if none exists
    pub async fn get_or_zero(pool: &PgPool, user_id: Uuid, asset: &str) -> Result<Decimal, sqlx::Error> {
        let balance = Self::get(pool, user_id, asset).await?;
        Ok(balance.map(|b| b.available).unwrap_or(Decimal::ZERO))
    }
}
