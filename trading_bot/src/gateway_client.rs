use futures_util::StreamExt;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

use crate::types::{GatewayMessage, MarketState, OrderRequest};

pub struct GatewayClient {
    gateway_url: String,
    ws_url: String,
    http_client: reqwest::Client,
    market_state: Arc<RwLock<MarketState>>,
}

impl GatewayClient {
    pub fn new(gateway_host: &str, gateway_port: u16) -> Self {
        let gateway_url = format!("http://{}:{}", gateway_host, gateway_port);
        let ws_url = format!("ws://{}:{}/ws", gateway_host, gateway_port);

        Self {
            gateway_url,
            ws_url,
            http_client: reqwest::Client::new(),
            market_state: Arc::new(RwLock::new(MarketState::new())),
        }
    }

    pub fn market_state(&self) -> Arc<RwLock<MarketState>> {
        Arc::clone(&self.market_state)
    }

    pub async fn connect_websocket(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Connecting to WebSocket: {}", self.ws_url);

        let (ws_stream, _) = connect_async(&self.ws_url).await?;
        let (_write, mut read) = ws_stream.split();

        let market_state = Arc::clone(&self.market_state);

        // Handle incoming messages
        tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Err(e) = Self::handle_message(&text, &market_state).await {
                            warn!("Failed to handle message: {}", e);
                        }
                    }
                    Ok(Message::Close(_)) => {
                        info!("WebSocket closed");
                        break;
                    }
                    Ok(Message::Ping(_)) => {
                        debug!("Received ping");
                    }
                    Err(e) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        });

        Ok(())
    }

    async fn handle_message(
        text: &str,
        market_state: &Arc<RwLock<MarketState>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let msg: GatewayMessage = serde_json::from_str(text)?;

        match msg {
            GatewayMessage::OrderbookSnapshot { bids, asks, .. } => {
                let mut state = market_state.write().await;
                if let Some(best_bid) = bids.first() {
                    state.best_bid = Some(best_bid.price);
                }
                if let Some(best_ask) = asks.first() {
                    state.best_ask = Some(best_ask.price);
                }
                debug!(
                    "Orderbook snapshot: bid={:?}, ask={:?}",
                    state.best_bid, state.best_ask
                );
            }
            GatewayMessage::OrderbookUpdate { bids, asks, .. } => {
                let mut state = market_state.write().await;
                if let Some(best_bid) = bids.first() {
                    state.best_bid = Some(best_bid.price);
                }
                if let Some(best_ask) = asks.first() {
                    state.best_ask = Some(best_ask.price);
                }
            }
            GatewayMessage::Trade { price, .. } => {
                let mut state = market_state.write().await;
                state.update_price(price);
                debug!("Trade: price={}", price);
            }
            GatewayMessage::Unknown => {
                debug!("Unknown message type");
            }
        }

        Ok(())
    }

    pub async fn submit_order(
        &self,
        order: &OrderRequest,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/api/order", self.gateway_url);

        let response = self
            .http_client
            .post(&url)
            .json(order)
            .send()
            .await?;

        if response.status().is_success() {
            debug!("Order submitted: {:?}", order);
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(format!("Order failed: {} - {}", status, body).into())
        }
    }

    pub async fn submit_orders(
        &self,
        orders: &[OrderRequest],
    ) -> Vec<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
        let mut results = Vec::with_capacity(orders.len());

        for order in orders {
            results.push(self.submit_order(order).await);
        }

        results
    }
}
