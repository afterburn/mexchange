use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Extension, Json, Router,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::models::{Balance, EntryType, LedgerEntry, User};
use crate::AppState;

#[derive(Debug, Serialize)]
pub struct BalanceResponse {
    asset: String,
    available: String,
    locked: String,
}

#[derive(Debug, Serialize)]
pub struct BalancesResponse {
    balances: Vec<BalanceResponse>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    error: String,
}

pub fn balance_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(get_balances))
        .route("/deposit", post(deposit))
        .route("/withdraw", post(withdraw))
}

#[derive(Debug, Deserialize)]
pub struct DepositRequest {
    asset: String,
    amount: Decimal,
}

#[derive(Debug, Serialize)]
pub struct DepositResponse {
    success: bool,
    balance: BalanceResponse,
}

#[derive(Debug, Deserialize)]
pub struct WithdrawRequest {
    asset: String,
    amount: Decimal,
}

#[derive(Debug, Serialize)]
pub struct WithdrawResponse {
    success: bool,
    balance: BalanceResponse,
}

async fn get_balances(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
) -> Result<Json<BalancesResponse>, (StatusCode, Json<ErrorResponse>)> {
    let balances = Balance::get_for_user(&state.pool, user.id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get balances: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to get balances".into() }))
        })?;

    Ok(Json(BalancesResponse {
        balances: balances
            .into_iter()
            .map(|b| {
                let available = LedgerEntry::round_to_precision(&b.asset, b.available);
                let locked = LedgerEntry::round_to_precision(&b.asset, b.locked);
                BalanceResponse {
                    asset: b.asset,
                    available: available.to_string(),
                    locked: locked.to_string(),
                }
            })
            .collect(),
    }))
}

async fn deposit(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Json(req): Json<DepositRequest>,
) -> Result<Json<DepositResponse>, (StatusCode, Json<ErrorResponse>)> {
    if req.amount <= Decimal::ZERO {
        return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "Amount must be positive".into() })));
    }

    // Only allow EUR deposits for demo
    if req.asset != "EUR" {
        return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "Only EUR deposits are supported".into() })));
    }

    // Round to asset precision (EUR = 2 decimals)
    let amount = LedgerEntry::round_to_precision(&req.asset, req.amount);

    // Append to ledger (this also updates the cached balance)
    let entry = LedgerEntry::append(
        &state.pool,
        user.id,
        &req.asset,
        amount,
        EntryType::Deposit,
        None,
        Some("User deposit"),
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to deposit: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to process deposit".into() }))
    })?;

    Ok(Json(DepositResponse {
        success: true,
        balance: BalanceResponse {
            asset: entry.asset,
            available: entry.balance_after.to_string(),
            locked: "0".to_string(),
        },
    }))
}

async fn withdraw(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Json(req): Json<WithdrawRequest>,
) -> Result<Json<WithdrawResponse>, (StatusCode, Json<ErrorResponse>)> {
    if req.amount <= Decimal::ZERO {
        return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "Amount must be positive".into() })));
    }

    // Only allow EUR withdrawals for demo
    if req.asset != "EUR" {
        return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "Only EUR withdrawals are supported".into() })));
    }

    // Round to asset precision (EUR = 2 decimals)
    let amount = LedgerEntry::round_to_precision(&req.asset, req.amount);

    // Append withdrawal to ledger (negative amount)
    // LedgerEntry::append handles the balance check atomically within the advisory lock
    let entry = LedgerEntry::append(
        &state.pool,
        user.id,
        &req.asset,
        -amount,
        EntryType::Withdrawal,
        None,
        Some("User withdrawal"),
    )
    .await
    .map_err(|e| {
        // Distinguish between insufficient balance and other errors
        let msg = e.to_string();
        if msg.contains("Insufficient balance") {
            (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "Insufficient balance".into() }))
        } else {
            tracing::error!("Failed to withdraw: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to process withdrawal".into() }))
        }
    })?;

    Ok(Json(WithdrawResponse {
        success: true,
        balance: BalanceResponse {
            asset: entry.asset,
            available: entry.balance_after.to_string(),
            locked: "0".to_string(),
        },
    }))
}
