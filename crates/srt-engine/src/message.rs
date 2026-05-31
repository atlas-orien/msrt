//! Message delivery and reassembly boundaries.

use srt_core::{Result, StreamId};
use srt_reliability::{MessageFragment, MessageKey, MessageStatus};

/// Borrowed message delivered by the engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeliveredMessage<'a> {
    /// Stream that owns the message.
    pub stream_id: StreamId,
    /// Message key.
    pub key: MessageKey,
    /// Complete message bytes.
    pub bytes: &'a [u8],
}

impl<'a> DeliveredMessage<'a> {
    /// Creates a delivered message view.
    #[must_use]
    pub const fn new(stream_id: StreamId, key: MessageKey, bytes: &'a [u8]) -> Self {
        Self {
            stream_id,
            key,
            bytes,
        }
    }

    /// Returns the delivered message length.
    #[must_use]
    pub const fn len(self) -> usize {
        self.bytes.len()
    }

    /// Returns whether the delivered message is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.bytes.is_empty()
    }
}

/// Message delivery boundary.
pub trait MessageDelivery {
    /// Delivers one complete message to the embedding environment.
    fn deliver(&mut self, message: DeliveredMessage<'_>) -> Result<()>;
}

/// Message fragment reassembly boundary.
pub trait Reassembly {
    /// Observes a message fragment and returns the current reassembly status.
    fn observe_fragment(&mut self, fragment: MessageFragment) -> Result<MessageStatus>;

    /// Returns the current status for a message key.
    fn status_of(&self, key: MessageKey) -> MessageStatus;
}
