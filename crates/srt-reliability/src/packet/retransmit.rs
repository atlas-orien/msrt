//! Retransmission policy boundary.

use srt_core::PacketNumber;

use super::TimeoutEvent;

/// Decision made after a packet timeout or reliability check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetransmitDecision {
    /// Retransmit the packet.
    Retransmit {
        /// Packet selected for retransmission.
        packet_number: PacketNumber,
    },
    /// Keep waiting; do not retransmit yet.
    Wait {
        /// Packet that remains in flight.
        packet_number: PacketNumber,
    },
    /// Drop the packet from reliability tracking.
    Drop {
        /// Packet removed from tracking.
        packet_number: PacketNumber,
    },
}

/// Chooses whether timed-out packets should be retransmitted.
pub trait RetransmitPolicy {
    /// Applies a timeout event and returns the retransmission decision.
    fn on_timeout(&mut self, event: TimeoutEvent) -> RetransmitDecision;

    /// Returns whether a packet is currently eligible for retransmission.
    fn should_retransmit(&self, packet_number: PacketNumber) -> bool;
}
