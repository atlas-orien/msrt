//! Protocol frame primitives carried inside packet payloads.

pub mod ack;
pub mod kind;
pub mod ping;
pub mod reset_stream;
pub mod stream;

pub use ack::AckFrame;
pub use kind::FrameKind;
pub use ping::PingFrame;
pub use reset_stream::ResetStreamFrame;
pub use stream::{MessageId, StreamData, StreamFlags, StreamFrame, StreamId};

/// Borrowed protocol frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Frame<'a> {
    /// Message fragment carried on a stream.
    Stream(StreamFrame<'a>),
    /// Acknowledgement information.
    Ack(AckFrame),
    /// Ping frame.
    Ping(PingFrame),
    /// Reset stream frame.
    ResetStream(ResetStreamFrame),
}

impl Frame<'_> {
    /// Returns the frame kind.
    #[must_use]
    pub const fn kind(self) -> FrameKind {
        match self {
            Self::Stream(_) => FrameKind::Stream,
            Self::Ack(_) => FrameKind::Ack,
            Self::Ping(_) => FrameKind::Ping,
            Self::ResetStream(_) => FrameKind::ResetStream,
        }
    }
}
