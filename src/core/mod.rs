#![doc = "Core protocol primitives for MSRT."]

pub mod frame;
pub mod packet;

pub use crate::error::{Error, ErrorKind, Result};
pub use frame::{
    AckFrame, AckRange, ChannelId, Frame, FrameKind, MAX_ACK_RANGES, MessageData, MessageFlags,
    MessageFrame, MessageId,
};
pub use packet::{Flags, Packet, PacketHeader, PacketNumber, PacketPayload, PacketType};
