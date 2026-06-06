//! Packet receive-side state.

use crate::engine::config::MAX_IN_FLIGHT_PACKETS;
use crate::reliability::PacketDedup;

/// Receive-side duplicate suppression state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReceiveState {
    dedup: PacketDedup<MAX_IN_FLIGHT_PACKETS>,
}

impl ReceiveState {
    pub(crate) const fn new() -> Self {
        Self {
            dedup: PacketDedup::new(),
        }
    }

    pub(crate) fn dedup(&mut self) -> &mut PacketDedup<MAX_IN_FLIGHT_PACKETS> {
        &mut self.dedup
    }
}
