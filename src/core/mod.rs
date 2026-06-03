#![doc = "Core protocol primitives for MSRT."]

pub mod ack;
pub mod message;
pub mod packet;

pub use crate::error::{Error, ErrorKind, Result};
pub use ack::{Ack, AckRange, MAX_ACK_RANGES};
pub use message::{ChannelId, MessageFlags, MessageId};
pub use packet::{Flags, Packet, PacketHeader, PacketNumber, PacketPayload, PacketType};
