//! ACK tracking boundary.

use srt_core::{AckFrame, PacketNumber};

use super::PacketState;

/// Result of applying an ACK frame to local packet state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AckOutcome {
    /// The ACK confirmed a packet that was still in flight.
    NewlyAcked {
        /// Confirmed packet number.
        packet_number: PacketNumber,
    },
    /// The packet was already known as acknowledged.
    AlreadyAcked {
        /// Packet number that was already acknowledged.
        packet_number: PacketNumber,
    },
    /// The ACK could not be applied to the current state.
    Ignored {
        /// Packet number carried by the ACK.
        packet_number: PacketNumber,
    },
}

/// Tracks packet acknowledgement state.
pub trait AckTracker {
    /// Records that a packet has been sent and is waiting for acknowledgement.
    fn on_packet_sent(&mut self, packet_number: PacketNumber);

    /// Applies an ACK frame to the tracked packet state.
    fn on_ack(&mut self, frame: AckFrame) -> AckOutcome;

    /// Returns the current known state for a packet number.
    fn state_of(&self, packet_number: PacketNumber) -> PacketState;
}
