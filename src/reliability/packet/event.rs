//! Packet reliability events emitted or consumed by engine code.

use crate::core::PacketKey;

/// Packet-level reliability event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketReliabilityEvent {
    /// A packet entered the send side in-flight set.
    Sent {
        /// Packet key.
        key: PacketKey,
    },
    /// A packet was acknowledged by the peer.
    Acked {
        /// Packet key.
        key: PacketKey,
    },
    /// A received packet was detected as duplicate.
    Duplicate {
        /// Packet key.
        key: PacketKey,
    },
    /// A packet exceeded its timeout policy.
    TimedOut {
        /// Packet key.
        key: PacketKey,
    },
    /// A packet was selected for retransmission.
    Retransmit {
        /// Packet key.
        key: PacketKey,
    },
    /// A packet was dropped by reliability policy.
    Dropped {
        /// Packet key.
        key: PacketKey,
    },
}
