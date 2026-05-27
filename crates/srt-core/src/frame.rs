//! Frame-level protocol primitives.

/// Coarse frame categories reserved by the core protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameKind {
    /// Application data frame.
    Data,
    /// Acknowledgement frame.
    Ack,
    /// Runtime or control frame.
    Control,
}
