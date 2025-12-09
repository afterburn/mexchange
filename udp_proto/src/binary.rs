//! Binary encoding/decoding for market events using FlatBuffers.
//!
//! Provides a clean API for encoding/decoding market events with zero-copy deserialization.

use crate::fb;
use flatbuffers::FlatBufferBuilder;
use uuid::Uuid;

/// Side of an order (Bid or Ask)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Bid,
    Ask,
}

impl From<Side> for fb::Side {
    fn from(s: Side) -> Self {
        match s {
            Side::Bid => fb::Side::Bid,
            Side::Ask => fb::Side::Ask,
        }
    }
}

/// Error for invalid enum values with the raw value for debugging
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidEnumValue(pub i8);

impl std::fmt::Display for InvalidEnumValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Invalid enum value: {}", self.0)
    }
}

impl std::error::Error for InvalidEnumValue {}

impl TryFrom<fb::Side> for Side {
    type Error = InvalidEnumValue;

    fn try_from(s: fb::Side) -> Result<Self, Self::Error> {
        match s {
            fb::Side::Bid => Ok(Side::Bid),
            fb::Side::Ask => Ok(Side::Ask),
            _ => Err(InvalidEnumValue(s.0)),
        }
    }
}

/// Delta action for orderbook updates
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeltaAction {
    Add,
    Update,
    Remove,
}

impl From<DeltaAction> for fb::DeltaAction {
    fn from(a: DeltaAction) -> Self {
        match a {
            DeltaAction::Add => fb::DeltaAction::Add,
            DeltaAction::Update => fb::DeltaAction::Update,
            DeltaAction::Remove => fb::DeltaAction::Remove,
        }
    }
}

impl TryFrom<fb::DeltaAction> for DeltaAction {
    type Error = InvalidEnumValue;

    fn try_from(a: fb::DeltaAction) -> Result<Self, Self::Error> {
        match a {
            fb::DeltaAction::Add => Ok(DeltaAction::Add),
            fb::DeltaAction::Update => Ok(DeltaAction::Update),
            fb::DeltaAction::Remove => Ok(DeltaAction::Remove),
            _ => Err(InvalidEnumValue(a.0)),
        }
    }
}

/// A price level in the orderbook
/// Note: Uses f64 for wire format efficiency. The matching engine uses rust_decimal
/// internally for precise calculations. f64 is acceptable here since this is only
/// used for broadcasting market data to clients, not for order matching.
#[derive(Debug, Clone, Copy)]
pub struct PriceLevel {
    pub price: f64,
    pub quantity: f64,
}

/// A delta change to a price level
#[derive(Debug, Clone, Copy)]
pub struct LevelDelta {
    pub action: DeltaAction,
    pub side: Side,
    pub price: f64,
    pub quantity: f64,
}

/// Market event types
#[derive(Debug, Clone)]
pub enum MarketEvent {
    Fill {
        symbol: String,
        /// Order ID (UUID) for the buy side
        buy_order_id: Uuid,
        /// Order ID (UUID) for the sell side
        sell_order_id: Uuid,
        price: f64,
        quantity: f64,
        timestamp: u64,
    },
    OrderBookSnapshot {
        symbol: String,
        sequence: u64,
        bids: Vec<PriceLevel>,
        asks: Vec<PriceLevel>,
    },
    OrderBookDelta {
        symbol: String,
        sequence: u64,
        deltas: Vec<LevelDelta>,
    },
    /// Order was cancelled (including unfilled market orders)
    OrderCancelled {
        /// Order ID (UUID)
        order_id: Uuid,
        /// How much was filled before cancellation
        filled_quantity: f64,
    },
    /// Order was 100% filled
    OrderFilled {
        /// Order ID (UUID)
        order_id: Uuid,
    },
}

/// Helper to convert Uuid to FlatBuffer Uuid struct
fn uuid_to_fb(uuid: &Uuid) -> fb::Uuid {
    let bytes = uuid.as_u128();
    let high = (bytes >> 64) as u64;
    let low = bytes as u64;
    fb::Uuid::new(high, low)
}

/// Helper to convert FlatBuffer Uuid struct to Uuid
fn fb_to_uuid(fb_uuid: &fb::Uuid) -> Uuid {
    let high = fb_uuid.high() as u128;
    let low = fb_uuid.low() as u128;
    let bytes = (high << 64) | low;
    Uuid::from_u128(bytes)
}

/// Encoder for market events
pub struct MarketEventEncoder {
    builder: FlatBufferBuilder<'static>,
}

impl MarketEventEncoder {
    pub fn new() -> Self {
        Self {
            builder: FlatBufferBuilder::with_capacity(1024),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            builder: FlatBufferBuilder::with_capacity(capacity),
        }
    }

    /// Encode a market event and return the binary data.
    /// The encoder is reset after encoding.
    pub fn encode(&mut self, event: &MarketEvent) -> &[u8] {
        self.builder.reset();

        let payload_type;
        let payload_offset;

        match event {
            MarketEvent::Fill {
                symbol,
                buy_order_id,
                sell_order_id,
                price,
                quantity,
                timestamp,
            } => {
                let symbol_offset = self.builder.create_string(symbol);

                // Convert UUIDs to FlatBuffer format
                let buy_uuid = uuid_to_fb(buy_order_id);
                let sell_uuid = uuid_to_fb(sell_order_id);

                let fill = fb::Fill::create(
                    &mut self.builder,
                    &fb::FillArgs {
                        symbol: Some(symbol_offset),
                        buy_order_id: Some(&buy_uuid),
                        sell_order_id: Some(&sell_uuid),
                        price: *price,
                        quantity: *quantity,
                        timestamp: *timestamp,
                    },
                );
                payload_type = fb::EventPayload::Fill;
                payload_offset = fill.as_union_value();
            }
            MarketEvent::OrderBookSnapshot {
                symbol,
                sequence,
                bids,
                asks,
            } => {
                let symbol_offset = self.builder.create_string(symbol);

                // Create bid levels
                let bids_vec: Vec<fb::PriceLevel> = bids
                    .iter()
                    .map(|l| fb::PriceLevel::new(l.price, l.quantity))
                    .collect();
                let bids_offset = self.builder.create_vector(&bids_vec);

                // Create ask levels
                let asks_vec: Vec<fb::PriceLevel> = asks
                    .iter()
                    .map(|l| fb::PriceLevel::new(l.price, l.quantity))
                    .collect();
                let asks_offset = self.builder.create_vector(&asks_vec);

                let snapshot = fb::OrderBookSnapshot::create(
                    &mut self.builder,
                    &fb::OrderBookSnapshotArgs {
                        symbol: Some(symbol_offset),
                        sequence: *sequence,
                        bids: Some(bids_offset),
                        asks: Some(asks_offset),
                    },
                );
                payload_type = fb::EventPayload::OrderBookSnapshot;
                payload_offset = snapshot.as_union_value();
            }
            MarketEvent::OrderBookDelta {
                symbol,
                sequence,
                deltas,
            } => {
                let symbol_offset = self.builder.create_string(symbol);

                // Create delta levels
                let deltas_vec: Vec<fb::LevelDelta> = deltas
                    .iter()
                    .map(|d| fb::LevelDelta::new(d.action.into(), d.side.into(), d.price, d.quantity))
                    .collect();
                let deltas_offset = self.builder.create_vector(&deltas_vec);

                let delta = fb::OrderBookDelta::create(
                    &mut self.builder,
                    &fb::OrderBookDeltaArgs {
                        symbol: Some(symbol_offset),
                        sequence: *sequence,
                        deltas: Some(deltas_offset),
                    },
                );
                payload_type = fb::EventPayload::OrderBookDelta;
                payload_offset = delta.as_union_value();
            }
            MarketEvent::OrderCancelled {
                order_id,
                filled_quantity,
            } => {
                let order_uuid = uuid_to_fb(order_id);

                let cancelled = fb::OrderCancelled::create(
                    &mut self.builder,
                    &fb::OrderCancelledArgs {
                        order_id: Some(&order_uuid),
                        filled_quantity: *filled_quantity,
                    },
                );
                payload_type = fb::EventPayload::OrderCancelled;
                payload_offset = cancelled.as_union_value();
            }
            MarketEvent::OrderFilled { order_id } => {
                let order_uuid = uuid_to_fb(order_id);

                let filled = fb::OrderFilled::create(
                    &mut self.builder,
                    &fb::OrderFilledArgs {
                        order_id: Some(&order_uuid),
                    },
                );
                payload_type = fb::EventPayload::OrderFilled;
                payload_offset = filled.as_union_value();
            }
        }

        let market_event = fb::MarketEvent::create(
            &mut self.builder,
            &fb::MarketEventArgs {
                payload_type,
                payload: Some(payload_offset),
            },
        );

        self.builder.finish(market_event, None);
        self.builder.finished_data()
    }

    /// Get the size of the last encoded message
    pub fn last_encoded_size(&self) -> usize {
        self.builder.finished_data().len()
    }
}

impl Default for MarketEventEncoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Decode a market event from binary data (zero-copy)
pub fn decode_market_event(data: &[u8]) -> Result<MarketEvent, &'static str> {
    let event = fb::root_as_market_event(data).map_err(|_| "Invalid FlatBuffer")?;

    match event.payload_type() {
        fb::EventPayload::Fill => {
            let fill = event.payload_as_fill().ok_or("Missing Fill payload")?;

            // Symbol is required
            let symbol = fill.symbol().ok_or("Missing symbol in Fill")?;
            if symbol.is_empty() {
                return Err("Empty symbol in Fill");
            }

            // Decode UUIDs (required)
            let buy_order_id = fill
                .buy_order_id()
                .map(|u| fb_to_uuid(u))
                .ok_or("Missing buy_order_id in Fill")?;
            let sell_order_id = fill
                .sell_order_id()
                .map(|u| fb_to_uuid(u))
                .ok_or("Missing sell_order_id in Fill")?;

            Ok(MarketEvent::Fill {
                symbol: symbol.to_string(),
                buy_order_id,
                sell_order_id,
                price: fill.price(),
                quantity: fill.quantity(),
                timestamp: fill.timestamp(),
            })
        }
        fb::EventPayload::OrderBookSnapshot => {
            let snapshot = event
                .payload_as_order_book_snapshot()
                .ok_or("Missing OrderBookSnapshot payload")?;

            // Symbol is required
            let symbol = snapshot.symbol().ok_or("Missing symbol in OrderBookSnapshot")?;
            if symbol.is_empty() {
                return Err("Empty symbol in OrderBookSnapshot");
            }

            let bids = snapshot
                .bids()
                .map(|v| {
                    v.iter()
                        .map(|l| PriceLevel {
                            price: l.price(),
                            quantity: l.quantity(),
                        })
                        .collect()
                })
                .unwrap_or_default();

            let asks = snapshot
                .asks()
                .map(|v| {
                    v.iter()
                        .map(|l| PriceLevel {
                            price: l.price(),
                            quantity: l.quantity(),
                        })
                        .collect()
                })
                .unwrap_or_default();

            Ok(MarketEvent::OrderBookSnapshot {
                symbol: symbol.to_string(),
                sequence: snapshot.sequence(),
                bids,
                asks,
            })
        }
        fb::EventPayload::OrderBookDelta => {
            let delta_msg = event
                .payload_as_order_book_delta()
                .ok_or("Missing OrderBookDelta payload")?;

            // Symbol is required
            let symbol = delta_msg.symbol().ok_or("Missing symbol in OrderBookDelta")?;
            if symbol.is_empty() {
                return Err("Empty symbol in OrderBookDelta");
            }

            let deltas_result: Result<Vec<LevelDelta>, _> = delta_msg
                .deltas()
                .map(|v| {
                    v.iter()
                        .map(|d| {
                            Ok(LevelDelta {
                                action: d.action().try_into().map_err(|_| "Invalid delta action")?,
                                side: d.side().try_into().map_err(|_| "Invalid side")?,
                                price: d.price(),
                                quantity: d.quantity(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_else(|| Ok(Vec::new()));

            Ok(MarketEvent::OrderBookDelta {
                symbol: symbol.to_string(),
                sequence: delta_msg.sequence(),
                deltas: deltas_result?,
            })
        }
        fb::EventPayload::OrderCancelled => {
            let cancelled = event
                .payload_as_order_cancelled()
                .ok_or("Missing OrderCancelled payload")?;

            // Decode order_id UUID (required)
            let order_id = cancelled
                .order_id()
                .map(|u| fb_to_uuid(u))
                .ok_or("Missing order_id in OrderCancelled")?;

            Ok(MarketEvent::OrderCancelled {
                order_id,
                filled_quantity: cancelled.filled_quantity(),
            })
        }
        fb::EventPayload::OrderFilled => {
            let filled = event
                .payload_as_order_filled()
                .ok_or("Missing OrderFilled payload")?;

            // Decode order_id UUID (required)
            let order_id = filled
                .order_id()
                .map(|u| fb_to_uuid(u))
                .ok_or("Missing order_id in OrderFilled")?;

            Ok(MarketEvent::OrderFilled { order_id })
        }
        _ => Err("Unknown event type"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_snapshot() {
        let mut encoder = MarketEventEncoder::new();

        let event = MarketEvent::OrderBookSnapshot {
            symbol: "BTC/USD".to_string(),
            sequence: 12345,
            bids: vec![
                PriceLevel { price: 50000.0, quantity: 1.5 },
                PriceLevel { price: 49999.0, quantity: 2.0 },
            ],
            asks: vec![
                PriceLevel { price: 50001.0, quantity: 1.0 },
                PriceLevel { price: 50002.0, quantity: 3.0 },
            ],
        };

        let data = encoder.encode(&event);
        println!("Snapshot size: {} bytes", data.len());

        let decoded = decode_market_event(data).unwrap();

        match decoded {
            MarketEvent::OrderBookSnapshot { symbol, sequence, bids, asks } => {
                assert_eq!(symbol, "BTC/USD");
                assert_eq!(sequence, 12345);
                assert_eq!(bids.len(), 2);
                assert_eq!(asks.len(), 2);
                assert_eq!(bids[0].price, 50000.0);
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_encode_decode_delta() {
        let mut encoder = MarketEventEncoder::new();

        let event = MarketEvent::OrderBookDelta {
            symbol: "BTC/USD".to_string(),
            sequence: 12346,
            deltas: vec![
                LevelDelta {
                    action: DeltaAction::Update,
                    side: Side::Bid,
                    price: 50000.0,
                    quantity: 2.5,
                },
                LevelDelta {
                    action: DeltaAction::Remove,
                    side: Side::Ask,
                    price: 50001.0,
                    quantity: 0.0,
                },
            ],
        };

        let data = encoder.encode(&event);
        println!("Delta size: {} bytes", data.len());

        let decoded = decode_market_event(data).unwrap();

        match decoded {
            MarketEvent::OrderBookDelta { symbol, sequence, deltas } => {
                assert_eq!(symbol, "BTC/USD");
                assert_eq!(sequence, 12346);
                assert_eq!(deltas.len(), 2);
                assert_eq!(deltas[0].action, DeltaAction::Update);
                assert_eq!(deltas[1].action, DeltaAction::Remove);
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_10_level_snapshot_size() {
        let mut encoder = MarketEventEncoder::new();

        let bids: Vec<PriceLevel> = (0..10)
            .map(|i| PriceLevel {
                price: 50000.0 - i as f64,
                quantity: 1.0 + i as f64 * 0.1,
            })
            .collect();

        let asks: Vec<PriceLevel> = (0..10)
            .map(|i| PriceLevel {
                price: 50001.0 + i as f64,
                quantity: 1.0 + i as f64 * 0.1,
            })
            .collect();

        let event = MarketEvent::OrderBookSnapshot {
            symbol: "KCN/EUR".to_string(),
            sequence: 1,
            bids,
            asks,
        };

        let data = encoder.encode(&event);
        println!("10-level snapshot size: {} bytes", data.len());

        // Should be well under 1400 byte MTU
        assert!(data.len() < 500, "Snapshot too large: {} bytes", data.len());
    }

    #[test]
    fn test_fill_encoding() {
        let mut encoder = MarketEventEncoder::new();

        let buy_uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let sell_uuid = Uuid::parse_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap();

        let event = MarketEvent::Fill {
            symbol: "BTC/USD".to_string(),
            buy_order_id: buy_uuid,
            sell_order_id: sell_uuid,
            price: 50000.0,
            quantity: 0.5,
            timestamp: 1699999999000,
        };

        let data = encoder.encode(&event);
        println!("Fill size: {} bytes", data.len());

        let decoded = decode_market_event(data).unwrap();

        match decoded {
            MarketEvent::Fill {
                symbol,
                buy_order_id,
                sell_order_id,
                price,
                quantity,
                timestamp,
            } => {
                assert_eq!(symbol, "BTC/USD");
                assert_eq!(buy_order_id, buy_uuid);
                assert_eq!(sell_order_id, sell_uuid);
                assert_eq!(price, 50000.0);
                assert_eq!(quantity, 0.5);
                assert_eq!(timestamp, 1699999999000);
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_order_cancelled_encoding() {
        let mut encoder = MarketEventEncoder::new();

        let order_uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();

        let event = MarketEvent::OrderCancelled {
            order_id: order_uuid,
            filled_quantity: 5.5,
        };

        let data = encoder.encode(&event);
        println!("OrderCancelled size: {} bytes", data.len());

        let decoded = decode_market_event(data).unwrap();

        match decoded {
            MarketEvent::OrderCancelled {
                order_id,
                filled_quantity,
            } => {
                assert_eq!(order_id, order_uuid);
                assert_eq!(filled_quantity, 5.5);
            }
            _ => panic!("Wrong event type"),
        }
    }
}
