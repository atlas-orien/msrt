#![no_std]
#![doc = "Core protocol primitives for Serial Realtime Transport."]

pub mod frame;
pub mod packet;

pub use frame::{
    AckFrame, AckRange, ChannelId, Frame, FrameKind, MAX_ACK_RANGES, MessageData, MessageFlags,
    MessageFrame, MessageId,
};
pub use packet::{Flags, Packet, PacketHeader, PacketNumber, PacketPayload, PacketType};
pub use srt_error::{Error, ErrorKind, Result};
