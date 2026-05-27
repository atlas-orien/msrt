#![no_std]
#![doc = "Core protocol primitives for Serial Realtime Transport."]

pub mod packet;

pub use packet::{Flags, Packet, PacketHeader, PacketKind, Payload, Seq, StreamId};
pub use srt_error::{Error, ErrorKind, Result};
