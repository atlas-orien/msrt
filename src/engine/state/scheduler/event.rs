//! Engine scheduler event types.

use crate::core::PacketKey;
use crate::engine::{SendFailedEvent, config::MAX_WIRE_BYTES};

/// Events produced by engine state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EngineOutput {
    /// Protocol bytes should be written to the serial link.
    Write(WriteEvent),
    /// A message could not be sent reliably.
    SendFailed(SendFailedEvent),
}

/// A non-blocking write request produced by the engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WriteEvent {
    /// Message-scoped packet identity assigned to this write.
    pub key: PacketKey,
    /// Fixed storage containing encoded wire bytes.
    pub bytes: [u8; MAX_WIRE_BYTES],
    /// Number of valid bytes in `bytes`.
    pub len: usize,
    /// Send attempt count: 0 = first send, >=1 = retransmit.
    pub attempts: u8,
    /// Internal transmit priority used by the scheduler.
    pub priority: WritePriority,
}

impl WriteEvent {
    /// Returns the valid encoded wire bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8] {
        self.bytes.split_at(self.len).0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum WritePriority {
    Control,
    Retransmit,
    NewData,
}
