//! Packet kind definitions.

/// Coarse packet categories reserved by the core protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketKind {
    /// User message packet.
    Data,
    /// Acknowledgement packet.
    Ack,
    /// Transport control packet reserved for protocol runtime use.
    Control,
}
