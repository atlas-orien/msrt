#![no_std]
#![doc = "Core protocol primitives for Serial Realtime Transport."]

pub mod flags;
pub mod id;
pub mod packet;
pub mod seq;

pub use flags::Flags;
pub use id::StreamId;
pub use packet::{Packet, PacketHeader, PacketKind};
pub use seq::Seq;
pub use srt_error::{Error, ErrorKind, Result};
