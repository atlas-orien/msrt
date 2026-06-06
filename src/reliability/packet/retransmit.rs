//! Retransmission policy boundary.

use crate::core::PacketKey;

use super::TimeoutEvent;

/// Decision made after a packet timeout or reliability check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetransmitDecision {
    /// Retransmit the packet.
    Retransmit {
        /// Packet selected for retransmission.
        key: PacketKey,
    },
    /// Keep waiting; do not retransmit yet.
    Wait {
        /// Packet that remains in flight.
        key: PacketKey,
    },
    /// Drop the packet from reliability tracking.
    Drop {
        /// Packet removed from tracking.
        key: PacketKey,
    },
}

/// Chooses whether timed-out packets should be retransmitted.
pub trait RetransmitPolicy {
    /// Applies a timeout event and returns the retransmission decision.
    fn on_timeout(&mut self, event: TimeoutEvent) -> RetransmitDecision;

    /// Returns whether a packet is currently eligible for retransmission.
    fn should_retransmit(&self, key: PacketKey) -> bool;
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
            RetransmitDecision::Retransmit { key: event.key }
        } else {
            RetransmitDecision::Drop { key: event.key }
        }
    }

    fn should_retransmit(&self, _key: PacketKey) -> bool {
        self.max_attempts > 0
    }
}

#[cfg(test)]
mod tests {
    use crate::core::{MessageId, PacketIndex, PacketKey};

    use super::{RetransmitDecision, RetransmitPolicy, RetryLimitPolicy, TimeoutEvent};

    #[test]
    fn retry_limit_policy_drops_after_limit() {
        let key = PacketKey::new(MessageId::new(3), PacketIndex::ZERO);
        let mut policy = RetryLimitPolicy::new(2);

        assert_eq!(
            policy.on_timeout(TimeoutEvent::new(key, 10, 1)),
            RetransmitDecision::Retransmit { key }
        );
        assert_eq!(
            policy.on_timeout(TimeoutEvent::new(key, 20, 2)),
            RetransmitDecision::Drop { key }
        );
    }
}
