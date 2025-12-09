use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::info;

use crate::events::MarketEvent;

pub type ClientId = u64;

#[derive(Clone)]
pub struct GatewayState {
    clients: Arc<RwLock<HashMap<ClientId, broadcast::Sender<String>>>>,
    next_client_id: Arc<RwLock<ClientId>>,
    event_tx: broadcast::Sender<MarketEvent>,
}

impl GatewayState {
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(10000);
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            next_client_id: Arc::new(RwLock::new(1)),
            event_tx,
        }
    }

    pub async fn add_client(&self) -> (ClientId, broadcast::Receiver<String>) {
        let mut id = self.next_client_id.write().await;
        let client_id = *id;
        *id += 1;

        let (tx, rx) = broadcast::channel(1000);
        self.clients.write().await.insert(client_id, tx);

        info!("Client {} connected", client_id);
        (client_id, rx)
    }

    pub async fn remove_client(&self, client_id: ClientId) {
        self.clients.write().await.remove(&client_id);
        info!("Client {} disconnected", client_id);
    }

    pub async fn broadcast_event(&self, event: MarketEvent) {
        let json = match serde_json::to_string(&event) {
            Ok(json) => json,
            Err(e) => {
                tracing::error!("Failed to serialize event: {}", e);
                return;
            }
        };

        let clients = self.clients.read().await;
        let mut disconnected = Vec::new();

        for (client_id, tx) in clients.iter() {
            if tx.send(json.clone()).is_err() {
                disconnected.push(*client_id);
            }
        }

        drop(clients);

        if !disconnected.is_empty() {
            let mut clients = self.clients.write().await;
            for client_id in disconnected {
                clients.remove(&client_id);
            }
        }
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<MarketEvent> {
        self.event_tx.subscribe()
    }

    pub fn publish_event(&self, event: MarketEvent) {
        let _ = self.event_tx.send(event);
    }
}


