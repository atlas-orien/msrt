//! Packet reliability state.

/// Known reliability state for a packet key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketState {
    /// The packet is unknown to the tracker.
    Unknown,
    /// The packet has been sent and is waiting for acknowledgement.
    InFlight,
    /// The packet has been acknowledged.
    Acked,
    /// The packet is considered lost by policy.
    Lost,
    /// The packet was intentionally dropped by policy.
    Dropped,
}
