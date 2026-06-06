#![doc = "Core protocol primitives for MSRT."]

pub mod message;
pub mod packet;

pub use crate::error::{Error, ErrorKind, Result};
pub use message::{ChannelId, MessageId};
pub use packet::{Flags, Packet, PacketHeader, PacketIndex, PacketKey, PacketPayload, PacketType};
