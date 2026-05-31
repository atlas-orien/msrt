//! Engine configuration.

use srt_core::{MessageId, PacketNumber};

/// Maximum encoded wire bytes held by one MVP engine event.
pub const MAX_WIRE_BYTES: usize = 128;
/// Maximum ingress bytes buffered by the engine streaming decoder.
pub const MAX_INGRESS_BYTES: usize = MAX_WIRE_BYTES * MAX_EVENTS;
/// Maximum complete message bytes held by the MVP engine.
pub const MAX_MESSAGE_BYTES: usize = 256;
/// Maximum pending engine events.
pub const MAX_EVENTS: usize = 16;
/// Maximum in-flight packets tracked by the MVP engine.
pub const MAX_IN_FLIGHT_PACKETS: usize = 16;
/// Maximum observed packets retained for ACK range generation.
pub const MAX_ACK_TRACKED_PACKETS: usize = 16;
/// Maximum incomplete messages tracked by the reassembly table.
pub const MAX_REASSEMBLY_MESSAGES: usize = 4;
/// Default maximum message fragment bytes per packet.
pub const DEFAULT_FRAGMENT_BYTES: usize = 32;
/// Default maximum retransmission attempts before a send fails.
pub const DEFAULT_MAX_RETRANSMIT_ATTEMPTS: u8 = 3;
/// Default retransmission timeout in engine ticks.
pub const DEFAULT_RETRANSMIT_TIMEOUT_MS: u64 = 1;
/// Default incomplete message reassembly timeout in engine ticks.
pub const DEFAULT_REASSEMBLY_TIMEOUT_MS: u64 = 30;

/// Minimal protocol engine configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EngineConfig {
    /// First packet number used by this engine.
    pub initial_packet_number: PacketNumber,
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
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            initial_packet_number: PacketNumber::ZERO,
            initial_message_id: MessageId::ZERO,
            fragment_bytes: DEFAULT_FRAGMENT_BYTES,
            max_retransmit_attempts: DEFAULT_MAX_RETRANSMIT_ATTEMPTS,
            retransmit_timeout_ms: DEFAULT_RETRANSMIT_TIMEOUT_MS,
            reassembly_timeout_ms: DEFAULT_REASSEMBLY_TIMEOUT_MS,
        }
    }
}
