//! RESET_STREAM frame primitives.

use crate::StreamId;

/// RESET_STREAM frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResetStreamFrame {
    /// Stream to reset.
    pub stream_id: StreamId,
    /// Application-defined reset code.
    pub code: u16,
}

impl ResetStreamFrame {
    /// Creates a RESET_STREAM frame.
    #[must_use]
    pub const fn new(stream_id: StreamId, code: u16) -> Self {
        Self { stream_id, code }
    }
}
