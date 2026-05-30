//! Timeout boundary.

use srt_core::PacketNumber;

/// Handles packet timeout events.
pub trait TimeoutPolicy {
    /// Records that `seq` timed out.
    fn on_timeout(&mut self, packet_number: PacketNumber);
}
