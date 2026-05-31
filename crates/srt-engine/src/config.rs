//! Engine configuration.

use srt_core::{MessageId, PacketNumber};

/// Maximum encoded wire bytes held by one MVP engine event.
pub const MAX_WIRE_BYTES: usize = 128;
/// Maximum complete message bytes held by the MVP engine.
pub const MAX_MESSAGE_BYTES: usize = 256;
/// Maximum pending engine events.
pub const MAX_EVENTS: usize = 16;
/// Maximum in-flight packets tracked by the MVP engine.
pub const MAX_IN_FLIGHT_PACKETS: usize = 16;
/// Default maximum message fragment bytes per packet.
pub const DEFAULT_FRAGMENT_BYTES: usize = 12;

/// Minimal protocol engine configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EngineConfig {
    /// First packet number used by this engine.
    pub initial_packet_number: PacketNumber,
    /// First message identifier used by this engine.
    pub initial_message_id: MessageId,
    /// Maximum message fragment bytes written into one packet.
    pub fragment_bytes: usize,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            initial_packet_number: PacketNumber::ZERO,
            initial_message_id: MessageId::ZERO,
            fragment_bytes: DEFAULT_FRAGMENT_BYTES,
        }
    }
}
