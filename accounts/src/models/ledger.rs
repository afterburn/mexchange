use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "lowercase")]
pub enum EntryType {
    Deposit,
    Withdrawal,
    Trade,
    Fee,
    Lock,
    Unlock,
}

impl std::fmt::Display for EntryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntryType::Deposit => write!(f, "deposit"),
            EntryType::Withdrawal => write!(f, "withdrawal"),
            EntryType::Trade => write!(f, "trade"),
            EntryType::Fee => write!(f, "fee"),
            EntryType::Lock => write!(f, "lock"),
            EntryType::Unlock => write!(f, "unlock"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LedgerEntry {
    pub id: Uuid,
    pub user_id: Uuid,
    pub asset: String,
    pub amount: Decimal,
    pub balance_after: Decimal,
    pub entry_type: String,
    pub reference_id: Option<Uuid>,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl LedgerEntry {
    /// Validate asset string (1-10 alphanumeric characters)
    fn validate_asset(asset: &str) -> Result<(), sqlx::Error> {
        if asset.is_empty() || asset.len() > 10 {
            return Err(sqlx::Error::Protocol("Asset must be 1-10 characters".into()));
        }
        if !asset.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(sqlx::Error::Protocol("Asset must be alphanumeric".into()));
        }
        Ok(())
    }

    /// Get maximum decimal places for an asset
    fn max_decimals_for_asset(asset: &str) -> u32 {
        match asset {
            "EUR" | "USD" | "GBP" => 2,
            _ => 8, // Crypto assets get 8 decimals
        }
    }

    /// Validate that amount doesn't exceed asset's decimal precision
    fn validate_precision(asset: &str, amount: Decimal) -> Result<(), sqlx::Error> {
        let max_decimals = Self::max_decimals_for_asset(asset);
        let scale = amount.scale();
        if scale > max_decimals {
            return Err(sqlx::Error::Protocol(
                format!("{} amounts cannot have more than {} decimal places", asset, max_decimals).into()
            ));
        }
        Ok(())
    }

    /// Round amount to asset's precision
    pub fn round_to_precision(asset: &str, amount: Decimal) -> Decimal {
        let max_decimals = Self::max_decimals_for_asset(asset);
        amount.round_dp(max_decimals)
    }

    /// Append a new entry to the ledger and return the updated balance.
    /// This is the ONLY way to modify balances - all changes go through the ledger.
    pub async fn append(
        pool: &PgPool,
        user_id: Uuid,
        asset: &str,
        amount: Decimal,
        entry_type: EntryType,
        reference_id: Option<Uuid>,
        description: Option<&str>,
    ) -> Result<Self, sqlx::Error> {
        // Validate asset string
        Self::validate_asset(asset)?;
        // Validate decimal precision for the asset
        Self::validate_precision(asset, amount)?;

        // Use a transaction to ensure atomicity
        let mut tx = pool.begin().await?;

        // Acquire advisory lock on user_id + asset combination
        // This prevents concurrent modifications to the same user/asset balance
        // We use the first 8 bytes of user_id XOR'd with asset hash for the lock key
        let lock_key = Self::compute_lock_key(user_id, asset);
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(lock_key)
            .execute(&mut *tx)
            .await?;

        // Get current balance from the cached balance table (faster than scanning ledger)
        // The balances table is our authoritative cache, updated atomically with ledger
        let current_balance: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(
                (SELECT available FROM balances
                 WHERE user_id = $1 AND asset = $2),
                0
            )"
        )
        .bind(user_id)
        .bind(asset)
        .fetch_one(&mut *tx)
        .await?;

        let new_balance = current_balance + amount;

        // Prevent negative balances for available funds
        if new_balance < Decimal::ZERO && entry_type != EntryType::Lock {
            return Err(sqlx::Error::Protocol("Insufficient balance".into()));
        }

        // Insert the ledger entry
        let entry = sqlx::query_as::<_, Self>(
            "INSERT INTO ledger (user_id, asset, amount, balance_after, entry_type, reference_id, description)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING *"
        )
        .bind(user_id)
        .bind(asset)
        .bind(amount)
        .bind(new_balance)
        .bind(entry_type.to_string())
        .bind(reference_id)
        .bind(description)
        .fetch_one(&mut *tx)
        .await?;

        // Update the cached balance in the balances table
        sqlx::query(
            "INSERT INTO balances (user_id, asset, available)
             VALUES ($1, $2, $3)
             ON CONFLICT (user_id, asset) DO UPDATE SET
                available = $3,
                updated_at = NOW()"
        )
        .bind(user_id)
        .bind(asset)
        .bind(new_balance)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(entry)
    }

    /// Append a new entry within an existing transaction (for use in multi-step operations)
    pub async fn append_in_tx(
        tx: &mut Transaction<'_, Postgres>,
        user_id: Uuid,
        asset: &str,
        amount: Decimal,
        entry_type: EntryType,
        reference_id: Option<Uuid>,
        description: Option<&str>,
    ) -> Result<Self, sqlx::Error> {
        // Validate asset string
        Self::validate_asset(asset)?;
        // Validate decimal precision for the asset
        Self::validate_precision(asset, amount)?;

        // Acquire advisory lock
        let lock_key = Self::compute_lock_key(user_id, asset);
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(lock_key)
            .execute(&mut **tx)
            .await?;

        // Get current balance
        let current_balance: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(
                (SELECT available FROM balances
                 WHERE user_id = $1 AND asset = $2),
                0
            )"
        )
        .bind(user_id)
        .bind(asset)
        .fetch_one(&mut **tx)
        .await?;

        let new_balance = current_balance + amount;

        // Prevent negative balances (Lock entries can go negative temporarily)
        if new_balance < Decimal::ZERO && entry_type != EntryType::Lock {
            return Err(sqlx::Error::Protocol("Insufficient balance".into()));
        }

        // Insert the ledger entry
        let entry = sqlx::query_as::<_, Self>(
            "INSERT INTO ledger (user_id, asset, amount, balance_after, entry_type, reference_id, description)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING *"
        )
        .bind(user_id)
        .bind(asset)
        .bind(amount)
        .bind(new_balance)
        .bind(entry_type.to_string())
        .bind(reference_id)
        .bind(description)
        .fetch_one(&mut **tx)
        .await?;

        // Update the cached balance
        sqlx::query(
            "INSERT INTO balances (user_id, asset, available)
             VALUES ($1, $2, $3)
             ON CONFLICT (user_id, asset) DO UPDATE SET
                available = $3,
                updated_at = NOW()"
        )
        .bind(user_id)
        .bind(asset)
        .bind(new_balance)
        .execute(&mut **tx)
        .await?;

        Ok(entry)
    }

    /// Get ledger history for a user's asset
    pub async fn get_history(
        pool: &PgPool,
        user_id: Uuid,
        asset: &str,
        limit: i64,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            "SELECT * FROM ledger
             WHERE user_id = $1 AND asset = $2
             ORDER BY created_at DESC, id DESC
             LIMIT $3"
        )
        .bind(user_id)
        .bind(asset)
        .bind(limit)
        .fetch_all(pool)
        .await
    }

    /// Derive balance from ledger (for reconciliation)
    pub async fn derive_balance(
        pool: &PgPool,
        user_id: Uuid,
        asset: &str,
    ) -> Result<Decimal, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT COALESCE(SUM(amount), 0) FROM ledger WHERE user_id = $1 AND asset = $2"
        )
        .bind(user_id)
        .bind(asset)
        .fetch_one(pool)
        .await
    }

    /// Reconcile: check that cached balance matches derived balance
    pub async fn reconcile(
        pool: &PgPool,
        user_id: Uuid,
        asset: &str,
    ) -> Result<bool, sqlx::Error> {
        let derived = Self::derive_balance(pool, user_id, asset).await?;

        let cached: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(available, 0) FROM balances WHERE user_id = $1 AND asset = $2"
        )
        .bind(user_id)
        .bind(asset)
        .fetch_optional(pool)
        .await?
        .unwrap_or(Decimal::ZERO);

        Ok(derived == cached)
    }

    /// Compute a deterministic lock key for user_id + asset combination
    /// Uses XOR of UUID bytes with a hash of the asset string
    /// Public version for use in settlement
    pub fn compute_lock_key_public(user_id: Uuid, asset: &str) -> i64 {
        Self::compute_lock_key(user_id, asset)
    }

    /// Compute a deterministic lock key for user_id + asset combination
    /// Uses XOR of UUID bytes with a hash of the asset string
    fn compute_lock_key(user_id: Uuid, asset: &str) -> i64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let uuid_bytes = user_id.as_bytes();
        let uuid_part = i64::from_le_bytes([
            uuid_bytes[0], uuid_bytes[1], uuid_bytes[2], uuid_bytes[3],
            uuid_bytes[4], uuid_bytes[5], uuid_bytes[6], uuid_bytes[7],
        ]);

        let mut hasher = DefaultHasher::new();
        asset.hash(&mut hasher);
        let asset_hash = hasher.finish() as i64;

        uuid_part ^ asset_hash
    }
}
