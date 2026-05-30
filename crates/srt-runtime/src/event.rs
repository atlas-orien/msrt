//! Runtime events.

use srt_core::{PacketNumber, StreamId};
use srt_reliability::{MessageKey, PacketReliabilityEvent};

use crate::time::Instant;

/// Lightweight event kind without associated data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeEventKind {
    /// A full message can be delivered to the caller.
    MessageReceived,
    /// Protocol bytes should be written to the lower link.
    LinkWrite,
    /// An ACK response should be generated.
    AckRequired,
    /// A packet should be retransmitted.
    Retransmit,
    /// The runtime wants to be ticked later.
    WakeAt,
    /// A reliability event occurred.
    Reliability,
}

/// Events emitted by the protocol runtime to its embedding environment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeEvent {
    /// A complete message is available for a stream.
    MessageReceived {
        /// Stream that owns the message.
        stream_id: StreamId,
        /// Message identity on the stream.
        message_key: MessageKey,
    },
    /// A protocol response should be written to the lower link.
    LinkWrite,
    /// An ACK response should be generated for a packet.
    AckRequired {
        /// Packet number that should be acknowledged.
        packet_number: PacketNumber,
    },
    /// A retransmission became due.
    Retransmit {
        /// Packet number selected for retransmission.
        packet_number: PacketNumber,
    },
    /// The runtime needs to be ticked again at a later instant.
    WakeAt(Instant),
    /// A packet-level reliability event occurred.
    Reliability(PacketReliabilityEvent),
}

impl RuntimeEvent {
    /// Returns this event's kind.
    #[must_use]
    pub const fn kind(self) -> RuntimeEventKind {
        match self {
            Self::MessageReceived { .. } => RuntimeEventKind::MessageReceived,
            Self::LinkWrite => RuntimeEventKind::LinkWrite,
            Self::AckRequired { .. } => RuntimeEventKind::AckRequired,
            Self::Retransmit { .. } => RuntimeEventKind::Retransmit,
            Self::WakeAt(_) => RuntimeEventKind::WakeAt,
            Self::Reliability(_) => RuntimeEventKind::Reliability,
        }
    }
}
