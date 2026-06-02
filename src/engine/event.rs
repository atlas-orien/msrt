//! Engine events.

use crate::core::{ChannelId, PacketNumber};
use crate::reliability::{MessageKey, PacketReliabilityEvent};

use crate::engine::time::Instant;

/// Lightweight event kind without associated data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EngineEventKind {
    /// A full message can be delivered to the caller.
    MessageReceived,
    /// Protocol bytes should be written to the lower link.
    LinkWrite,
    /// An ACK response should be generated.
    AckRequired,
    /// A packet should be retransmitted.
    Retransmit,
    /// The engine wants to be ticked later.
    WakeAt,
    /// A reliability event occurred.
    Reliability,
}

/// Events emitted by the protocol engine to its embedding environment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EngineEvent {
    /// A complete message is available for a channel.
    MessageReceived {
        /// Channel that owns the message.
        channel_id: ChannelId,
        /// Message identity on the channel.
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
    /// The engine needs to be ticked again at a later instant.
    WakeAt(Instant),
    /// A packet-level reliability event occurred.
    Reliability(PacketReliabilityEvent),
}

impl EngineEvent {
    /// Returns this event's kind.
    #[must_use]
    pub const fn kind(self) -> EngineEventKind {
        match self {
            Self::MessageReceived { .. } => EngineEventKind::MessageReceived,
            Self::LinkWrite => EngineEventKind::LinkWrite,
            Self::AckRequired { .. } => EngineEventKind::AckRequired,
            Self::Retransmit { .. } => EngineEventKind::Retransmit,
            Self::WakeAt(_) => EngineEventKind::WakeAt,
            Self::Reliability(_) => EngineEventKind::Reliability,
        }
    }
}
