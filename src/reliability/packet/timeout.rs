//! Timeout policy boundary.

use crate::core::PacketKey;

/// Timeout event produced by a engine-provided clock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimeoutEvent {
    /// Packet that timed out.
    pub key: PacketKey,
    /// Engine-defined elapsed ticks since the packet was sent.
    pub elapsed_ticks: u64,
    /// Number of previous retransmission attempts.
    pub attempts: u8,
}

impl TimeoutEvent {
    /// Creates a timeout event.
    #[must_use]
    pub const fn new(key: PacketKey, elapsed_ticks: u64, attempts: u8) -> Self {
        Self {
            key,
            elapsed_ticks,
            attempts,
        }
    }
}

/// Determines whether packets have timed out.
pub trait TimeoutPolicy {
    /// Returns whether a packet should be considered timed out at `now_ticks`.
    fn has_timed_out(&self, key: PacketKey, now_ticks: u64) -> bool;

    /// Builds a timeout event for a packet.
    fn timeout_event(&self, key: PacketKey, elapsed_ticks: u64, attempts: u8) -> TimeoutEvent {
        TimeoutEvent::new(key, elapsed_ticks, attempts)
    }
}
