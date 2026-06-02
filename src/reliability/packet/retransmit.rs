//! Retransmission policy boundary.

use crate::core::PacketNumber;

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

/// Retry-limit retransmission policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetryLimitPolicy {
    /// Maximum retransmission attempts before a packet is dropped.
    pub max_attempts: u8,
}

impl RetryLimitPolicy {
    /// Creates a retry-limit policy.
    #[must_use]
    pub const fn new(max_attempts: u8) -> Self {
        Self { max_attempts }
    }
}

impl RetransmitPolicy for RetryLimitPolicy {
    fn on_timeout(&mut self, event: TimeoutEvent) -> RetransmitDecision {
        if event.attempts < self.max_attempts {
            RetransmitDecision::Retransmit {
                packet_number: event.packet_number,
            }
        } else {
            RetransmitDecision::Drop {
                packet_number: event.packet_number,
            }
        }
    }

    fn should_retransmit(&self, _packet_number: PacketNumber) -> bool {
        self.max_attempts > 0
    }
}

#[cfg(test)]
mod tests {
    use crate::core::PacketNumber;

    use super::{RetransmitDecision, RetransmitPolicy, RetryLimitPolicy, TimeoutEvent};

    #[test]
    fn retry_limit_policy_drops_after_limit() {
        let packet_number = PacketNumber::new(3);
        let mut policy = RetryLimitPolicy::new(2);

        assert_eq!(
            policy.on_timeout(TimeoutEvent::new(packet_number, 10, 1)),
            RetransmitDecision::Retransmit { packet_number }
        );
        assert_eq!(
            policy.on_timeout(TimeoutEvent::new(packet_number, 20, 2)),
            RetransmitDecision::Drop { packet_number }
        );
    }
}
