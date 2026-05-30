//! Send-side runtime boundaries.

use srt_core::{MessageId, PacketNumber, Result, StreamId};
use srt_reliability::StreamReliability;

/// Outbound message metadata understood by the SRT protocol engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SendOptions {
    /// Target logical stream.
    pub stream_id: StreamId,
    /// Optional message identifier chosen by a higher layer.
    pub message_id: Option<MessageId>,
    /// Reliability policy requested for this message.
    pub reliability: StreamReliability,
}

impl SendOptions {
    /// Creates send options for a stream using reliable delivery defaults.
    #[must_use]
    pub const fn reliable(stream_id: StreamId, max_retransmits: u8) -> Self {
        Self {
            stream_id,
            message_id: None,
            reliability: StreamReliability::reliable(stream_id, max_retransmits),
        }
    }

    /// Creates send options for a stream using best-effort delivery.
    #[must_use]
    pub const fn best_effort(stream_id: StreamId) -> Self {
        Self {
            stream_id,
            message_id: None,
            reliability: StreamReliability::best_effort(stream_id),
        }
    }

    /// Returns options with an explicit message id.
    #[must_use]
    pub const fn with_message_id(mut self, message_id: MessageId) -> Self {
        self.message_id = Some(message_id);
        self
    }
}

/// A send request accepted by the engine boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SendIntent<'a> {
    /// Complete message bytes supplied by the caller.
    pub message: &'a [u8],
    /// Send metadata.
    pub options: SendOptions,
}

impl<'a> SendIntent<'a> {
    /// Creates a send intent.
    #[must_use]
    pub const fn new(message: &'a [u8], options: SendOptions) -> Self {
        Self { message, options }
    }

    /// Returns the message length in bytes.
    #[must_use]
    pub const fn message_len(self) -> usize {
        self.message.len()
    }

    /// Returns whether this intent carries an empty message.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.message.is_empty()
    }
}

/// Send-side protocol boundary.
pub trait Sender {
    /// Queues a complete message for protocol transmission.
    fn send(&mut self, intent: SendIntent<'_>) -> Result<()>;

    /// Returns the next packet number that would be used for sending.
    fn next_packet_number(&self) -> PacketNumber;
}
