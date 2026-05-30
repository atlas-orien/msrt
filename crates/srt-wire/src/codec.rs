//! Wire codec boundaries.

pub mod decoder;
pub mod encoder;

pub use decoder::{DecodeOutcome, Decoder};
pub use encoder::{EncodeTarget, Encoder};
