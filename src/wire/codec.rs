//! Wire codec boundaries.

pub mod decoder;
pub mod encoder;
pub mod streaming;

pub use decoder::{DecodeOutcome, Decoder};
pub use encoder::{EncodeTarget, Encoder};
pub use streaming::{StreamDecodeOutcome, StreamingDecoder};
