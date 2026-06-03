//! ACK tracking boundary.

use crate::core::{Ack, Error, ErrorKind, PacketNumber, Result};

use super::PacketState;

/// Result of applying an ACK to local packet state.
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

    /// Applies an ACK to the tracked packet state.
    fn on_ack(&mut self, ack: Ack) -> AckOutcome;

    /// Returns the current known state for a packet number.
    fn state_of(&self, packet_number: PacketNumber) -> PacketState;
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
    pub fn try_on_packet_sent(&mut self, packet_number: PacketNumber) -> Result<()> {
        if N == 0 {
            return Err(Error::new(ErrorKind::Reliability));
        }

        for slot in &mut self.packets {
            if slot
                .map(|tracked| tracked.packet_number == packet_number)
                .unwrap_or(false)
            {
                *slot = Some(TrackedPacket::new(packet_number, PacketState::InFlight));
                return Ok(());
            }
        }

        for slot in &mut self.packets {
            if slot.is_none() {
                *slot = Some(TrackedPacket::new(packet_number, PacketState::InFlight));
                return Ok(());
            }
        }

        Err(Error::new(ErrorKind::Reliability))
    }

    /// Applies an ACK.
    #[must_use]
    pub fn apply_ack(&mut self, ack: Ack) -> AckOutcome {
        for slot in &mut self.packets {
            let Some(mut tracked) = *slot else {
                continue;
            };

            if !ack.acknowledges(tracked.packet_number) {
                continue;
            }

            let packet_number = tracked.packet_number;

            return match tracked.state {
                PacketState::InFlight => {
                    tracked.state = PacketState::Acked;
                    *slot = Some(tracked);
                    AckOutcome::NewlyAcked { packet_number }
                }
                PacketState::Acked => AckOutcome::AlreadyAcked { packet_number },
                _ => AckOutcome::Ignored { packet_number },
            };
        }

        AckOutcome::Ignored {
            packet_number: ack.largest_acknowledged,
        }
    }
}

impl<const N: usize> Default for PacketAckTracker<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> AckTracker for PacketAckTracker<N> {
    fn on_packet_sent(&mut self, packet_number: PacketNumber) {
        let _ = self.try_on_packet_sent(packet_number);
    }

    fn on_ack(&mut self, ack: Ack) -> AckOutcome {
        self.apply_ack(ack)
    }

    fn state_of(&self, packet_number: PacketNumber) -> PacketState {
        self.packets
            .iter()
            .flatten()
            .find(|tracked| tracked.packet_number == packet_number)
            .map(|tracked| tracked.state)
            .unwrap_or(PacketState::Unknown)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TrackedPacket {
    packet_number: PacketNumber,
    state: PacketState,
}

impl TrackedPacket {
    const fn new(packet_number: PacketNumber, state: PacketState) -> Self {
        Self {
            packet_number,
            state,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::core::{Ack, PacketNumber};

    use super::{AckOutcome, AckTracker, PacketAckTracker, PacketState};

    #[test]
    fn ack_tracker_marks_in_flight_packet_acked() {
        let mut tracker = PacketAckTracker::<2>::new();
        let packet_number = PacketNumber::new(7);

        tracker.on_packet_sent(packet_number);

        assert_eq!(tracker.state_of(packet_number), PacketState::InFlight);
        assert_eq!(
            tracker.on_ack(Ack::new(packet_number)),
            AckOutcome::NewlyAcked { packet_number }
        );
        assert_eq!(tracker.state_of(packet_number), PacketState::Acked);
        assert_eq!(
            tracker.on_ack(Ack::new(packet_number)),
            AckOutcome::AlreadyAcked { packet_number }
        );
    }

    #[test]
    fn ack_tracker_applies_ack_range() {
        let mut tracker = PacketAckTracker::<4>::new();
        let start = PacketNumber::new(1);
        let end = PacketNumber::new(3);
        let ranges = [crate::core::AckRange::new(start, end); crate::core::MAX_ACK_RANGES];

        tracker.on_packet_sent(start);
        tracker.on_packet_sent(PacketNumber::new(2));
        tracker.on_packet_sent(end);

        assert_eq!(
            tracker.on_ack(Ack::from_ranges(ranges, 1)),
            AckOutcome::NewlyAcked {
                packet_number: start
            }
        );
        assert_eq!(tracker.state_of(start), PacketState::Acked);
    }
}
