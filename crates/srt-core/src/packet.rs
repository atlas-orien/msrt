//! Packet-level protocol primitives.

/// Coarse packet categories reserved by the core protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketKind {
    /// User message packet.
    Message,
    /// Transport control packet.
    Control,
}
