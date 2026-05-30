#![no_std]
#![doc = "No-std facade crate for the Serial Realtime Transport protocol."]

/// Core protocol primitives.
pub use srt_core as core;
/// Protocol engine boundaries.
pub use srt_engine as engine;
/// Shared protocol errors.
pub use srt_error as error;
/// Reliability policy boundaries.
pub use srt_reliability as reliability;
/// Wire envelope boundaries.
pub use srt_wire as wire;
