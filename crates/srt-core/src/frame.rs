//! Protocol frame primitives carried inside packet payloads.

pub mod ack;
pub mod kind;
pub mod message;

pub use ack::{AckFrame, AckRange, MAX_ACK_RANGES};
pub use kind::FrameKind;
pub use message::{ChannelId, MessageData, MessageFlags, MessageFrame, MessageId};

/// Borrowed protocol frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Frame<'a> {
    /// Message fragment carried on a channel.
    Message(MessageFrame<'a>),
    /// Acknowledgement information.
    Ack(AckFrame),
}

impl Frame<'_> {
    /// Returns the frame kind.
    #[must_use]
    pub const fn kind(self) -> FrameKind {
        match self {
            Self::Message(_) => FrameKind::Message,
            Self::Ack(_) => FrameKind::Ack,
        }
    }
}
