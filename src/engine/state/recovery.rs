//! Reliable packet recovery state.

use crate::core::{MessageId, PacketKey, PacketType, Result};
#[cfg(feature = "dynamic-recovery")]
use crate::reliability::{DynamicRecoveryConfig, DynamicRecoveryState};

use self::inflight::{InFlightPacket, InFlightPackets};

pub(crate) mod inflight;
pub(crate) mod retransmit;

/// Reliable-send recovery state machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RecoveryState {
    in_flight: InFlightPackets,
    last_tick_ms: Option<u64>,
    #[cfg(feature = "dynamic-recovery")]
    dynamic: DynamicRecoveryState,
}

impl RecoveryState {
    #[cfg(not(feature = "dynamic-recovery"))]
    pub(crate) const fn new() -> Self {
        Self {
            in_flight: InFlightPackets::new(),
            last_tick_ms: None,
        }
    }

    #[cfg(feature = "dynamic-recovery")]
    pub(crate) const fn new(dynamic_config: DynamicRecoveryConfig) -> Self {
        Self {
            in_flight: InFlightPackets::new(),
            last_tick_ms: None,
            dynamic: DynamicRecoveryState::new(dynamic_config),
        }
    }

    pub(crate) fn track(&mut self, packet: InFlightPacket) -> Result<()> {
        self.in_flight.track(packet)
    }

    pub(crate) fn apply_ack(&mut self, key: PacketKey) {
        self.in_flight.apply_ack(key);
    }

    #[cfg(feature = "dynamic-recovery")]
    pub(crate) fn apply_ack_at(&mut self, key: PacketKey, now_ms: u64) {
        if let Some(packet) = self.in_flight.packet(key)
            && packet.sent
        {
            self.dynamic
                .observe_ack(now_ms.saturating_sub(packet.last_sent_ms));
        }

        self.apply_ack(key);
    }

    pub(crate) fn packets(&self) -> impl Iterator<Item = &InFlightPacket> {
        self.in_flight.packets()
    }

    #[cfg_attr(not(feature = "tracing"), allow(dead_code))]
    pub(crate) const fn in_flight_len(&self) -> usize {
        self.in_flight.len()
    }

    pub(crate) const fn available_in_flight(&self) -> usize {
        self.in_flight.available()
    }

    pub(crate) fn should_tick(&mut self, now_ms: u64) -> bool {
        if self.last_tick_ms == Some(now_ms) {
            return false;
        }

        self.last_tick_ms = Some(now_ms);
        true
    }

    pub(crate) fn remove_message(&mut self, packet_type: PacketType, message_id: MessageId) {
        self.in_flight.remove_message(packet_type, message_id);
    }

    pub(crate) fn note_sent(&mut self, key: PacketKey, now_ms: u64) {
        self.in_flight.note_sent(key, now_ms);
    }

    pub(crate) fn note_retransmit_sent(&mut self, key: PacketKey, now_ms: u64) {
        self.in_flight.note_retransmit_sent(key, now_ms);
    }

    #[cfg(feature = "dynamic-recovery")]
    pub(crate) fn dynamic_timeout_ms(&self, config: DynamicRecoveryConfig, attempts: u8) -> u64 {
        self.dynamic.timeout_ms(config, attempts)
    }
}
