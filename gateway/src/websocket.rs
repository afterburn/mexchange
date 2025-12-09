use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn, error};
use uuid::Uuid;

use crate::channel_updates::OrderBookState;
use crate::events::{OrderCommand, Side};
use crate::udp_transport::UdpOrderSender;


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ClientMessage {
    Action(ActionMessage),
    Orders(OrdersMessage),
    BotRegister(BotRegisterMessage),
    BotStatus(BotStatusMessage),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrdersMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub orders: Vec<WsOrderRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsOrderRequest {
    pub side: String,
    pub order_type: String,
    pub price: Option<f64>,
    pub quantity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum ActionMessage {
    #[serde(rename = "subscribe")]
    Subscribe { channel: String },
    #[serde(rename = "unsubscribe")]
    Unsubscribe { channel: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BotRegisterMessage {
    #[serde(rename = "register_bot")]
    RegisterBot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotStatusMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub strategies: Vec<StrategyStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyStatus {
    pub strategy: String,
    pub running: bool,
    pub uptime_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BotCommand {
    pub command: BotCommandInner,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "action")]
pub enum BotCommandInner {
    #[serde(rename = "start")]
    Start { strategy: String },
    #[serde(rename = "stop")]
    Stop { strategy: String },
    #[serde(rename = "status")]
    Status,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChannelNotification {
    pub channel_name: String,
    pub notification: NotificationData,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotificationData {
    pub trades: Vec<TradeData>,
    pub bid_changes: Vec<PriceLevelChange>,
    pub ask_changes: Vec<PriceLevelChange>,
    pub total_bid_amount: f64,
    pub total_ask_amount: f64,
    pub time: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats_24h: Option<Stats24h>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Stats24h {
    pub high_24h: f64,
    pub low_24h: f64,
    pub volume_24h: f64,
    pub open_24h: f64,
    pub last_price: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TradeData {
    pub price: f64,
    pub quantity: f64,
    pub side: String,
    pub timestamp: u64,
    /// Order ID of the buyer (for matching user orders to fills)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buy_order_id: Option<String>,
    /// Order ID of the seller (for matching user orders to fills)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sell_order_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PriceLevelChange {
    pub price: f64,
    pub old_quantity: f64,
    pub new_quantity: f64,
}

impl serde::Serialize for PriceLevelChange {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeTuple;
        let mut tuple = serializer.serialize_tuple(3)?;
        tuple.serialize_element(&self.price)?;
        tuple.serialize_element(&self.old_quantity)?;
        tuple.serialize_element(&self.new_quantity)?;
        tuple.end()
    }
}

pub struct WebSocketSession {
    pub client_id: u64,
    pub subscriptions: HashSet<String>,
    pub sender: futures_util::stream::SplitSink<WebSocket, Message>,
}

pub struct ChannelManager {
    subscribers: HashMap<String, HashSet<u64>>,
    client_senders: HashMap<u64, broadcast::Sender<String>>,
    bot_client_id: Option<u64>,
    last_bot_status: Option<Vec<StrategyStatus>>,
}

impl ChannelManager {
    pub fn new() -> Self {
        Self {
            subscribers: HashMap::new(),
            client_senders: HashMap::new(),
            bot_client_id: None,
            last_bot_status: None,
        }
    }

    pub fn register_bot(&mut self, client_id: u64, sender: broadcast::Sender<String>) {
        self.bot_client_id = Some(client_id);
        self.client_senders.insert(client_id, sender);
        info!("Bot registered with client_id {}", client_id);
    }

    pub fn is_bot_connected(&self) -> bool {
        self.bot_client_id.is_some()
    }

    pub fn get_bot_sender(&self) -> Option<&broadcast::Sender<String>> {
        self.bot_client_id.and_then(|id| self.client_senders.get(&id))
    }

    pub fn set_bot_status(&mut self, status: Vec<StrategyStatus>) {
        self.last_bot_status = Some(status);
    }

    pub fn get_bot_status(&self) -> Option<&Vec<StrategyStatus>> {
        self.last_bot_status.as_ref()
    }

    pub fn subscribe(&mut self, client_id: u64, channel: String, sender: broadcast::Sender<String>) {
        self.subscribers
            .entry(channel.clone())
            .or_insert_with(HashSet::new)
            .insert(client_id);
        self.client_senders.insert(client_id, sender);
        info!("Client {} subscribed to channel {}", client_id, channel);
    }

    pub fn unsubscribe(&mut self, client_id: u64, channel: &str) {
        if let Some(subscribers) = self.subscribers.get_mut(channel) {
            subscribers.remove(&client_id);
            if subscribers.is_empty() {
                self.subscribers.remove(channel);
            }
        }
        info!("Client {} unsubscribed from channel {}", client_id, channel);
    }

    pub fn remove_client(&mut self, client_id: u64) {
        let channels: Vec<String> = self
            .subscribers
            .iter()
            .filter(|(_, subs)| subs.contains(&client_id))
            .map(|(channel, _)| channel.clone())
            .collect();

        for channel in channels {
            self.unsubscribe(client_id, &channel);
        }

        self.client_senders.remove(&client_id);

        // If this was the bot, clear bot state
        if self.bot_client_id == Some(client_id) {
            self.bot_client_id = None;
            self.last_bot_status = None;
            info!("Bot disconnected");
        }
    }

    pub fn get_subscribers(&self, channel: &str) -> Vec<u64> {
        self.subscribers
            .get(channel)
            .map(|subs| subs.iter().copied().collect())
            .unwrap_or_default()
    }

    pub fn get_sender(&self, client_id: u64) -> Option<&broadcast::Sender<String>> {
        self.client_senders.get(&client_id)
    }
}

pub async fn handle_websocket_connection(
    socket: WebSocket,
    client_id: u64,
    mut event_rx: broadcast::Receiver<String>,
    channel_manager: Arc<tokio::sync::RwLock<ChannelManager>>,
    orderbook_states: Arc<tokio::sync::RwLock<HashMap<String, OrderBookState>>>,
    order_sender: Arc<UdpOrderSender>,
) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = broadcast::channel(1000);

    {
        let mut cm = channel_manager.write().await;
        cm.client_senders.insert(client_id, tx.clone());
    }

    let channel_manager_clone = channel_manager.clone();
    let orderbook_states_clone = orderbook_states.clone();
    let order_sender_clone = order_sender.clone();
    let client_id_clone = client_id;
    let tx_for_recv = tx.clone();

    // Forward direct market events (OrderFilled, OrderCancelled) to client
    let tx_for_events = tx.clone();
    tokio::spawn(async move {
        while let Ok(event_json) = event_rx.recv().await {
            // Forward directly to the client's send channel
            let _ = tx_for_events.send(event_json);
        }
    });

    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            match serde_json::from_str::<ClientMessage>(&text) {
                Ok(ClientMessage::Action(ActionMessage::Subscribe { channel })) => {
                    // Parse the channel to extract the symbol (e.g., "book.KCN/EUR.none.10.100ms" -> "KCN/EUR")
                    let symbol = channel
                        .strip_prefix("book.")
                        .and_then(|s| s.split('.').next())
                        .unwrap_or("KCN/EUR");

                    // Send snapshot first
                    let ob_states = orderbook_states_clone.read().await;
                    if let Some(ob_state) = ob_states.get(symbol) {
                        let snapshot = ob_state.get_orderbook_snapshot(symbol);
                        match serde_json::to_string(&snapshot) {
                            Ok(json) => {
                                let _ = tx_for_recv.send(json);
                                info!("Sent orderbook snapshot to client {} for {}", client_id_clone, symbol);
                            }
                            Err(e) => {
                                error!("Failed to serialize snapshot: {}", e);
                            }
                        }
                    }
                    drop(ob_states);

                    let mut cm = channel_manager_clone.write().await;
                    cm.subscribe(client_id_clone, channel, tx_for_recv.clone());
                }
                Ok(ClientMessage::Action(ActionMessage::Unsubscribe { channel })) => {
                    let mut cm = channel_manager_clone.write().await;
                    cm.unsubscribe(client_id_clone, &channel);
                }
                Ok(ClientMessage::Orders(orders_msg)) => {
                    if orders_msg.msg_type == "orders" {
                        info!("Received {} orders via WebSocket from client {}", orders_msg.orders.len(), client_id_clone);
                        // Process batch orders via WebSocket
                        for order in orders_msg.orders {
                            let side = match order.side.as_str() {
                                "bid" => Side::Bid,
                                "ask" => Side::Ask,
                                _ => continue,
                            };

                            let price = order.price.map(|p| {
                                Decimal::from_str(&p.to_string()).unwrap_or_default()
                            });

                            let quantity = Decimal::from_str(&order.quantity.to_string())
                                .unwrap_or_default();

                            let command = OrderCommand::PlaceOrder {
                                order_id: Uuid::new_v4(),
                                side,
                                order_type: order.order_type,
                                price,
                                quantity,
                                user_id: None,
                            };

                            if let Err(e) = order_sender_clone.send_order_command(&command).await {
                                error!("Failed to send order via WS: {}", e);
                            }
                        }
                    }
                }
                Ok(ClientMessage::BotRegister(BotRegisterMessage::RegisterBot)) => {
                    let mut cm = channel_manager_clone.write().await;
                    cm.register_bot(client_id_clone, tx_for_recv.clone());
                }
                Ok(ClientMessage::BotStatus(status)) => {
                    if status.msg_type == "bot_status" {
                        let mut cm = channel_manager_clone.write().await;
                        cm.set_bot_status(status.strategies);
                        info!("Received bot status update");
                    }
                }
                Err(e) => {
                    warn!("Invalid client message: {} - {}", text, e);
                }
            }
        }

        let mut cm = channel_manager_clone.write().await;
        cm.remove_client(client_id_clone);
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };
}

