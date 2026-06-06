//! Engine configuration.

use crate::core::MessageId;
use crate::integrity::IntegrityConfig;

/// Maximum encoded wire bytes held by one MVP engine event.
pub(crate) const MAX_WIRE_BYTES: usize = 128;
/// Maximum ingress bytes buffered by the engine streaming decoder.
pub(crate) const MAX_INGRESS_BYTES: usize = MAX_WIRE_BYTES * MAX_EVENTS;
/// Maximum complete message bytes held by the MVP engine.
pub(crate) const MAX_MESSAGE_BYTES: usize = 256;
/// Maximum pending engine events.
pub(crate) const MAX_EVENTS: usize = 16;
/// Maximum complete messages waiting for the application to poll.
pub(crate) const MAX_MESSAGE_EVENTS: usize = 16;
/// Maximum in-flight packets tracked by the MVP engine.
pub(crate) const MAX_IN_FLIGHT_PACKETS: usize = 16;
/// Maximum pending ACK packet keys retained by the engine.
pub(crate) const MAX_PENDING_ACKS: usize = 16;
/// Maximum incomplete messages tracked by the reassembly table.
pub(crate) const MAX_REASSEMBLY_MESSAGES: usize = 4;
/// Default maximum message fragment bytes per packet.
pub(crate) const DEFAULT_FRAGMENT_BYTES: usize = 64;
/// Default maximum retransmission attempts before a send fails.
///
/// This gives a reliable packet one initial send plus five retransmission
/// opportunities. With the default 250 ms timeout, a missing ACK is reported
/// after roughly 1.5 s, which is conservative for UART-like MCU links without
/// hiding disconnects for too long.
pub(crate) const DEFAULT_MAX_RETRANSMIT_ATTEMPTS: u8 = 10;
/// Default retransmission timeout in engine ticks.
pub(crate) const DEFAULT_RETRANSMIT_TIMEOUT_MS: u64 = 250;
/// Default incomplete message reassembly timeout in engine ticks.
///
/// This must outlive the default reliable-send retry window so late
/// retransmitted fragments can still complete an in-progress message.
pub(crate) const DEFAULT_REASSEMBLY_TIMEOUT_MS: u64 = 2_000;

/// Minimal protocol engine configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EngineConfig {
    /// First message identifier used by this engine.
    pub initial_message_id: MessageId,
    /// Maximum message fragment bytes written into one packet.
    pub fragment_bytes: usize,
    /// Maximum retransmission attempts before a packet is considered failed.
    pub max_retransmit_attempts: u8,
    /// Ticks that must elapse before an in-flight packet is eligible for retransmission.
    pub retransmit_timeout_ms: u64,
    /// Ticks after which incomplete reassembly slots are released.
    pub reassembly_timeout_ms: u64,
    /// Packet integrity backend used by this engine.
    pub integrity: IntegrityConfig,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            initial_message_id: MessageId::ZERO,
            fragment_bytes: DEFAULT_FRAGMENT_BYTES,
            max_retransmit_attempts: DEFAULT_MAX_RETRANSMIT_ATTEMPTS,
            retransmit_timeout_ms: DEFAULT_RETRANSMIT_TIMEOUT_MS,
            reassembly_timeout_ms: DEFAULT_REASSEMBLY_TIMEOUT_MS,
            integrity: IntegrityConfig::DEFAULT,
        }
    }
}
