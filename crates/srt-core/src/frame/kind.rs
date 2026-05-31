//! Protocol frame kind definitions.

/// Protocol frame type carried inside packet payloads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameKind {
    /// Carries one message fragment on a channel.
    Message,
    /// Carries acknowledgement information.
    Ack,
}
