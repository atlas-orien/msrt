//! Decoder boundary for serial-like byte streams.

pub mod buffer;
pub mod state;

pub use buffer::DecoderBuffer;
pub use state::DecodeStatus;
