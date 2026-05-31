#![no_std]
#![doc = "Core protocol primitives for Serial Realtime Transport."]

pub mod frame;
pub mod packet;

pub use frame::{
    AckFrame, ChannelId, Frame, FrameKind, MessageData, MessageFlags, MessageFrame, MessageId,
};
pub use packet::{Flags, Packet, PacketHeader, PacketNumber, PacketPayload, PacketType};
pub use srt_error::{Error, ErrorKind, Result};
