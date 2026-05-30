//! Packet type definitions.

/// Coarse packet categories reserved by the core protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketType {
    /// Initial packet used to start protocol state.
    Initial,
    /// Normal packet carrying protocol frames.
    Data,
    /// Control packet reserved for protocol runtime use.
    Control,
}
