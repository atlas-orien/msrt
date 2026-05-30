//! Packet reliability events emitted or consumed by runtime code.

use srt_core::PacketNumber;

/// Packet-level reliability event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketReliabilityEvent {
    /// A packet entered the send side in-flight set.
    Sent {
        /// Packet number.
        packet_number: PacketNumber,
    },
    /// A packet was acknowledged by the peer.
    Acked {
        /// Packet number.
        packet_number: PacketNumber,
    },
    /// A received packet was detected as duplicate.
    Duplicate {
        /// Packet number.
        packet_number: PacketNumber,
    },
    /// A packet exceeded its timeout policy.
    TimedOut {
        /// Packet number.
        packet_number: PacketNumber,
    },
    /// A packet was selected for retransmission.
    Retransmit {
        /// Packet number.
        packet_number: PacketNumber,
    },
    /// A packet was dropped by reliability policy.
    Dropped {
        /// Packet number.
        packet_number: PacketNumber,
    },
}
