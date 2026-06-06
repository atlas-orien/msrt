//! Packet header body variants.

use super::{AckHeader, DataHeader, LogHeader, PingHeader, PongHeader};
use crate::core::{ChannelId, MessageId};

/// Kind-specific packet header data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketHeaderBody {
    /// Data-like fragment using the legacy channel field.
    Data {
        /// Logical channel carrying this message fragment.
        channel_id: ChannelId,
        /// DATA header fields.
        header: DataHeader,
    },
    /// Log fragment using the legacy channel field.
    Log {
        /// Logical channel carrying this log fragment.
        channel_id: ChannelId,
        /// LOG header fields.
        header: LogHeader,
    },
    /// Single-packet acknowledgement using the legacy channel field.
    Ack {
        /// Logical channel from the acknowledged packet key.
        channel_id: ChannelId,
        /// ACK header fields.
        header: AckHeader,
    },
    /// Liveness probe.
    Ping {
        /// PING header fields.
        header: PingHeader,
        /// Legacy message id kept until liveness wire format is shortened.
        legacy_message_id: MessageId,
    },
    /// Liveness response.
    Pong {
        /// PONG header fields.
        header: PongHeader,
        /// Legacy message id kept until liveness wire format is shortened.
        legacy_message_id: MessageId,
    },
}
