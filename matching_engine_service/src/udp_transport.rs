use parking_lot;
use std::net::{SocketAddr, ToSocketAddrs};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info};

use udp_proto::{
    MessageType, ReceiverConfig, ReceivedMessage, SenderConfig, UdpReceiver, UdpSender,
    binary::{MarketEventEncoder, MarketEvent as BinaryMarketEvent, PriceLevel, LevelDelta, Side as BinarySide, DeltaAction as BinaryDeltaAction},
};

use crate::events::{MarketEvent, OrderCommand, Side, DeltaAction};

/// Resolve a string address (hostname:port or ip:port) to SocketAddr
/// Retries with exponential backoff for DNS resolution
fn resolve_addr(addr_str: &str) -> SocketAddr {
    let mut attempts = 0;
    let max_attempts = 10;
    let mut delay = Duration::from_millis(100);

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
                delay = std::cmp::min(delay * 2, Duration::from_secs(5));
            }
        }
    }
}

// Stream IDs (must match gateway)
const ORDER_STREAM_ID: u32 = 1;
const EVENT_STREAM_ID: u32 = 2;

/// UDP sender for market events (matching engine -> gateway)
/// Uses lazy DNS resolution for the target address
pub struct UdpEventSender {
    sender: parking_lot::RwLock<Option<UdpSender>>,
    encoder: parking_lot::Mutex<MarketEventEncoder>,
    target_addr_str: String,
    bind_addr: SocketAddr,
}

impl UdpEventSender {
    pub fn new(target_addr_str: String, bind_addr: SocketAddr) -> anyhow::Result<Self> {
        info!(
            "UDP event sender initialized (lazy): {} -> {}",
            bind_addr, target_addr_str
        );

        // Try to create the sender immediately (might fail if DNS not ready)
        let sender = Self::try_create_sender(&target_addr_str, bind_addr);

        Ok(Self {
            sender: parking_lot::RwLock::new(sender),
            encoder: parking_lot::Mutex::new(MarketEventEncoder::new()),
            target_addr_str,
            bind_addr,
        })
    }

    fn try_create_sender(target_addr_str: &str, bind_addr: SocketAddr) -> Option<UdpSender> {
        match target_addr_str.to_socket_addrs() {
            Ok(mut addrs) => {
                if let Some(target_addr) = addrs.next() {
                    let config = SenderConfig {
                        stream_id: EVENT_STREAM_ID,
                        target_addr,
                        max_batch_delay: Duration::from_micros(100),
                        channel_capacity: 10_000,
                        enable_heartbeats: true,
                    };
                    match UdpSender::new(config, bind_addr) {
                        Ok(sender) => {
                            info!(
                                "UDP event sender created: {} -> {}",
                                bind_addr, target_addr
                            );
                            return Some(sender);
                        }
                        Err(e) => {
                            error!("Failed to create UDP sender: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                info!(
                    "DNS not ready for '{}': {}. Will retry on first send.",
                    target_addr_str, e
                );
            }
        }
        None
    }

    fn ensure_sender(&self) -> Option<()> {
        // Check if sender already exists
        if self.sender.read().is_some() {
            return Some(());
        }

        // Try to create sender
        let mut guard = self.sender.write();
        if guard.is_some() {
            return Some(());
        }

        // Retry DNS resolution
        if let Some(sender) = Self::try_create_sender(&self.target_addr_str, self.bind_addr) {
            *guard = Some(sender);
            return Some(());
        }

        None
    }

    pub async fn send_event(&self, event: &MarketEvent) -> anyhow::Result<()> {
        // Skip events that should not be broadcast (they're only for the originating client)
        match event {
            MarketEvent::OrderAccepted { .. } => {
                // OrderAccepted is handled by the gateway's HTTP response path
                // It should not be broadcast over UDP
                return Ok(());
            }
            MarketEvent::OrderCancelled { order_id, filled_quantity, .. } => {
                info!("Sending OrderCancelled event via UDP: order_id={}, filled_qty={}",
                    order_id, filled_quantity);
            }
            MarketEvent::OrderFilled { order_id } => {
                info!("Sending OrderFilled event via UDP: order_id={}", order_id);
            }
            MarketEvent::Fill { buy_order_id, sell_order_id, price, quantity, .. } => {
                info!("Sending Fill event via UDP: buy_order={}, sell_order={}, price={}, qty={}",
                    buy_order_id, sell_order_id, price, quantity);
            }
            _ => {}
        }

        // Ensure sender is ready (lazy init)
        if self.ensure_sender().is_none() {
            return Err(anyhow::anyhow!(
                "UDP sender not ready (DNS resolution pending for {})",
                self.target_addr_str
            ));
        }

        // Convert to binary format
        let binary_event = Self::convert_to_binary(event);

        // Encode with FlatBuffers
        let data = {
            let mut encoder = self.encoder.lock();
            encoder.encode(&binary_event).to_vec()
        };

        let guard = self.sender.read();
        if let Some(sender) = guard.as_ref() {
            sender
                .try_send(MessageType::MatchEvent, data)
                .map_err(|e| anyhow::anyhow!("UDP send error: {:?}", e))?;
        }

        Ok(())
    }

    fn convert_to_binary(event: &MarketEvent) -> BinaryMarketEvent {
        match event {
            MarketEvent::Fill {
                buy_order_id,
                sell_order_id,
                price,
                quantity,
                timestamp,
                symbol,
            } => BinaryMarketEvent::Fill {
                symbol: symbol.clone(),
                buy_order_id: *buy_order_id,
                sell_order_id: *sell_order_id,
                price: price.to_string().parse().unwrap_or(0.0),
                quantity: quantity.to_string().parse().unwrap_or(0.0),
                timestamp: *timestamp,
            },
            MarketEvent::OrderBookSnapshot {
                symbol,
                sequence,
                bids,
                asks,
            } => BinaryMarketEvent::OrderBookSnapshot {
                symbol: symbol.clone(),
                sequence: *sequence,
                bids: bids
                    .iter()
                    .map(|l| PriceLevel {
                        price: l.price.to_string().parse().unwrap_or(0.0),
                        quantity: l.quantity.to_string().parse().unwrap_or(0.0),
                    })
                    .collect(),
                asks: asks
                    .iter()
                    .map(|l| PriceLevel {
                        price: l.price.to_string().parse().unwrap_or(0.0),
                        quantity: l.quantity.to_string().parse().unwrap_or(0.0),
                    })
                    .collect(),
            },
            MarketEvent::OrderBookDelta {
                symbol,
                sequence,
                deltas,
            } => BinaryMarketEvent::OrderBookDelta {
                symbol: symbol.clone(),
                sequence: *sequence,
                deltas: deltas
                    .iter()
                    .map(|d| LevelDelta {
                        action: match d.action {
                            DeltaAction::Add => BinaryDeltaAction::Add,
                            DeltaAction::Update => BinaryDeltaAction::Update,
                            DeltaAction::Remove => BinaryDeltaAction::Remove,
                        },
                        side: match d.side {
                            Side::Bid => BinarySide::Bid,
                            Side::Ask => BinarySide::Ask,
                        },
                        price: d.price.to_string().parse().unwrap_or(0.0),
                        quantity: d.quantity.to_string().parse().unwrap_or(0.0),
                    })
                    .collect(),
            },
            // OrderAccepted is filtered out in send_event()
            MarketEvent::OrderAccepted { .. } => {
                unreachable!("OrderAccepted should be filtered in send_event()")
            }
            MarketEvent::OrderCancelled { order_id, filled_quantity } => {
                BinaryMarketEvent::OrderCancelled {
                    order_id: *order_id,
                    filled_quantity: filled_quantity.to_string().parse().unwrap_or(0.0),
                }
            }
            MarketEvent::OrderFilled { order_id } => {
                BinaryMarketEvent::OrderFilled {
                    order_id: *order_id,
                }
            }
        }
    }

    pub fn stats(&self) -> Option<udp_proto::SenderStatsSnapshot> {
        self.sender.read().as_ref().map(|s| s.stats())
    }
}

/// UDP receiver for order commands (gateway -> matching engine)
pub struct UdpOrderReceiver {
    _receiver: UdpReceiver,
}

impl UdpOrderReceiver {
    pub fn new(
        bind_addr: SocketAddr,
    ) -> anyhow::Result<(Self, mpsc::Receiver<OrderCommand>)> {
        let config = ReceiverConfig {
            stream_id: ORDER_STREAM_ID,
            channel_capacity: 10_000,
            recv_timeout: Duration::from_millis(10),
            stream_timeout: Duration::from_millis(500),
        };

        let receiver = UdpReceiver::new(config, bind_addr)?;
        info!("UDP order receiver created on {}", bind_addr);

        // Create channel for forwarding orders
        let (order_tx, order_rx) = mpsc::channel(10_000);

        // Clone for the spawned thread
        let rx_clone = receiver;

        // Spawn receiver task
        std::thread::spawn(move || {
            Self::receiver_loop(rx_clone, order_tx);
        });

        Ok((
            Self {
                _receiver: UdpReceiver::new(
                    ReceiverConfig {
                        stream_id: 0, // Dummy, not used
                        ..Default::default()
                    },
                    "127.0.0.1:0".parse().unwrap(), // Dummy address
                )
                .unwrap_or_else(|_| panic!("Failed to create dummy receiver")),
            },
            order_rx,
        ))
    }

    fn receiver_loop(receiver: UdpReceiver, order_tx: mpsc::Sender<OrderCommand>) {
        info!("UDP order receiver loop started");

        loop {
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(Some(msg)) => {
                    if let Err(e) = Self::handle_message(&msg, &order_tx) {
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
        order_tx: &mpsc::Sender<OrderCommand>,
    ) -> anyhow::Result<()> {
        // Parse the JSON payload
        let command: OrderCommand = serde_json::from_slice(&msg.payload)?;
        info!("Received order command via UDP: {:?}", command);

        // Send to channel (blocking in sync context)
        order_tx.blocking_send(command)?;

        Ok(())
    }
}

/// Configuration for UDP transport
#[derive(Debug, Clone)]
pub struct UdpTransportConfig {
    /// Address to receive orders on
    pub order_receiver_bind: SocketAddr,
    /// Address to send events to (gateway) - stored as string for lazy resolution
    pub gateway_event_addr: String,
    /// Local address to bind event sender
    pub event_sender_bind: SocketAddr,
}

impl Default for UdpTransportConfig {
    fn default() -> Self {
        Self {
            order_receiver_bind: "127.0.0.1:9100".parse().unwrap(),
            gateway_event_addr: "127.0.0.1:9101".to_string(),
            event_sender_bind: "127.0.0.1:9103".parse().unwrap(),
        }
    }
}

impl UdpTransportConfig {
    pub fn from_env() -> Self {
        // Bind addresses should resolve immediately (they're local)
        let order_receiver_bind_str = std::env::var("ORDER_RECEIVER_BIND")
            .unwrap_or_else(|_| "0.0.0.0:9100".to_string());
        let order_receiver_bind = resolve_addr(&order_receiver_bind_str);

        // Keep target address as string for lazy resolution
        let gateway_event_addr = std::env::var("GATEWAY_EVENT_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:9101".to_string());

        let event_sender_bind_str = std::env::var("EVENT_SENDER_BIND")
            .unwrap_or_else(|_| "0.0.0.0:9103".to_string());
        let event_sender_bind = resolve_addr(&event_sender_bind_str);

        Self {
            order_receiver_bind,
            gateway_event_addr,
            event_sender_bind,
        }
    }
}
