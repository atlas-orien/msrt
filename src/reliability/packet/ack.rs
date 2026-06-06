//! ACK tracking boundary.

use crate::core::{Error, ErrorKind, PacketKey, Result};

use super::PacketState;

/// Result of applying an ACK to local packet state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AckOutcome {
    /// The ACK confirmed a packet that was still in flight.
    NewlyAcked {
        /// Confirmed packet key.
        key: PacketKey,
    },
    /// The packet was already known as acknowledged.
    AlreadyAcked {
        /// Packet key that was already acknowledged.
        key: PacketKey,
    },
    /// The ACK could not be applied to the current state.
    Ignored {
        /// Packet key carried by the ACK.
        key: PacketKey,
    },
}

/// Tracks packet acknowledgement state.
pub trait AckTracker {
    /// Records that a packet has been sent and is waiting for acknowledgement.
    fn on_packet_sent(&mut self, key: PacketKey);

    /// Applies an ACK key to the tracked packet state.
    fn on_ack(&mut self, key: PacketKey) -> AckOutcome;

    /// Returns the current known state for a packet key.
    fn state_of(&self, key: PacketKey) -> PacketState;
}

/// Fixed-capacity ACK tracker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PacketAckTracker<const N: usize> {
    packets: [Option<TrackedPacket>; N],
}

impl<const N: usize> PacketAckTracker<N> {
    /// Creates an empty ACK tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self { packets: [None; N] }
    }

    /// Records an in-flight packet or returns an error when no slot exists.
    pub fn try_on_packet_sent(&mut self, key: PacketKey) -> Result<()> {
        if N == 0 {
            return Err(Error::new(ErrorKind::Reliability));
        }

        for slot in &mut self.packets {
            if slot.map(|tracked| tracked.key == key).unwrap_or(false) {
                *slot = Some(TrackedPacket::new(key, PacketState::InFlight));
                return Ok(());
            }
        }

        for slot in &mut self.packets {
            if slot.is_none() {
                *slot = Some(TrackedPacket::new(key, PacketState::InFlight));
                return Ok(());
            }
        }

        Err(Error::new(ErrorKind::Reliability))
    }

    /// Applies an ACK key.
    #[must_use]
    pub fn apply_ack(&mut self, key: PacketKey) -> AckOutcome {
        for slot in &mut self.packets {
            let Some(mut tracked) = *slot else {
                continue;
            };

            if tracked.key != key {
                continue;
            }

            return match tracked.state {
                PacketState::InFlight => {
                    tracked.state = PacketState::Acked;
                    *slot = Some(tracked);
                    AckOutcome::NewlyAcked { key }
                }
                PacketState::Acked => AckOutcome::AlreadyAcked { key },
                _ => AckOutcome::Ignored { key },
            };
        }

        AckOutcome::Ignored { key }
    }
}

impl<const N: usize> Default for PacketAckTracker<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> AckTracker for PacketAckTracker<N> {
    fn on_packet_sent(&mut self, key: PacketKey) {
        let _ = self.try_on_packet_sent(key);
    }

    fn on_ack(&mut self, key: PacketKey) -> AckOutcome {
        self.apply_ack(key)
    }

    fn state_of(&self, key: PacketKey) -> PacketState {
        self.packets
            .iter()
            .flatten()
            .find(|tracked| tracked.key == key)
            .map(|tracked| tracked.state)
            .unwrap_or(PacketState::Unknown)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TrackedPacket {
    key: PacketKey,
    state: PacketState,
}

impl TrackedPacket {
    const fn new(key: PacketKey, state: PacketState) -> Self {
        Self { key, state }
    }
}

#[cfg(test)]
mod tests {
    use crate::core::{MessageId, PacketIndex, PacketKey};

    use super::{AckOutcome, AckTracker, PacketAckTracker, PacketState};

    #[test]
    fn ack_tracker_marks_in_flight_packet_acked() {
        let mut tracker = PacketAckTracker::<2>::new();
        let key = PacketKey::new(MessageId::new(7), PacketIndex::ZERO);

        tracker.on_packet_sent(key);

        assert_eq!(tracker.state_of(key), PacketState::InFlight);
        assert_eq!(tracker.on_ack(key), AckOutcome::NewlyAcked { key });
        assert_eq!(tracker.state_of(key), PacketState::Acked);
        assert_eq!(tracker.on_ack(key), AckOutcome::AlreadyAcked { key });
    }
}
