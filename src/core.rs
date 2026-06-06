#![doc = "Core protocol primitives for MSRT."]

pub mod message;
pub mod packet;

pub use crate::error::{Error, ErrorKind, Result};
pub use message::MessageId;
pub use packet::{
    ACK_PACKET_HEADER_LEN, AckHeader, DATA_PACKET_HEADER_LEN, DataHeader, Flags,
    LIVENESS_PACKET_HEADER_LEN, LOG_PACKET_HEADER_LEN, LogHeader, Packet, PacketBody, PacketHeader,
    PacketIndex, PacketKey, PacketPayload, PacketType, PingHeader, PongHeader,
};
