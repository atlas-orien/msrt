//! Retransmission boundary.

use srt_core::PacketNumber;

/// Chooses packets eligible for retransmission.
pub trait RetransmitPolicy {
    /// Returns whether `seq` should be retransmitted.
    fn should_retransmit(&self, packet_number: PacketNumber) -> bool;
}
