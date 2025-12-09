//! Settlement client for calling the accounts service.
//!
//! This module handles synchronous settlement of trades before events are published.
//! If settlement fails, the matching engine can rollback the orderbook state.

use reqwest::Client;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Maximum number of retry attempts for settlement
const MAX_RETRIES: u32 = 3;
/// Base delay for exponential backoff (doubles each retry)
const BASE_RETRY_DELAY_MS: u64 = 50;
/// Timeout for settlement requests
const SETTLEMENT_TIMEOUT_MS: u64 = 5000;

/// Settlement client for calling the accounts service internal API
#[derive(Clone)]
pub struct SettlementClient {
    client: Client,
    accounts_url: String,
}

#[derive(Debug, Serialize)]
struct SettleFillRequest {
    symbol: String,
    buy_order_id: Uuid,
    sell_order_id: Uuid,
    price: Decimal,
    quantity: Decimal,
    timestamp: i64,
}

#[derive(Debug, Deserialize)]
pub struct SettleFillResponse {
    pub trade_id: String,
    pub buyer_id: String,
    pub seller_id: String,
    pub settled: bool,
}

#[derive(Debug, Deserialize)]
struct SettleErrorResponse {
    error: String,
    code: String,
}

/// Result of a settlement attempt
#[derive(Debug)]
pub enum SettlementResult {
    /// Settlement succeeded
    Success(SettleFillResponse),
    /// Settlement was skipped (both sides are anonymous/bot orders)
    Skipped,
    /// Settlement failed - should trigger rollback
    Failed(String),
}

#[derive(Debug, Serialize)]
struct CancelOrderRequest {
    order_id: Uuid,
    filled_quantity: Decimal,
}

#[derive(Debug, Deserialize)]
pub struct CancelOrderResponse {
    pub success: bool,
    pub order_id: String,
    pub status: String,
}

impl SettlementClient {
    pub fn new(accounts_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_millis(SETTLEMENT_TIMEOUT_MS))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            accounts_url,
        }
    }

    /// Settle a fill synchronously with retry logic.
    /// Returns SettlementResult indicating success, skip, or failure.
    ///
    /// This is called BEFORE publishing events. If settlement fails,
    /// the caller should rollback the orderbook state.
    pub async fn settle_fill(
        &self,
        symbol: &str,
        buy_order_id: Uuid,
        sell_order_id: Uuid,
        price: Decimal,
        quantity: Decimal,
        timestamp: u64,
    ) -> SettlementResult {
        let url = format!("{}/internal/settle", self.accounts_url);

        // Safe timestamp conversion
        let timestamp_i64 = if timestamp > i64::MAX as u64 {
            warn!("Timestamp {} exceeds i64::MAX, capping", timestamp);
            i64::MAX
        } else {
            timestamp as i64
        };

        let request = SettleFillRequest {
            symbol: symbol.to_string(),
            buy_order_id,
            sell_order_id,
            price,
            quantity,
            timestamp: timestamp_i64,
        };

        let mut attempts = 0u32;
        loop {
            attempts += 1;

            match self.client.post(&url).json(&request).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        match response.json::<SettleFillResponse>().await {
                            Ok(result) => {
                                info!(
                                    "Settled trade {}: {} buys from {} ({} {} @ {})",
                                    result.trade_id,
                                    result.buyer_id,
                                    result.seller_id,
                                    quantity,
                                    symbol,
                                    price
                                );
                                return SettlementResult::Success(result);
                            }
                            Err(e) => {
                                warn!("Failed to parse settlement response: {}", e);
                                return SettlementResult::Failed(format!("Parse error: {}", e));
                            }
                        }
                    } else {
                        let status = response.status();
                        match response.json::<SettleErrorResponse>().await {
                            Ok(err) => {
                                // Handle specific error codes
                                if err.code == "PARTIAL_SETTLEMENT" {
                                    // Both sides are anonymous - this is OK, just skip
                                    info!(
                                        "Settlement skipped (both sides anonymous): buy={}, sell={}",
                                        buy_order_id, sell_order_id
                                    );
                                    return SettlementResult::Skipped;
                                } else if err.code == "ORDER_NOT_FOUND" {
                                    // One side is anonymous - partial settlement happened
                                    info!(
                                        "Partial settlement (one side anonymous): buy={}, sell={} - {}",
                                        buy_order_id, sell_order_id, err.error
                                    );
                                    // This is still a success - the real user side was settled
                                    return SettlementResult::Skipped;
                                } else if err.code == "ALREADY_SETTLED" {
                                    // Idempotent - already processed
                                    info!(
                                        "Settlement already completed: buy={}, sell={}",
                                        buy_order_id, sell_order_id
                                    );
                                    return SettlementResult::Success(SettleFillResponse {
                                        trade_id: "already_settled".to_string(),
                                        buyer_id: String::new(),
                                        seller_id: String::new(),
                                        settled: true,
                                    });
                                } else if status.is_server_error() && attempts < MAX_RETRIES {
                                    // Server error - retry
                                    let delay = Duration::from_millis(BASE_RETRY_DELAY_MS * 2u64.pow(attempts - 1));
                                    warn!(
                                        "Settlement failed (attempt {}/{}), retrying in {:?}: {} ({})",
                                        attempts, MAX_RETRIES, delay, err.error, err.code
                                    );
                                    tokio::time::sleep(delay).await;
                                    continue;
                                } else {
                                    // Non-retriable error or max retries exceeded
                                    error!(
                                        "Settlement failed permanently for buy={}, sell={}: {} ({})",
                                        buy_order_id, sell_order_id, err.error, err.code
                                    );
                                    return SettlementResult::Failed(format!("{}: {}", err.code, err.error));
                                }
                            }
                            Err(_) => {
                                if status.is_server_error() && attempts < MAX_RETRIES {
                                    let delay = Duration::from_millis(BASE_RETRY_DELAY_MS * 2u64.pow(attempts - 1));
                                    warn!(
                                        "Settlement failed (attempt {}/{}), retrying in {:?}: status {}",
                                        attempts, MAX_RETRIES, delay, status
                                    );
                                    tokio::time::sleep(delay).await;
                                    continue;
                                }
                                error!(
                                    "Settlement failed permanently for buy={}, sell={}: status {}",
                                    buy_order_id, sell_order_id, status
                                );
                                return SettlementResult::Failed(format!("HTTP {}", status));
                            }
                        }
                    }
                }
                Err(e) => {
                    // Network error - retry if attempts remain
                    if attempts < MAX_RETRIES {
                        let delay = Duration::from_millis(BASE_RETRY_DELAY_MS * 2u64.pow(attempts - 1));
                        warn!(
                            "Settlement network error (attempt {}/{}), retrying in {:?}: {}",
                            attempts, MAX_RETRIES, delay, e
                        );
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    error!(
                        "Settlement failed permanently after {} attempts for buy={}, sell={}: {}",
                        attempts, buy_order_id, sell_order_id, e
                    );
                    return SettlementResult::Failed(format!("Network error: {}", e));
                }
            }
        }
    }

    /// Cancel an order in the accounts service.
    /// This is called when a market order cannot be fully filled.
    /// The accounts service will update the order status and unlock remaining funds.
    pub async fn cancel_order(&self, order_id: Uuid, filled_quantity: Decimal) -> bool {
        let url = format!("{}/internal/cancel", self.accounts_url);

        let request = CancelOrderRequest {
            order_id,
            filled_quantity,
        };

        info!(
            "Cancelling order {} with filled_quantity={}",
            order_id, filled_quantity
        );

        match self.client.post(&url).json(&request).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<CancelOrderResponse>().await {
                        Ok(result) => {
                            info!(
                                "Cancelled order {}: status={}",
                                result.order_id, result.status
                            );
                            true
                        }
                        Err(e) => {
                            warn!("Failed to parse cancel response: {}", e);
                            false
                        }
                    }
                } else {
                    let status = response.status();
                    match response.json::<SettleErrorResponse>().await {
                        Ok(err) => {
                            if err.code == "ORDER_NOT_FOUND" {
                                // Anonymous order - no need to cancel in accounts
                                info!("Cancel skipped (anonymous order): {}", order_id);
                                true
                            } else {
                                warn!(
                                    "Cancel failed for order {}: {} ({})",
                                    order_id, err.error, err.code
                                );
                                false
                            }
                        }
                        Err(_) => {
                            warn!("Cancel failed for order {}: status {}", order_id, status);
                            false
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Cancel network error for order {}: {}", order_id, e);
                false
            }
        }
    }
}
