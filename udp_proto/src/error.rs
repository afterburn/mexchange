use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("Buffer too small: need {needed} bytes, have {available}")]
    BufferTooSmall { needed: usize, available: usize },

    #[error("Invalid version: expected {expected}, got {got}")]
    InvalidVersion { expected: u8, got: u8 },

    #[error("Invalid header length: expected {expected}, got {got}")]
    InvalidHeaderLength { expected: u8, got: u8 },

    #[error("Packet too large: {size} bytes exceeds MTU of {mtu}")]
    PacketTooLarge { size: usize, mtu: usize },

    #[error("Message too large: {size} bytes exceeds max of {max}")]
    MessageTooLarge { size: usize, max: usize },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Gap detected: expected seq {expected}, got {got}")]
    GapDetected { expected: u64, got: u64 },

    #[error("Duplicate packet: seq {seq}")]
    DuplicatePacket { seq: u64 },

    #[error("Stream timeout: no packets for {ms}ms")]
    StreamTimeout { ms: u64 },

    #[error("Channel closed")]
    ChannelClosed,
}

pub type Result<T> = std::result::Result<T, ProtocolError>;
