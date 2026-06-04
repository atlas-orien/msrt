//! Reliable packet recovery state.

use crate::core::{Ack, ChannelId, MessageId, PacketNumber, Result};

use self::inflight::{InFlightPacket, InFlightPackets};

pub(crate) mod inflight;
pub(crate) mod retransmit;

/// Reliable-send recovery state machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RecoveryState {
    in_flight: InFlightPackets,
}

impl RecoveryState {
    pub(crate) const fn new() -> Self {
        Self {
            in_flight: InFlightPackets::new(),
        }
    }

    pub(crate) fn track(&mut self, packet: InFlightPacket) -> Result<()> {
        self.in_flight.track(packet)
    }

    pub(crate) fn apply_ack(&mut self, ack: Ack) {
        self.in_flight.apply_ack(ack);
    }

    pub(crate) fn packets(&self) -> impl Iterator<Item = &InFlightPacket> {
        self.in_flight.packets()
    }

    #[cfg_attr(not(feature = "std"), allow(dead_code))]
    pub(crate) const fn in_flight_len(&self) -> usize {
        self.in_flight.len()
    }

    pub(crate) const fn available_in_flight(&self) -> usize {
        self.in_flight.available()
    }

    pub(crate) fn remove_message(&mut self, channel_id: ChannelId, message_id: MessageId) {
        self.in_flight.remove_message(channel_id, message_id);
    }

    pub(crate) fn note_sent(&mut self, packet_number: PacketNumber, now_ms: u64) {
        self.in_flight.note_sent(packet_number, now_ms);
    }

    pub(crate) fn note_retransmit_sent(&mut self, packet_number: PacketNumber, now_ms: u64) {
        self.in_flight.note_retransmit_sent(packet_number, now_ms);
    }
}
