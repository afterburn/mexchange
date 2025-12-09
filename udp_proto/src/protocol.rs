use crate::error::{ProtocolError, Result};

// Protocol constants
pub const PROTOCOL_VERSION: u8 = 1;
pub const PACKET_HEADER_LEN: usize = 24;
pub const MESSAGE_HEADER_LEN: usize = 4;
pub const MAX_MTU: usize = 1400;
pub const MAX_PAYLOAD: usize = MAX_MTU - PACKET_HEADER_LEN;
pub const HEARTBEAT_INTERVAL_MS: u64 = 100;
pub const STREAM_TIMEOUT_MS: u64 = 500;

/// Message types as defined in the protocol spec
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    OrderNew = 0x01,
    OrderCancel = 0x02,
    OrderReplace = 0x03,
    MatchEvent = 0x10,
    BookSnapshot = 0x11,
    BookUpdate = 0x12,
    PositionUpdate = 0x20,
    Heartbeat = 0x30,
    Control = 0x40,
}

impl MessageType {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x01 => Some(Self::OrderNew),
            0x02 => Some(Self::OrderCancel),
            0x03 => Some(Self::OrderReplace),
            0x10 => Some(Self::MatchEvent),
            0x11 => Some(Self::BookSnapshot),
            0x12 => Some(Self::BookUpdate),
            0x20 => Some(Self::PositionUpdate),
            0x30 => Some(Self::Heartbeat),
            0x40 => Some(Self::Control),
            _ => None,
        }
    }
}

/// Message flags
#[derive(Debug, Clone, Copy, Default)]
pub struct MessageFlags(pub u8);

impl MessageFlags {
    pub const NONE: Self = Self(0);
    pub const LAST_IN_BATCH: Self = Self(0x01);
    pub const URGENT: Self = Self(0x02);
}

/// Packet header (24 bytes)
/// ```text
/// 0  - 1   : version        (u8)
/// 1  - 2   : header_len     (u8)
/// 2  - 4   : msg_count      (u16)
/// 4  - 8   : stream_id      (u32)
/// 8  - 16  : packet_seq     (u64)
/// 16 - 24  : first_msg_seq  (u64)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketHeader {
    pub version: u8,
    pub header_len: u8,
    pub msg_count: u16,
    pub stream_id: u32,
    pub packet_seq: u64,
    pub first_msg_seq: u64,
}

impl PacketHeader {
    pub fn new(stream_id: u32, packet_seq: u64, first_msg_seq: u64, msg_count: u16) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            header_len: PACKET_HEADER_LEN as u8,
            msg_count,
            stream_id,
            packet_seq,
            first_msg_seq,
        }
    }

    pub fn heartbeat(stream_id: u32, packet_seq: u64, msg_seq: u64) -> Self {
        Self::new(stream_id, packet_seq, msg_seq, 0)
    }

    /// Write header to buffer. Returns bytes written.
    #[inline]
    pub fn write_to(&self, buf: &mut [u8]) -> Result<usize> {
        if buf.len() < PACKET_HEADER_LEN {
            return Err(ProtocolError::BufferTooSmall {
                needed: PACKET_HEADER_LEN,
                available: buf.len(),
            });
        }

        buf[0] = self.version;
        buf[1] = self.header_len;
        buf[2..4].copy_from_slice(&self.msg_count.to_le_bytes());
        buf[4..8].copy_from_slice(&self.stream_id.to_le_bytes());
        buf[8..16].copy_from_slice(&self.packet_seq.to_le_bytes());
        buf[16..24].copy_from_slice(&self.first_msg_seq.to_le_bytes());

        Ok(PACKET_HEADER_LEN)
    }

    /// Read header from buffer.
    #[inline]
    pub fn read_from(buf: &[u8]) -> Result<Self> {
        if buf.len() < PACKET_HEADER_LEN {
            return Err(ProtocolError::BufferTooSmall {
                needed: PACKET_HEADER_LEN,
                available: buf.len(),
            });
        }

        let version = buf[0];
        if version != PROTOCOL_VERSION {
            return Err(ProtocolError::InvalidVersion {
                expected: PROTOCOL_VERSION,
                got: version,
            });
        }

        let header_len = buf[1];
        if header_len != PACKET_HEADER_LEN as u8 {
            return Err(ProtocolError::InvalidHeaderLength {
                expected: PACKET_HEADER_LEN as u8,
                got: header_len,
            });
        }

        Ok(Self {
            version,
            header_len,
            msg_count: u16::from_le_bytes([buf[2], buf[3]]),
            stream_id: u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]),
            packet_seq: u64::from_le_bytes([
                buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15],
            ]),
            first_msg_seq: u64::from_le_bytes([
                buf[16], buf[17], buf[18], buf[19], buf[20], buf[21], buf[22], buf[23],
            ]),
        })
    }

    pub fn is_heartbeat(&self) -> bool {
        self.msg_count == 0
    }
}

/// Message header (4 bytes)
/// ```text
/// 0 - 1 : msg_type   (u8)
/// 1 - 2 : flags      (u8)
/// 2 - 4 : msg_len    (u16)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageHeader {
    pub msg_type: u8,
    pub flags: u8,
    pub msg_len: u16,
}

impl MessageHeader {
    pub fn new(msg_type: MessageType, flags: MessageFlags, payload_len: u16) -> Self {
        Self {
            msg_type: msg_type as u8,
            flags: flags.0,
            msg_len: payload_len,
        }
    }

    #[inline]
    pub fn write_to(&self, buf: &mut [u8]) -> Result<usize> {
        if buf.len() < MESSAGE_HEADER_LEN {
            return Err(ProtocolError::BufferTooSmall {
                needed: MESSAGE_HEADER_LEN,
                available: buf.len(),
            });
        }

        buf[0] = self.msg_type;
        buf[1] = self.flags;
        buf[2..4].copy_from_slice(&self.msg_len.to_le_bytes());

        Ok(MESSAGE_HEADER_LEN)
    }

    #[inline]
    pub fn read_from(buf: &[u8]) -> Result<Self> {
        if buf.len() < MESSAGE_HEADER_LEN {
            return Err(ProtocolError::BufferTooSmall {
                needed: MESSAGE_HEADER_LEN,
                available: buf.len(),
            });
        }

        Ok(Self {
            msg_type: buf[0],
            flags: buf[1],
            msg_len: u16::from_le_bytes([buf[2], buf[3]]),
        })
    }

    pub fn message_type(&self) -> Option<MessageType> {
        MessageType::from_u8(self.msg_type)
    }

    pub fn total_len(&self) -> usize {
        MESSAGE_HEADER_LEN + self.msg_len as usize
    }
}

/// A complete message with header and payload
#[derive(Debug, Clone)]
pub struct Message {
    pub header: MessageHeader,
    pub payload: Vec<u8>,
    pub seq: u64,
}

impl Message {
    pub fn new(msg_type: MessageType, payload: Vec<u8>) -> Self {
        Self {
            header: MessageHeader::new(msg_type, MessageFlags::NONE, payload.len() as u16),
            payload,
            seq: 0,
        }
    }

    pub fn with_seq(mut self, seq: u64) -> Self {
        self.seq = seq;
        self
    }

    pub fn message_type(&self) -> Option<MessageType> {
        self.header.message_type()
    }
}

/// A parsed packet containing header and messages
#[derive(Debug)]
pub struct Packet {
    pub header: PacketHeader,
    pub messages: Vec<Message>,
}

impl Packet {
    /// Parse a packet from raw bytes
    pub fn parse(buf: &[u8]) -> Result<Self> {
        let header = PacketHeader::read_from(buf)?;
        let mut messages = Vec::with_capacity(header.msg_count as usize);
        let mut offset = PACKET_HEADER_LEN;

        for i in 0..header.msg_count {
            if offset >= buf.len() {
                break;
            }

            let msg_header = MessageHeader::read_from(&buf[offset..])?;
            offset += MESSAGE_HEADER_LEN;

            let payload_end = offset + msg_header.msg_len as usize;
            if payload_end > buf.len() {
                return Err(ProtocolError::BufferTooSmall {
                    needed: payload_end,
                    available: buf.len(),
                });
            }

            let payload = buf[offset..payload_end].to_vec();
            offset = payload_end;

            messages.push(Message {
                header: msg_header,
                payload,
                seq: header.first_msg_seq + i as u64,
            });
        }

        Ok(Self { header, messages })
    }
}

/// Buffer for building packets
pub struct PacketBuilder {
    buf: Vec<u8>,
    stream_id: u32,
    packet_seq: u64,
    first_msg_seq: u64,
    msg_count: u16,
    write_offset: usize,
}

impl PacketBuilder {
    pub fn new(stream_id: u32, packet_seq: u64, first_msg_seq: u64) -> Self {
        let buf = vec![0u8; MAX_MTU];
        Self {
            buf,
            stream_id,
            packet_seq,
            first_msg_seq,
            msg_count: 0,
            write_offset: PACKET_HEADER_LEN,
        }
    }

    pub fn remaining_capacity(&self) -> usize {
        MAX_MTU - self.write_offset
    }

    pub fn is_empty(&self) -> bool {
        self.msg_count == 0
    }

    pub fn msg_count(&self) -> u16 {
        self.msg_count
    }

    /// Try to add a message. Returns false if it doesn't fit.
    pub fn try_add_message(&mut self, msg_type: MessageType, payload: &[u8]) -> bool {
        let needed = MESSAGE_HEADER_LEN + payload.len();
        if needed > self.remaining_capacity() {
            return false;
        }

        let header = MessageHeader::new(msg_type, MessageFlags::NONE, payload.len() as u16);
        header.write_to(&mut self.buf[self.write_offset..]).unwrap();
        self.write_offset += MESSAGE_HEADER_LEN;

        self.buf[self.write_offset..self.write_offset + payload.len()].copy_from_slice(payload);
        self.write_offset += payload.len();
        self.msg_count += 1;

        true
    }

    /// Finalize and return the packet bytes
    pub fn finish(mut self) -> Vec<u8> {
        let header = PacketHeader::new(
            self.stream_id,
            self.packet_seq,
            self.first_msg_seq,
            self.msg_count,
        );
        header.write_to(&mut self.buf).unwrap();
        self.buf.truncate(self.write_offset);
        self.buf
    }

    /// Create a heartbeat packet
    pub fn heartbeat(stream_id: u32, packet_seq: u64, msg_seq: u64) -> Vec<u8> {
        let mut buf = vec![0u8; PACKET_HEADER_LEN];
        let header = PacketHeader::heartbeat(stream_id, packet_seq, msg_seq);
        header.write_to(&mut buf).unwrap();
        buf
    }
}
