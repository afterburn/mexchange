use rust_decimal::Decimal;
use std::net::{SocketAddr, ToSocketAddrs};
use std::str::FromStr;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info};

use udp_proto::{
    MessageType, ReceiverConfig, ReceivedMessage, SenderConfig, UdpReceiver, UdpSender,
    binary::{decode_market_event, MarketEvent as BinaryMarketEvent, DeltaAction as BinaryDeltaAction, Side as BinarySide},
};

use crate::events::{MarketEvent, OrderCommand, PriceLevel, LevelDelta, DeltaAction, Side};

/// Resolve a string address (hostname:port or ip:port) to SocketAddr
/// Retries with exponential backoff for DNS resolution
fn resolve_addr(addr_str: &str) -> SocketAddr {
    let mut attempts = 0;
    let max_attempts = 10;
    let mut delay = std::time::Duration::from_millis(100);

    loop {
        match addr_str.to_socket_addrs() {
            Ok(mut addrs) => {
                if let Some(addr) = addrs.next() {
                    return addr;
                }
            }
            Err(e) => {
                attempts += 1;
                if attempts >= max_attempts {
                    panic!(
                        "Failed to resolve address '{}' after {} attempts: {}",
                        addr_str, max_attempts, e
                    );
                }
                tracing::warn!(
                    "DNS resolution failed for '{}' (attempt {}/{}): {}. Retrying in {:?}...",
                    addr_str,
                    attempts,
                    max_attempts,
                    e,
                    delay
                );
                std::thread::sleep(delay);
                delay = std::cmp::min(delay * 2, std::time::Duration::from_secs(5));
            }
        }
    }
}

// Stream IDs
const ORDER_STREAM_ID: u32 = 1;
const EVENT_STREAM_ID: u32 = 2;

/// UDP sender for order commands (gateway -> matching engine)
pub struct UdpOrderSender {
    sender: UdpSender,
}

impl UdpOrderSender {
    pub fn new(target_addr: SocketAddr, bind_addr: SocketAddr) -> anyhow::Result<Self> {
        let config = SenderConfig {
            stream_id: ORDER_STREAM_ID,
            target_addr,
            max_batch_delay: Duration::from_micros(100),
            channel_capacity: 10_000,
            enable_heartbeats: true,
        };

        let sender = UdpSender::new(config, bind_addr)?;
        info!("UDP order sender created: {} -> {}", bind_addr, target_addr);

        Ok(Self { sender })
    }

    pub async fn send_order_command(&self, command: &OrderCommand) -> anyhow::Result<()> {
        let json = serde_json::to_vec(command)?;

        self.sender
            .try_send(MessageType::OrderNew, json)
            .map_err(|e| anyhow::anyhow!("UDP send error: {:?}", e))?;

        Ok(())
    }

    pub fn stats(&self) -> udp_proto::SenderStatsSnapshot {
        self.sender.stats()
    }
}

/// UDP receiver for market events (matching engine -> gateway)
pub struct UdpEventReceiver {
    event_tx: mpsc::Sender<MarketEvent>,
}

impl UdpEventReceiver {
    pub fn new(
        bind_addr: SocketAddr,
    ) -> anyhow::Result<(Self, mpsc::Receiver<MarketEvent>)> {
        let config = ReceiverConfig {
            stream_id: EVENT_STREAM_ID,
            channel_capacity: 10_000,
            recv_timeout: Duration::from_millis(10),
            stream_timeout: Duration::from_millis(500),
        };

        let receiver = UdpReceiver::new(config, bind_addr)?;
        info!("UDP event receiver created on {}", bind_addr);

        // Create channel for forwarding events
        let (event_tx, event_rx) = mpsc::channel(10_000);

        // Spawn receiver task
        let tx = event_tx.clone();
        std::thread::spawn(move || {
            Self::receiver_loop(receiver, tx);
        });

        Ok((Self { event_tx }, event_rx))
    }

    fn receiver_loop(receiver: UdpReceiver, event_tx: mpsc::Sender<MarketEvent>) {
        info!("UDP event receiver loop started");

        loop {
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(Some(msg)) => {
                    if let Err(e) = Self::handle_message(&msg, &event_tx) {
                        error!("Failed to handle UDP message: {}", e);
                    }
                }
                Ok(None) => {
                    // Timeout, continue
                }
                Err(e) => {
                    error!("UDP receiver error: {:?}", e);
                    break;
                }
            }
        }
    }

    fn handle_message(
        msg: &ReceivedMessage,
        event_tx: &mpsc::Sender<MarketEvent>,
    ) -> anyhow::Result<()> {
        // Decode FlatBuffers payload
        let binary_event = decode_market_event(&msg.payload)
            .map_err(|e| anyhow::anyhow!("FlatBuffer decode error: {}", e))?;

        // Convert to internal MarketEvent type
        let event = Self::convert_from_binary(binary_event)?;

        // Log fill and cancel events for debugging
        match &event {
            MarketEvent::Fill { buy_order_id, sell_order_id, price, quantity, .. } => {
                info!("Received Fill event via UDP: buy_order={}, sell_order={}, price={}, qty={}",
                    buy_order_id, sell_order_id, price, quantity);
            }
            MarketEvent::OrderCancelled { order_id, filled_quantity } => {
                info!("Received OrderCancelled event via UDP: order_id={}, filled_qty={}",
                    order_id, filled_quantity);
            }
            MarketEvent::OrderFilled { order_id } => {
                info!("Received OrderFilled event via UDP: order_id={}", order_id);
            }
            _ => {}
        }

        // Send to channel (blocking in sync context)
        event_tx.blocking_send(event)?;

        Ok(())
    }

    fn convert_from_binary(event: BinaryMarketEvent) -> anyhow::Result<MarketEvent> {
        match event {
            BinaryMarketEvent::Fill {
                symbol,
                buy_order_id,
                sell_order_id,
                price,
                quantity,
                timestamp,
            } => Ok(MarketEvent::Fill {
                symbol,
                buy_order_id,
                sell_order_id,
                price: Decimal::from_str(&price.to_string()).unwrap_or_default(),
                quantity: Decimal::from_str(&quantity.to_string()).unwrap_or_default(),
                timestamp,
            }),
            BinaryMarketEvent::OrderBookSnapshot {
                symbol,
                sequence,
                bids,
                asks,
            } => Ok(MarketEvent::OrderBookSnapshot {
                symbol,
                sequence,
                bids: bids
                    .into_iter()
                    .map(|l| PriceLevel {
                        price: Decimal::from_str(&l.price.to_string()).unwrap_or_default(),
                        quantity: Decimal::from_str(&l.quantity.to_string()).unwrap_or_default(),
                    })
                    .collect(),
                asks: asks
                    .into_iter()
                    .map(|l| PriceLevel {
                        price: Decimal::from_str(&l.price.to_string()).unwrap_or_default(),
                        quantity: Decimal::from_str(&l.quantity.to_string()).unwrap_or_default(),
                    })
                    .collect(),
            }),
            BinaryMarketEvent::OrderBookDelta {
                symbol,
                sequence,
                deltas,
            } => Ok(MarketEvent::OrderBookDelta {
                symbol,
                sequence,
                deltas: deltas
                    .into_iter()
                    .map(|d| LevelDelta {
                        action: match d.action {
                            BinaryDeltaAction::Add => DeltaAction::Add,
                            BinaryDeltaAction::Update => DeltaAction::Update,
                            BinaryDeltaAction::Remove => DeltaAction::Remove,
                        },
                        side: match d.side {
                            BinarySide::Bid => Side::Bid,
                            BinarySide::Ask => Side::Ask,
                        },
                        price: Decimal::from_str(&d.price.to_string()).unwrap_or_default(),
                        quantity: Decimal::from_str(&d.quantity.to_string()).unwrap_or_default(),
                    })
                    .collect(),
            }),
            BinaryMarketEvent::OrderCancelled {
                order_id,
                filled_quantity,
            } => Ok(MarketEvent::OrderCancelled {
                order_id,
                filled_quantity: Decimal::from_str(&filled_quantity.to_string()).unwrap_or_default(),
            }),
            BinaryMarketEvent::OrderFilled { order_id } => Ok(MarketEvent::OrderFilled {
                order_id,
            }),
        }
    }
}

/// Configuration for UDP transport
#[derive(Debug, Clone)]
pub struct UdpTransportConfig {
    /// Address to send orders to (matching engine)
    pub matching_engine_addr: SocketAddr,
    /// Local address to bind order sender
    pub order_sender_bind: SocketAddr,
    /// Local address to receive market events
    pub event_receiver_bind: SocketAddr,
}

impl Default for UdpTransportConfig {
    fn default() -> Self {
        Self {
            matching_engine_addr: "127.0.0.1:9100".parse().unwrap(),
            order_sender_bind: "127.0.0.1:9102".parse().unwrap(),
            event_receiver_bind: "127.0.0.1:9101".parse().unwrap(),
        }
    }
}

impl UdpTransportConfig {
    pub fn from_env() -> Self {
        let matching_engine_addr_str = std::env::var("MATCHING_ENGINE_UDP_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:9100".to_string());
        let matching_engine_addr = resolve_addr(&matching_engine_addr_str);

        let order_sender_bind_str = std::env::var("ORDER_SENDER_BIND")
            .unwrap_or_else(|_| "0.0.0.0:9102".to_string());
        let order_sender_bind = resolve_addr(&order_sender_bind_str);

        let event_receiver_bind_str = std::env::var("EVENT_RECEIVER_BIND")
            .unwrap_or_else(|_| "0.0.0.0:9101".to_string());
        let event_receiver_bind = resolve_addr(&event_receiver_bind_str);

        Self {
            matching_engine_addr,
            order_sender_bind,
            event_receiver_bind,
        }
    }
}
