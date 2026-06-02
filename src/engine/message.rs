//! Message delivery and reassembly boundaries.

use crate::core::{ChannelId, Result};
use crate::reliability::{MessageFragment, MessageKey, MessageStatus};

/// Borrowed message delivered by the engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeliveredMessage<'a> {
    /// Channel that owns the message.
    pub channel_id: ChannelId,
    /// Message key.
    pub key: MessageKey,
    /// Complete message bytes.
    pub bytes: &'a [u8],
}

impl<'a> DeliveredMessage<'a> {
    /// Creates a delivered message view.
    #[must_use]
    pub const fn new(channel_id: ChannelId, key: MessageKey, bytes: &'a [u8]) -> Self {
        Self {
            channel_id,
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
