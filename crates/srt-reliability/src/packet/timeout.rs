//! Timeout policy boundary.

use srt_core::PacketNumber;

/// Timeout event produced by a engine-provided clock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimeoutEvent {
    /// Packet that timed out.
    pub packet_number: PacketNumber,
    /// Engine-defined elapsed ticks since the packet was sent.
    pub elapsed_ticks: u64,
    /// Number of previous retransmission attempts.
    pub attempts: u8,
}

impl TimeoutEvent {
    /// Creates a timeout event.
    #[must_use]
    pub const fn new(packet_number: PacketNumber, elapsed_ticks: u64, attempts: u8) -> Self {
        Self {
            packet_number,
            elapsed_ticks,
            attempts,
        }
    }
}

/// Determines whether packets have timed out.
pub trait TimeoutPolicy {
    /// Returns whether a packet should be considered timed out at `now_ticks`.
    fn has_timed_out(&self, packet_number: PacketNumber, now_ticks: u64) -> bool;

    /// Builds a timeout event for a packet.
    fn timeout_event(
        &self,
        packet_number: PacketNumber,
        elapsed_ticks: u64,
        attempts: u8,
    ) -> TimeoutEvent {
        TimeoutEvent::new(packet_number, elapsed_ticks, attempts)
    }
}
