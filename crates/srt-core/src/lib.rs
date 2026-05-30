#![no_std]
#![doc = "Core protocol primitives for Serial Realtime Transport."]

pub mod frame;
pub mod packet;

pub use frame::{
    AckFrame, Frame, FrameKind, MessageId, PingFrame, ResetStreamFrame, StreamData, StreamFlags,
    StreamFrame, StreamId,
};
pub use packet::{Flags, Packet, PacketHeader, PacketNumber, PacketPayload, PacketType};
pub use srt_error::{Error, ErrorKind, Result};
