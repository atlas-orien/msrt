//! Client-side endpoint manager.

use crate::endpoint::PeerSlot;
use crate::engine::{Engine, EngineConfig};

/// Client-side endpoint with at most one active peer session.
///
/// This is the frontend-shaped endpoint manager: the adapter owns the actual
/// link, while this type owns the single `Engine` used for the current peer
/// session. Reconnect means dropping the old engine and creating a fresh one.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ClientEndpoint {
    peer: PeerSlot,
}

impl ClientEndpoint {
    /// Creates a client endpoint using `config` for each new session.
    #[must_use]
    pub const fn new(config: EngineConfig) -> Self {
        Self {
            peer: PeerSlot::new(config),
        }
    }

    /// Returns the client peer slot.
    #[must_use]
    pub const fn peer(&self) -> &PeerSlot {
        &self.peer
    }

    /// Returns the mutable client peer slot.
    pub fn peer_mut(&mut self) -> &mut PeerSlot {
        &mut self.peer
    }

    /// Starts a fresh session and returns its engine.
    pub fn connect(&mut self, now_ms: u64) -> &mut Engine {
        self.peer.connect(now_ms)
    }

    /// Drops the active session if one exists.
    pub fn disconnect(&mut self) {
        self.peer.disconnect();
    }

    /// Returns the active engine, creating a fresh session if needed.
    pub fn engine_or_connect(&mut self, now_ms: u64) -> &mut Engine {
        self.peer.engine_or_connect(now_ms)
    }

    /// Returns the active engine if the client is connected.
    pub fn engine_mut(&mut self) -> Option<&mut Engine> {
        self.peer.engine_mut()
    }

    /// Drops the active session if it has been idle for at least `timeout_ms`.
    pub fn disconnect_if_idle(&mut self, now_ms: u64, timeout_ms: u64) -> bool {
        self.peer.disconnect_if_idle(now_ms, timeout_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::ClientEndpoint;

    #[test]
    fn client_reconnect_creates_fresh_engine() {
        let mut endpoint = ClientEndpoint::default();

        endpoint.engine_or_connect(1).send(b"hello").unwrap();
        endpoint.disconnect();
        let engine = endpoint.engine_or_connect(2);

        assert_eq!(engine.send(b"hello").unwrap().get(), 0);
    }
}
