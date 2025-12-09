use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::{EntryType, LedgerEntry};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Bid,
    Ask,
}

impl std::fmt::Display for Side {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Side::Bid => write!(f, "bid"),
            Side::Ask => write!(f, "ask"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderType {
    Limit,
    Market,
}

impl std::fmt::Display for OrderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderType::Limit => write!(f, "limit"),
            OrderType::Market => write!(f, "market"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Pending,
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
    Expired,
}

impl std::fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderStatus::Pending => write!(f, "pending"),
            OrderStatus::Open => write!(f, "open"),
            OrderStatus::PartiallyFilled => write!(f, "partially_filled"),
            OrderStatus::Filled => write!(f, "filled"),
            OrderStatus::Cancelled => write!(f, "cancelled"),
            OrderStatus::Rejected => write!(f, "rejected"),
            OrderStatus::Expired => write!(f, "expired"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Order {
    pub id: Uuid,
    pub user_id: Uuid,
    pub symbol: String,
    pub side: String,
    pub order_type: String,
    pub price: Option<Decimal>,
    pub quantity: Decimal,
    pub filled_quantity: Decimal,
    pub status: String,
    pub lock_entry_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct PlaceOrderRequest {
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    pub price: Option<Decimal>,
    /// Quantity of base asset. For quote currency orders, this is calculated from quote_amount.
    pub quantity: Decimal,
    /// For quote currency orders: the original quote amount requested (for reference).
    pub quote_amount: Option<Decimal>,
    /// For market buy orders: maximum price willing to pay (slippage protection).
    /// Used to calculate the amount of quote asset to lock.
    pub max_slippage_price: Option<Decimal>,
}

#[derive(Debug)]
pub struct PlaceOrderResult {
    pub order: Order,
    pub locked_asset: String,
    pub locked_amount: Decimal,
}

#[derive(Debug, thiserror::Error)]
pub enum OrderError {
    #[error("Insufficient balance: available {available}, required {required}")]
    InsufficientBalance { available: Decimal, required: Decimal },
    #[error("Limit order requires price")]
    LimitOrderRequiresPrice,
    #[error("Invalid symbol format")]
    InvalidSymbol,
    #[error("Order not found")]
    NotFound,
    #[error("Order cannot be cancelled (status: {0})")]
    CannotCancel(String),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl Order {
    /// Parse symbol into base and quote assets (e.g., "KCN/EUR" -> ("KCN", "EUR"))
    fn parse_symbol(symbol: &str) -> Result<(&str, &str), OrderError> {
        let parts: Vec<&str> = symbol.split('/').collect();
        if parts.len() != 2 {
            return Err(OrderError::InvalidSymbol);
        }
        Ok((parts[0], parts[1]))
    }

    // Fallback price if no market data available (conservative estimate)
    const FALLBACK_PRICE: i64 = 1000;

    /// Calculate the amount to lock based on order side
    fn calculate_lock_amount(req: &PlaceOrderRequest) -> Result<(String, Decimal), OrderError> {
        let (base, quote) = Self::parse_symbol(&req.symbol)?;

        match req.side {
            Side::Bid => {
                // Buying base asset: lock quote asset (e.g., buying KCN, lock EUR)
                let lock_amount = match req.order_type {
                    OrderType::Limit => {
                        let price = req.price.ok_or(OrderError::LimitOrderRequiresPrice)?;
                        price * req.quantity
                    }
                    OrderType::Market => {
                        // For market buy orders, use max_slippage_price if provided.
                        // This is the industry standard approach where the client provides
                        // the best ask price plus a slippage buffer (typically 2-5%).
                        // If not provided, fall back to a conservative estimate.
                        let lock_price = req.max_slippage_price.unwrap_or_else(|| {
                            Decimal::from(Self::FALLBACK_PRICE)
                        });
                        lock_price * req.quantity
                    }
                };
                // Round to quote asset precision (e.g., EUR = 2 decimals)
                let rounded = LedgerEntry::round_to_precision(&quote, lock_amount);
                Ok((quote.to_string(), rounded))
            }
            Side::Ask => {
                // Selling base asset: lock base asset (e.g., selling KCN, lock KCN)
                // Round to base asset precision (e.g., KCN = 8 decimals)
                let rounded = LedgerEntry::round_to_precision(&base, req.quantity);
                Ok((base.to_string(), rounded))
            }
        }
    }

    /// Place a new order with balance locking
    pub async fn place(
        pool: &PgPool,
        user_id: Uuid,
        req: PlaceOrderRequest,
    ) -> Result<PlaceOrderResult, OrderError> {
        // Validate limit orders have price
        if req.order_type == OrderType::Limit && req.price.is_none() {
            return Err(OrderError::LimitOrderRequiresPrice);
        }

        // Calculate lock amount
        let (lock_asset, lock_amount) = Self::calculate_lock_amount(&req)?;

        // For market buy orders, store the slippage price so we can unlock correctly on cancel
        // For limit orders, use the actual price
        // For market sell orders, price is not needed (we lock the base asset, not quote)
        let stored_price = match (req.order_type, req.side) {
            (OrderType::Limit, _) => req.price,
            (OrderType::Market, Side::Bid) => req.max_slippage_price.or(Some(Decimal::from(Self::FALLBACK_PRICE))),
            (OrderType::Market, Side::Ask) => None,
        };

        // Pre-generate order ID so we can reference it in the ledger entry
        // (ledger is immutable, so we must know the order ID before creating the lock entry)
        let order_id = Uuid::new_v4();

        // Start transaction
        let mut tx = pool.begin().await?;

        // Lock funds via ledger (this checks balance atomically)
        let lock_entry = LedgerEntry::append_in_tx(
            &mut tx,
            user_id,
            &lock_asset,
            -lock_amount, // Negative to reduce available
            EntryType::Lock,
            Some(order_id), // Reference the order we're about to create
            Some(&format!("Order lock: {} {} @ {:?}", req.side, req.quantity, stored_price)),
        )
        .await
        .map_err(|e| {
            let msg = e.to_string();
            // Check for insufficient balance - either from our custom error or the DB constraint
            if msg.contains("Insufficient balance") || msg.contains("chk_available_non_negative") {
                OrderError::InsufficientBalance {
                    available: Decimal::ZERO,
                    required: lock_amount,
                }
            } else {
                OrderError::Database(e)
            }
        })?;

        // Create order record with pre-generated ID
        let order = sqlx::query_as::<_, Order>(
            "INSERT INTO orders (id, user_id, symbol, side, order_type, price, quantity, status, lock_entry_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7, 'pending', $8)
             RETURNING *"
        )
        .bind(order_id)
        .bind(user_id)
        .bind(&req.symbol)
        .bind(req.side.to_string())
        .bind(req.order_type.to_string())
        .bind(stored_price)
        .bind(req.quantity)
        .bind(lock_entry.id)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(PlaceOrderResult {
            order,
            locked_asset: lock_asset,
            locked_amount: lock_amount,
        })
    }

    /// Cancel an order and release locked funds
    pub async fn cancel(pool: &PgPool, user_id: Uuid, order_id: Uuid) -> Result<Order, OrderError> {
        let mut tx = pool.begin().await?;

        // Get order with lock
        let order = sqlx::query_as::<_, Order>(
            "SELECT * FROM orders WHERE id = $1 AND user_id = $2 FOR UPDATE"
        )
        .bind(order_id)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(OrderError::NotFound)?;

        // Check if cancellable
        match order.status.as_str() {
            "pending" | "open" | "partially_filled" => {}
            status => return Err(OrderError::CannotCancel(status.to_string())),
        }

        // Calculate remaining locked amount
        // For market orders, we use the stored price (which was set when the order was placed)
        // or the fallback price if it was a market order without slippage price
        let (lock_asset, total_lock) = Self::calculate_lock_amount(&PlaceOrderRequest {
            symbol: order.symbol.clone(),
            side: if order.side == "bid" { Side::Bid } else { Side::Ask },
            order_type: if order.order_type == "limit" { OrderType::Limit } else { OrderType::Market },
            price: order.price,
            quantity: order.quantity,
            quote_amount: None,
            // For cancellation, use the order's price as max_slippage_price
            // This ensures we unlock the same amount that was locked
            max_slippage_price: order.price,
        })?;

        // Calculate how much is still locked (unfilled portion)
        // Round to asset precision to avoid validation errors
        let filled_ratio = order.filled_quantity / order.quantity;
        let unlock_amount = LedgerEntry::round_to_precision(&lock_asset, total_lock * (Decimal::ONE - filled_ratio));

        // Unlock remaining funds
        if unlock_amount > Decimal::ZERO {
            LedgerEntry::append_in_tx(
                &mut tx,
                user_id,
                &lock_asset,
                unlock_amount,
                EntryType::Unlock,
                Some(order_id),
                Some("Order cancelled - unlock remaining"),
            )
            .await?;
        }

        // Update order status
        let updated = sqlx::query_as::<_, Order>(
            "UPDATE orders SET status = 'cancelled', updated_at = NOW()
             WHERE id = $1 RETURNING *"
        )
        .bind(order_id)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(updated)
    }

    /// Cancel an order internally (called by gateway/matching engine, not by users)
    /// This is used when the matching engine determines an order can't be filled
    /// (e.g., unfilled portion of market orders).
    ///
    /// # Arguments
    /// * `pool` - Database connection pool
    /// * `order_id` - The client order ID (UUID)
    /// * `filled_quantity` - How much of the order was filled before cancellation
    pub async fn cancel_internal(
        pool: &PgPool,
        order_id: Uuid,
        filled_quantity: Decimal,
    ) -> Result<Order, OrderError> {
        let mut tx = pool.begin().await?;

        // Get order with lock (no user_id check - internal call)
        let order = sqlx::query_as::<_, Order>(
            "SELECT * FROM orders WHERE id = $1 FOR UPDATE"
        )
        .bind(order_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(OrderError::NotFound)?;

        // Check if cancellable
        match order.status.as_str() {
            "pending" | "open" | "partially_filled" => {}
            status => return Err(OrderError::CannotCancel(status.to_string())),
        }

        // Calculate remaining locked amount based on what was actually filled
        let (lock_asset, total_lock) = Self::calculate_lock_amount(&PlaceOrderRequest {
            symbol: order.symbol.clone(),
            side: if order.side == "bid" { Side::Bid } else { Side::Ask },
            order_type: if order.order_type == "limit" { OrderType::Limit } else { OrderType::Market },
            price: order.price,
            quantity: order.quantity,
            quote_amount: None,
            max_slippage_price: order.price,
        })?;

        // Calculate how much is still locked (unfilled portion)
        // Round to asset precision to avoid validation errors
        let filled_ratio = filled_quantity / order.quantity;
        let unlock_amount = LedgerEntry::round_to_precision(&lock_asset, total_lock * (Decimal::ONE - filled_ratio));

        tracing::info!(
            "Internal cancel order {}: filled {} of {}, unlocking {} {}",
            order_id, filled_quantity, order.quantity, unlock_amount, lock_asset
        );

        // Unlock remaining funds
        if unlock_amount > Decimal::ZERO {
            LedgerEntry::append_in_tx(
                &mut tx,
                order.user_id,
                &lock_asset,
                unlock_amount,
                EntryType::Unlock,
                Some(order_id),
                Some("Order cancelled by matching engine - unlock remaining"),
            )
            .await?;
        }

        // Update order status and filled_quantity
        let updated = sqlx::query_as::<_, Order>(
            "UPDATE orders SET status = 'cancelled', filled_quantity = $1, updated_at = NOW()
             WHERE id = $2 RETURNING *"
        )
        .bind(filled_quantity)
        .bind(order_id)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(updated)
    }

    /// Get order by ID (for user queries - requires user_id for authorization)
    pub async fn get(pool: &PgPool, user_id: Uuid, order_id: Uuid) -> Result<Option<Order>, sqlx::Error> {
        sqlx::query_as::<_, Order>(
            "SELECT * FROM orders WHERE id = $1 AND user_id = $2"
        )
        .bind(order_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
    }

    /// Get order by ID (internal use - no user authorization check)
    pub async fn get_by_id(pool: &PgPool, order_id: Uuid) -> Result<Option<Order>, sqlx::Error> {
        sqlx::query_as::<_, Order>(
            "SELECT * FROM orders WHERE id = $1"
        )
        .bind(order_id)
        .fetch_optional(pool)
        .await
    }

    /// List orders for user with pagination
    pub async fn list_for_user(
        pool: &PgPool,
        user_id: Uuid,
        status_filter: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Order>, sqlx::Error> {
        if let Some(status) = status_filter {
            sqlx::query_as::<_, Order>(
                "SELECT * FROM orders WHERE user_id = $1 AND status = $2
                 ORDER BY created_at DESC LIMIT $3 OFFSET $4"
            )
            .bind(user_id)
            .bind(status)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
        } else {
            sqlx::query_as::<_, Order>(
                "SELECT * FROM orders WHERE user_id = $1
                 ORDER BY created_at DESC LIMIT $2 OFFSET $3"
            )
            .bind(user_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
        }
    }

    /// Count total orders for user
    pub async fn count_for_user(
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<i64, sqlx::Error> {
        let result: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM orders WHERE user_id = $1"
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        Ok(result.0)
    }

    /// Update order fill quantity
    /// Note: Does not update cancelled orders - if a cancel event races ahead of settlement,
    /// the order stays cancelled and this fill is a no-op for status/quantity.
    pub async fn add_fill(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        order_id: Uuid,
        fill_quantity: Decimal,
    ) -> Result<Order, sqlx::Error> {
        sqlx::query_as::<_, Order>(
            "UPDATE orders SET
                filled_quantity = CASE
                    WHEN status = 'cancelled' THEN filled_quantity
                    ELSE filled_quantity + $1
                END,
                status = CASE
                    WHEN status = 'cancelled' THEN 'cancelled'
                    WHEN filled_quantity + $1 >= quantity THEN 'filled'
                    ELSE 'partially_filled'
                END,
                updated_at = NOW()
             WHERE id = $2
             RETURNING *"
        )
        .bind(fill_quantity)
        .bind(order_id)
        .fetch_one(&mut **tx)
        .await
    }
}
