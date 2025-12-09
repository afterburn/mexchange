use axum::{
    extract::State,
    http::StatusCode,
    routing::post,
    Extension, Json, Router,
};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::models::{EntryType, LedgerEntry, User};
use crate::AppState;

const FAUCET_AMOUNT_KCN: &str = "100";
const FAUCET_COOLDOWN_HOURS: i64 = 24;

#[derive(Debug, Deserialize)]
pub struct FaucetRequest {
    pub asset: Option<String>, // Default to KCN
}

#[derive(Debug, Serialize)]
pub struct FaucetResponse {
    pub success: bool,
    pub asset: String,
    pub amount: String,
    pub new_balance: String,
    pub next_claim_at: String,
}

#[derive(Debug, Serialize)]
pub struct FaucetErrorResponse {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_claim_at: Option<String>,
}

pub fn faucet_routes() -> Router<AppState> {
    Router::new()
        .route("/claim", post(claim_faucet))
        .route("/status", axum::routing::get(faucet_status))
}

async fn claim_faucet(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Json(req): Json<FaucetRequest>,
) -> Result<Json<FaucetResponse>, (StatusCode, Json<FaucetErrorResponse>)> {
    let asset = req.asset.unwrap_or_else(|| "KCN".to_string());

    // Only allow KCN faucet for demo
    if asset != "KCN" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(FaucetErrorResponse {
                error: "Only KCN faucet is available".into(),
                next_claim_at: None,
            }),
        ));
    }

    // Check last claim time
    let last_claim: Option<chrono::DateTime<Utc>> = sqlx::query_scalar(
        "SELECT claimed_at FROM faucet_claims
         WHERE user_id = $1 AND asset = $2
         ORDER BY claimed_at DESC
         LIMIT 1"
    )
    .bind(user.id)
    .bind(&asset)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to check faucet claim: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(FaucetErrorResponse {
                error: "Failed to check faucet status".into(),
                next_claim_at: None,
            }),
        )
    })?;

    let cooldown = Duration::hours(FAUCET_COOLDOWN_HOURS);
    let now = Utc::now();

    if let Some(last) = last_claim {
        let next_claim = last + cooldown;
        if now < next_claim {
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                Json(FaucetErrorResponse {
                    error: format!(
                        "Faucet is on cooldown. Please wait {} hours between claims.",
                        FAUCET_COOLDOWN_HOURS
                    ),
                    next_claim_at: Some(next_claim.to_rfc3339()),
                }),
            ));
        }
    }

    let amount = Decimal::from_str(FAUCET_AMOUNT_KCN).unwrap();

    // Credit the asset
    let entry = LedgerEntry::append(
        &state.pool,
        user.id,
        &asset,
        amount,
        EntryType::Deposit,
        None,
        Some("Faucet claim"),
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to credit faucet: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(FaucetErrorResponse {
                error: "Failed to process faucet claim".into(),
                next_claim_at: None,
            }),
        )
    })?;

    // Record the claim
    sqlx::query(
        "INSERT INTO faucet_claims (user_id, asset, amount) VALUES ($1, $2, $3)"
    )
    .bind(user.id)
    .bind(&asset)
    .bind(amount)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to record faucet claim: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(FaucetErrorResponse {
                error: "Failed to record faucet claim".into(),
                next_claim_at: None,
            }),
        )
    })?;

    let next_claim = now + cooldown;

    Ok(Json(FaucetResponse {
        success: true,
        asset,
        amount: amount.to_string(),
        new_balance: entry.balance_after.to_string(),
        next_claim_at: next_claim.to_rfc3339(),
    }))
}

#[derive(Debug, Serialize)]
pub struct FaucetStatusResponse {
    pub available: bool,
    pub next_claim_at: Option<String>,
    pub last_claim_at: Option<String>,
    pub amount_per_claim: String,
    pub cooldown_hours: i64,
}

async fn faucet_status(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
) -> Result<Json<FaucetStatusResponse>, (StatusCode, Json<FaucetErrorResponse>)> {
    let last_claim: Option<chrono::DateTime<Utc>> = sqlx::query_scalar(
        "SELECT claimed_at FROM faucet_claims
         WHERE user_id = $1 AND asset = 'KCN'
         ORDER BY claimed_at DESC
         LIMIT 1"
    )
    .bind(user.id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to check faucet status: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(FaucetErrorResponse {
                error: "Failed to check faucet status".into(),
                next_claim_at: None,
            }),
        )
    })?;

    let cooldown = Duration::hours(FAUCET_COOLDOWN_HOURS);
    let now = Utc::now();

    let (available, next_claim_at) = match last_claim {
        Some(last) => {
            let next = last + cooldown;
            (now >= next, Some(next.to_rfc3339()))
        }
        None => (true, None),
    };

    Ok(Json(FaucetStatusResponse {
        available,
        next_claim_at,
        last_claim_at: last_claim.map(|t| t.to_rfc3339()),
        amount_per_claim: FAUCET_AMOUNT_KCN.to_string(),
        cooldown_hours: FAUCET_COOLDOWN_HOURS,
    }))
}
