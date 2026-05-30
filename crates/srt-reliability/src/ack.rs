//! Acknowledgement boundary.

use srt_core::PacketNumber;

/// Tracks acknowledgement state.
pub trait AckTracker {
    /// Records that `seq` was acknowledged.
    fn on_ack(&mut self, packet_number: PacketNumber);
}
