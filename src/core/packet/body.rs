//! Packet body variants.

use super::{AckHeader, DataHeader, PacketPayload, PingHeader, PongHeader};

/// Kind-specific borrowed packet content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketBody<'a> {
    /// DATA packet content.
    Data {
        /// DATA header fields.
        header: DataHeader,
        /// Borrowed message fragment bytes.
        payload: PacketPayload<'a>,
    },
    /// LOG packet content.
    Log {
        /// LOG header fields.
        header: super::LogHeader,
        /// Borrowed log fragment bytes.
        payload: PacketPayload<'a>,
    },
    /// ACK packet content.
    Ack {
        /// ACK header fields.
        header: AckHeader,
    },
    /// PING packet content.
    Ping {
        /// PING header fields.
        header: PingHeader,
    },
    /// PONG packet content.
    Pong {
        /// PONG header fields.
        header: PongHeader,
    },
}
