//! Low-latency UDP messaging protocol for exchange services.
//!
//! This crate implements a custom UDP protocol designed for internal
//! low-latency messaging between exchange services.

mod protocol;
mod sender;
mod receiver;
mod error;
mod generated;
pub mod binary;

pub use protocol::*;
pub use sender::*;
pub use receiver::*;
pub use error::*;
pub use generated::market_data_generated::market_data as fb;

#[cfg(test)]
mod tests;
