#![no_std]
#![doc = "Core protocol primitives for Serial Realtime Transport."]

pub mod flags;
pub mod frame;
pub mod id;
pub mod packet;
pub mod seq;

pub use flags::Flags;
pub use frame::FrameKind;
pub use id::StreamId;
pub use packet::PacketKind;
pub use seq::Seq;
pub use srt_error::{Error, ErrorKind, Result};
