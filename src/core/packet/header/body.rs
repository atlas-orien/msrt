//! Packet header body variants.

use super::{AckHeader, DataHeader, LogHeader, PingHeader, PongHeader};

/// Kind-specific packet header data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketHeaderBody {
    /// DATA fragment.
    Data {
        /// DATA header fields.
        header: DataHeader,
    },
    /// LOG fragment.
    Log {
        /// LOG header fields.
        header: LogHeader,
    },
    /// Single-packet acknowledgement.
    Ack {
        /// ACK header fields.
        header: AckHeader,
    },
    /// Liveness probe.
    Ping {
        /// PING header fields.
        header: PingHeader,
    },
    /// Liveness response.
    Pong {
        /// PONG header fields.
        header: PongHeader,
    },
}
