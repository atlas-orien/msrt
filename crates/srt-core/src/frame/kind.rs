//! Protocol frame kind definitions.

/// Protocol frame type carried inside packet payloads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameKind {
    /// Carries one message fragment on a stream.
    Stream,
    /// Carries acknowledgement information.
    Ack,
    /// Keeps the link active or elicits acknowledgement.
    Ping,
    /// Resets a stream.
    ResetStream,
}
