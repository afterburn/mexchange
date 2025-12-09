use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post, delete},
    Extension, Json, Router,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::{Order, OrderError, OrderType, PlaceOrderRequest, Side, Trade, User};
use crate::AppState;

// === Request/Response Types ===

#[derive(Debug, Deserialize)]
pub struct PlaceOrderHttpRequest {
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    pub price: Option<Decimal>,
    /// Quantity of base asset to buy/sell.
    /// For quote currency orders (e.g., "spend 1000 EUR"), set this to None and use quote_amount instead.
    pub quantity: Option<Decimal>,
    /// For quote currency orders: amount of quote asset to spend (e.g., 1000 EUR).
    /// Backend calculates the quantity based on this and the market price.
    /// Only valid for market buy orders.
    pub quote_amount: Option<Decimal>,
    /// For market buy orders: maximum price willing to pay (slippage protection).
    /// Used to calculate the amount of quote asset to lock.
    /// If not provided for market buys, uses a conservative fallback.
    pub max_slippage_price: Option<Decimal>,
}

#[derive(Debug, Serialize)]
pub struct OrderResponse {
    pub id: String,
    pub symbol: String,
    pub side: String,
    pub order_type: String,
    pub price: Option<String>,
    pub quantity: String,
    pub filled_quantity: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct PlaceOrderResponse {
    pub order: OrderResponse,
    pub locked_asset: String,
    pub locked_amount: String,
}

#[derive(Debug, Serialize)]
pub struct CancelOrderResponse {
    pub order: OrderResponse,
    pub unlocked_asset: Option<String>,
    pub unlocked_amount: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OrdersListQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

#[derive(Debug, Serialize)]
pub struct OrdersListResponse {
    pub orders: Vec<OrderResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Serialize)]
pub struct TradeResponse {
    pub id: String,
    pub symbol: String,
    pub side: String,
    pub price: String,
    pub quantity: String,
    pub total: String,
    pub fee: String,
    pub settled_at: String,
}

#[derive(Debug, Deserialize)]
pub struct TradesListQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    20
}

#[derive(Debug, Serialize)]
pub struct TradesListResponse {
    pub trades: Vec<TradeResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<String>,
}

// === Helper Functions ===

fn order_to_response(order: &Order) -> OrderResponse {
    OrderResponse {
        id: order.id.to_string(),
        symbol: order.symbol.clone(),
        side: order.side.clone(),
        order_type: order.order_type.clone(),
        price: order.price.map(|p| p.to_string()),
        quantity: order.quantity.to_string(),
        filled_quantity: order.filled_quantity.to_string(),
        status: order.status.clone(),
        created_at: order.created_at.to_rfc3339(),
    }
}

fn trade_to_response(trade: &Trade, user_id: Uuid) -> TradeResponse {
    let is_buyer = trade.buyer_id == Some(user_id);
    let fee = if is_buyer { trade.buyer_fee } else { trade.seller_fee };
    let total = trade.price * trade.quantity;

    TradeResponse {
        id: trade.id.to_string(),
        symbol: trade.symbol.clone(),
        side: if is_buyer { "buy".to_string() } else { "sell".to_string() },
        price: trade.price.to_string(),
        quantity: trade.quantity.to_string(),
        total: total.to_string(),
        fee: fee.to_string(),
        settled_at: trade.settled_at.to_rfc3339(),
    }
}

// === Route Handlers ===

#[derive(Debug, Serialize)]
pub struct OrderFillsResponse {
    pub fills: Vec<TradeResponse>,
}

pub fn order_routes() -> Router<AppState> {
    Router::new()
        .route("/", post(place_order))
        .route("/", get(list_orders))
        .route("/:order_id", get(get_order))
        .route("/:order_id", delete(cancel_order))
        .route("/:order_id/fills", get(get_order_fills))
        .route("/trades", get(list_trades))
}

async fn place_order(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Json(req): Json<PlaceOrderHttpRequest>,
) -> Result<(StatusCode, Json<PlaceOrderResponse>), (StatusCode, Json<ErrorResponse>)> {
    // Determine quantity - either from quantity field or calculated from quote_amount
    let (quantity, quote_amount) = match (req.quantity, req.quote_amount) {
        // Quote currency order: "spend X EUR to buy KCN"
        (None, Some(qa)) => {
            // Only valid for market buy orders
            if req.order_type != OrderType::Market || req.side != Side::Bid {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Quote currency orders (quote_amount) are only supported for market buy orders".into(),
                        available: None,
                        required: None,
                    }),
                ));
            }

            if qa <= Decimal::ZERO {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Quote amount must be positive".into(),
                        available: None,
                        required: None,
                    }),
                ));
            }

            // Calculate quantity from quote_amount using max_slippage_price
            let price = req.max_slippage_price.unwrap_or_else(|| Decimal::from(1000));
            if price <= Decimal::ZERO {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "max_slippage_price must be positive for quote currency orders".into(),
                        available: None,
                        required: None,
                    }),
                ));
            }

            // quantity = quote_amount / price, rounded down to 8 decimals
            let calculated_qty = (qa / price).round_dp(8);
            (calculated_qty, Some(qa))
        }
        // Quantity provided but zero - treat as quote currency order if quote_amount present
        (Some(qty), Some(qa)) if qty.is_zero() => {
            // Only valid for market buy orders
            if req.order_type != OrderType::Market || req.side != Side::Bid {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Quote currency orders (quote_amount) are only supported for market buy orders".into(),
                        available: None,
                        required: None,
                    }),
                ));
            }

            if qa <= Decimal::ZERO {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Quote amount must be positive".into(),
                        available: None,
                        required: None,
                    }),
                ));
            }

            let price = req.max_slippage_price.unwrap_or_else(|| Decimal::from(1000));
            if price <= Decimal::ZERO {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "max_slippage_price must be positive for quote currency orders".into(),
                        available: None,
                        required: None,
                    }),
                ));
            }

            let calculated_qty = (qa / price).round_dp(8);
            (calculated_qty, Some(qa))
        }
        // Normal order with quantity
        (Some(qty), _) => {
            if qty <= Decimal::ZERO {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Quantity must be positive".into(),
                        available: None,
                        required: None,
                    }),
                ));
            }
            (qty, None)
        }
        // Neither provided
        (None, None) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Either quantity or quote_amount must be provided".into(),
                    available: None,
                    required: None,
                }),
            ));
        }
    };

    // Validate price for limit orders
    if req.order_type == OrderType::Limit {
        match req.price {
            Some(p) if p <= Decimal::ZERO => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Price must be positive".into(),
                        available: None,
                        required: None,
                    }),
                ));
            }
            None => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Limit order requires price".into(),
                        available: None,
                        required: None,
                    }),
                ));
            }
            _ => {}
        }
    }

    let result = Order::place(
        &state.pool,
        user.id,
        PlaceOrderRequest {
            symbol: req.symbol,
            side: req.side,
            order_type: req.order_type,
            price: req.price,
            quantity,
            quote_amount,
            max_slippage_price: req.max_slippage_price,
        },
    )
    .await
    .map_err(|e| {
        match e {
            OrderError::InsufficientBalance { available, required } => (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Insufficient balance".into(),
                    available: Some(available.to_string()),
                    required: Some(required.to_string()),
                }),
            ),
            OrderError::LimitOrderRequiresPrice => (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Limit order requires price".into(),
                    available: None,
                    required: None,
                }),
            ),
            OrderError::InvalidSymbol => (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Invalid symbol format (expected BASE/QUOTE)".into(),
                    available: None,
                    required: None,
                }),
            ),
            _ => {
                tracing::error!("Failed to place order: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Failed to place order".into(),
                        available: None,
                        required: None,
                    }),
                )
            }
        }
    })?;

    Ok((
        StatusCode::ACCEPTED,
        Json(PlaceOrderResponse {
            order: order_to_response(&result.order),
            locked_asset: result.locked_asset,
            locked_amount: result.locked_amount.to_string(),
        }),
    ))
}

async fn list_orders(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Query(query): Query<OrdersListQuery>,
) -> Result<Json<OrdersListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let limit = query.limit.min(100).max(1);
    let offset = query.offset.max(0);

    let (orders, total) = tokio::try_join!(
        Order::list_for_user(&state.pool, user.id, None, limit, offset),
        Order::count_for_user(&state.pool, user.id)
    )
    .map_err(|e| {
        tracing::error!("Failed to list orders: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to list orders".into(),
                available: None,
                required: None,
            }),
        )
    })?;

    Ok(Json(OrdersListResponse {
        orders: orders.iter().map(order_to_response).collect(),
        total,
        limit,
        offset,
    }))
}

async fn get_order(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    axum::extract::Path(order_id): axum::extract::Path<Uuid>,
) -> Result<Json<OrderResponse>, (StatusCode, Json<ErrorResponse>)> {
    let order = Order::get(&state.pool, user.id, order_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get order: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to get order".into(),
                    available: None,
                    required: None,
                }),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Order not found".into(),
                    available: None,
                    required: None,
                }),
            )
        })?;

    Ok(Json(order_to_response(&order)))
}

async fn cancel_order(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    axum::extract::Path(order_id): axum::extract::Path<Uuid>,
) -> Result<Json<CancelOrderResponse>, (StatusCode, Json<ErrorResponse>)> {
    let order = Order::cancel(&state.pool, user.id, order_id)
        .await
        .map_err(|e| {
            match e {
                OrderError::NotFound => (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: "Order not found".into(),
                        available: None,
                        required: None,
                    }),
                ),
                OrderError::CannotCancel(status) => (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: format!("Cannot cancel order with status: {}", status),
                        available: None,
                        required: None,
                    }),
                ),
                _ => {
                    tracing::error!("Failed to cancel order: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: "Failed to cancel order".into(),
                            available: None,
                            required: None,
                        }),
                    )
                }
            }
        })?;

    Ok(Json(CancelOrderResponse {
        order: order_to_response(&order),
        unlocked_asset: None, // TODO: Return actual unlocked amount
        unlocked_amount: None,
    }))
}

async fn list_trades(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Query(query): Query<TradesListQuery>,
) -> Result<Json<TradesListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let limit = query.limit.min(100).max(1);
    let offset = query.offset.max(0);

    let (trades, total) = tokio::try_join!(
        Trade::list_for_user(&state.pool, user.id, limit, offset),
        Trade::count_for_user(&state.pool, user.id)
    )
    .map_err(|e| {
        tracing::error!("Failed to list trades: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to list trades".into(),
                available: None,
                required: None,
            }),
        )
    })?;

    Ok(Json(TradesListResponse {
        trades: trades.iter().map(|t| trade_to_response(t, user.id)).collect(),
        total,
        limit,
        offset,
    }))
}

async fn get_order_fills(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    axum::extract::Path(order_id): axum::extract::Path<Uuid>,
) -> Result<Json<OrderFillsResponse>, (StatusCode, Json<ErrorResponse>)> {
    // First verify the order belongs to this user
    let order = Order::get(&state.pool, user.id, order_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get order: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to get order".into(),
                    available: None,
                    required: None,
                }),
            )
        })?;

    let Some(_order) = order else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Order not found".into(),
                available: None,
                required: None,
            }),
        ));
    };

    let fills = Trade::list_for_order(&state.pool, order_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get order fills: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to get fills".into(),
                    available: None,
                    required: None,
                }),
            )
        })?;

    Ok(Json(OrderFillsResponse {
        fills: fills.iter().map(|t| trade_to_response(t, user.id)).collect(),
    }))
}
