use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::{EntryType, LedgerEntry, Order};

/// Trading fee rate (0.1% = 0.001)
/// TODO: Make this configurable per user/tier
const FEE_RATE: Decimal = dec!(0.001);

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Trade {
    pub id: Uuid,
    pub symbol: String,
    /// Buy order ID - None for bot orders
    pub buy_order_id: Option<Uuid>,
    /// Sell order ID - None for bot orders
    pub sell_order_id: Option<Uuid>,
    /// Buyer user ID - None for bot orders
    pub buyer_id: Option<Uuid>,
    /// Seller user ID - None for bot orders
    pub seller_id: Option<Uuid>,
    pub price: Decimal,
    pub quantity: Decimal,
    pub buyer_fee: Decimal,
    pub seller_fee: Decimal,
    pub exchange_fill_id: Option<String>,
    pub settled_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct Fill {
    /// Order ID (UUID) for the buy side
    /// If this UUID is not found in the database, the buy side is treated as an anonymous/bot order
    pub buy_order_id: Uuid,
    /// Order ID (UUID) for the sell side
    /// If this UUID is not found in the database, the sell side is treated as an anonymous/bot order
    pub sell_order_id: Uuid,
    pub price: Decimal,
    pub quantity: Decimal,
    pub timestamp: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum SettlementError {
    #[error("Order not found: {0}")]
    OrderNotFound(Uuid),
    #[error("Trade already settled: {0}")]
    AlreadySettled(String),
    #[error("Partial settlement: {0}")]
    PartialSettlement(String),
    #[error("Invalid symbol format: {0}")]
    InvalidSymbol(String),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl Trade {
    /// Check if a trade with this fill ID already exists (idempotency)
    pub async fn exists_by_fill_id(pool: &PgPool, fill_id: &str) -> Result<bool, sqlx::Error> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM trades WHERE exchange_fill_id = $1"
        )
        .bind(fill_id)
        .fetch_one(pool)
        .await?;
        Ok(count > 0)
    }

    /// Get trade by fill ID
    pub async fn get_by_fill_id(pool: &PgPool, fill_id: &str) -> Result<Option<Trade>, sqlx::Error> {
        sqlx::query_as::<_, Trade>(
            "SELECT * FROM trades WHERE exchange_fill_id = $1"
        )
        .bind(fill_id)
        .fetch_optional(pool)
        .await
    }

    /// Settle a trade atomically
    /// This handles:
    /// 1. Unlocking funds from both orders
    /// 2. Transferring assets between buyer and seller
    /// 3. Recording the trade
    /// 4. Updating order fill quantities
    ///
    /// Supports partial settlement where one or both sides are anonymous/bot orders.
    /// If an order UUID is not found in the database, that side is treated as anonymous.
    pub async fn settle(pool: &PgPool, symbol: &str, fill: &Fill) -> Result<Trade, SettlementError> {
        // Generate idempotency key based on order IDs and timestamp
        let fill_id = format!("{}-{}-{}", fill.buy_order_id, fill.sell_order_id, fill.timestamp);

        // Quick idempotency check outside transaction for performance
        // The actual guarantee comes from the unique constraint inside the transaction
        if let Some(existing) = Self::get_by_fill_id(pool, &fill_id).await? {
            return Ok(existing);
        }

        // Look up orders - orders not found are treated as anonymous/bot orders
        let buy_order = Order::get_by_id(pool, fill.buy_order_id).await?;
        let sell_order = Order::get_by_id(pool, fill.sell_order_id).await?;

        // At least one order must exist for settlement to proceed
        if buy_order.is_none() && sell_order.is_none() {
            return Err(SettlementError::PartialSettlement(
                format!("Neither order found: buy={}, sell={}", fill.buy_order_id, fill.sell_order_id)
            ));
        }

        // Extract user IDs - None for bot orders
        let buyer_id = buy_order.as_ref().map(|o| o.user_id);
        let seller_id = sell_order.as_ref().map(|o| o.user_id);

        // Parse symbol for asset names
        let parts: Vec<&str> = symbol.split('/').collect();
        if parts.len() != 2 {
            return Err(SettlementError::InvalidSymbol(symbol.to_string()));
        }
        let base_asset = parts[0]; // KCN
        let quote_asset = parts[1]; // EUR

        // Round quote amount to asset precision (EUR = 2 decimals)
        let quote_amount = LedgerEntry::round_to_precision(quote_asset, fill.price * fill.quantity);

        // Begin transaction with default READ COMMITTED isolation
        // Advisory locks handle concurrency - SERIALIZABLE is not needed
        let mut tx = pool.begin().await?;

        // Acquire locks in deterministic order to prevent deadlocks
        // Lock by user_id ordering, then by asset
        // Collect existing user IDs for locking
        let mut user_ids: Vec<Uuid> = vec![];
        if let Some(id) = buyer_id {
            user_ids.push(id);
        }
        if let Some(id) = seller_id {
            if !user_ids.contains(&id) {
                user_ids.push(id);
            }
        }
        user_ids.sort();

        // Lock each user's assets in deterministic order
        for user_id in &user_ids {
            for asset in [base_asset, quote_asset] {
                let lock_key = LedgerEntry::compute_lock_key_public(*user_id, asset);
                sqlx::query("SELECT pg_advisory_xact_lock($1)")
                    .bind(lock_key)
                    .execute(&mut *tx)
                    .await?;
            }
        }

        // === BUYER SIDE (only if real user order) ===
        if let Some(ref buy_ord) = buy_order {
            // buyer_id is guaranteed Some when buy_order exists
            let buyer = buyer_id.unwrap();

            // 1. Unlock EUR (the amount that was locked for this fill)
            LedgerEntry::append_in_tx(
                &mut tx,
                buyer,
                quote_asset,
                quote_amount, // Positive: increase available
                EntryType::Unlock,
                Some(buy_ord.id),
                Some(&format!("Trade fill unlock: buy {} {}", fill.quantity, base_asset)),
            )
            .await?;

            // 2. Debit EUR (pay for the trade)
            LedgerEntry::append_in_tx(
                &mut tx,
                buyer,
                quote_asset,
                -quote_amount, // Negative: decrease available
                EntryType::Trade,
                Some(buy_ord.id),
                Some(&format!("Buy {} {} @ {}", fill.quantity, base_asset, fill.price)),
            )
            .await?;

            // 3. Credit KCN (receive the asset)
            LedgerEntry::append_in_tx(
                &mut tx,
                buyer,
                base_asset,
                fill.quantity, // Positive: increase available
                EntryType::Trade,
                Some(buy_ord.id),
                Some(&format!("Receive {} {}", fill.quantity, base_asset)),
            )
            .await?;

            // Update order fill quantity
            Order::add_fill(&mut tx, buy_ord.id, fill.quantity).await?;
        }

        // === SELLER SIDE (only if real user order) ===
        if let Some(ref sell_ord) = sell_order {
            // seller_id is guaranteed Some when sell_order exists
            let seller = seller_id.unwrap();

            // 1. Unlock KCN (the amount that was locked for this fill)
            LedgerEntry::append_in_tx(
                &mut tx,
                seller,
                base_asset,
                fill.quantity, // Positive: increase available
                EntryType::Unlock,
                Some(sell_ord.id),
                Some(&format!("Trade fill unlock: sell {} {}", fill.quantity, base_asset)),
            )
            .await?;

            // 2. Debit KCN (transfer the asset)
            LedgerEntry::append_in_tx(
                &mut tx,
                seller,
                base_asset,
                -fill.quantity, // Negative: decrease available
                EntryType::Trade,
                Some(sell_ord.id),
                Some(&format!("Sell {} {} @ {}", fill.quantity, base_asset, fill.price)),
            )
            .await?;

            // 3. Credit EUR (receive payment)
            LedgerEntry::append_in_tx(
                &mut tx,
                seller,
                quote_asset,
                quote_amount, // Positive: increase available
                EntryType::Trade,
                Some(sell_ord.id),
                Some(&format!("Receive {} {}", quote_amount, quote_asset)),
            )
            .await?;

            // Update order fill quantity
            Order::add_fill(&mut tx, sell_ord.id, fill.quantity).await?;
        }

        // Record the trade with ON CONFLICT for idempotency
        // This handles race conditions where the same fill is processed concurrently
        let buy_order_id = buy_order.as_ref().map(|o| o.id);
        let sell_order_id = sell_order.as_ref().map(|o| o.id);

        // Calculate fees (0.1% of trade value for each side), rounded to quote asset precision
        let buyer_fee = LedgerEntry::round_to_precision(quote_asset, quote_amount * FEE_RATE);
        let seller_fee = LedgerEntry::round_to_precision(quote_asset, quote_amount * FEE_RATE);

        let trade = sqlx::query_as::<_, Trade>(
            "INSERT INTO trades (symbol, buy_order_id, sell_order_id, buyer_id, seller_id, price, quantity, buyer_fee, seller_fee, exchange_fill_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             ON CONFLICT (exchange_fill_id) DO UPDATE SET id = trades.id
             RETURNING *"
        )
        .bind(symbol)
        .bind(buy_order_id)
        .bind(sell_order_id)
        .bind(buyer_id)
        .bind(seller_id)
        .bind(fill.price)
        .bind(fill.quantity)
        .bind(buyer_fee)
        .bind(seller_fee)
        .bind(&fill_id)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(trade)
    }

    /// Get trades for a user (as buyer or seller) with pagination
    pub async fn list_for_user(
        pool: &PgPool,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Trade>, sqlx::Error> {
        sqlx::query_as::<_, Trade>(
            "SELECT * FROM trades
             WHERE buyer_id = $1 OR seller_id = $1
             ORDER BY settled_at DESC
             LIMIT $2 OFFSET $3"
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
    }

    /// Count total trades for a user
    pub async fn count_for_user(
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<i64, sqlx::Error> {
        let result: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM trades WHERE buyer_id = $1 OR seller_id = $1"
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        Ok(result.0)
    }

    /// Get fills (trades) for a specific order
    pub async fn list_for_order(
        pool: &PgPool,
        order_id: Uuid,
    ) -> Result<Vec<Trade>, sqlx::Error> {
        sqlx::query_as::<_, Trade>(
            "SELECT * FROM trades
             WHERE buy_order_id = $1 OR sell_order_id = $1
             ORDER BY settled_at ASC"
        )
        .bind(order_id)
        .fetch_all(pool)
        .await
    }
}
