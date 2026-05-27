//! Decoder progress states.

/// Decoder progress after receiving bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodeStatus {
    /// More bytes are needed.
    NeedMore,
    /// A full frame is available.
    FrameReady,
    /// Bytes were skipped while resynchronizing.
    Resynced,
    /// A corrupted or invalid frame was discarded.
    Discarded,
}
