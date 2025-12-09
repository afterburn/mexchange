use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::models::OHLCV;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct OHLCVQuery {
    symbol: String,
    interval: String,
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_limit() -> i64 {
    500
}

#[derive(Debug, Serialize)]
pub struct OHLCVBar {
    open_time: String,
    open: String,
    high: String,
    low: String,
    close: String,
    volume: String,
    trade_count: i32,
}

#[derive(Debug, Serialize)]
pub struct OHLCVResponse {
    data: Vec<OHLCVBar>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    error: String,
}

pub fn ohlcv_routes() -> Router<AppState> {
    Router::new().route("/", get(get_ohlcv))
}

async fn get_ohlcv(
    State(state): State<AppState>,
    Query(query): Query<OHLCVQuery>,
) -> Result<Json<OHLCVResponse>, (StatusCode, Json<ErrorResponse>)> {
    let limit = query.limit.min(1000).max(1);

    let data = OHLCV::get_latest(&state.pool, &query.symbol, &query.interval, limit)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get OHLCV data: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to get data".into() }))
        })?;

    // Reverse to get chronological order (oldest first)
    let bars: Vec<OHLCVBar> = data
        .into_iter()
        .rev()
        .map(|bar| OHLCVBar {
            open_time: bar.open_time.to_rfc3339(),
            open: bar.open.to_string(),
            high: bar.high.to_string(),
            low: bar.low.to_string(),
            close: bar.close.to_string(),
            volume: bar.volume.to_string(),
            trade_count: bar.trade_count,
        })
        .collect();

    Ok(Json(OHLCVResponse { data: bars }))
}
