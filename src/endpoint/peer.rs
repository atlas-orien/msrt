//! Peer session slot shared by endpoint managers.

use crate::engine::{Engine, EngineConfig};

/// A single peer session slot.
///
/// `Engine` represents one active peer session. `PeerSlot` owns that optional
/// engine and makes the session lifecycle explicit: connect creates protocol
/// state, disconnect drops it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeerSlot {
    config: EngineConfig,
    engine: Option<Engine>,
    last_seen_ms: u64,
}

impl PeerSlot {
    /// Creates an empty peer slot using `config` for future sessions.
    #[must_use]
    pub const fn new(config: EngineConfig) -> Self {
        Self {
            config,
            engine: None,
            last_seen_ms: 0,
        }
    }

    /// Returns whether a peer session is currently active.
    #[must_use]
    pub const fn is_connected(&self) -> bool {
        self.engine.is_some()
    }

    /// Returns the last activity time recorded for the active session.
    #[must_use]
    pub const fn last_seen_ms(&self) -> u64 {
        self.last_seen_ms
    }

    /// Creates a fresh engine for this peer and records activity time.
    pub fn connect(&mut self, now_ms: u64) -> &mut Engine {
        self.engine = Some(Engine::new(self.config));
        self.last_seen_ms = now_ms;
        self.engine.as_mut().expect("engine was just inserted")
    }

    /// Drops the current peer engine.
    pub fn disconnect(&mut self) {
        self.engine = None;
        self.last_seen_ms = 0;
    }

    /// Returns the active engine, creating one if needed.
    pub fn engine_or_connect(&mut self, now_ms: u64) -> &mut Engine {
        if self.engine.is_none() {
            return self.connect(now_ms);
        }

        self.last_seen_ms = now_ms;
        self.engine.as_mut().expect("engine exists")
    }

    /// Returns the active engine if a session exists.
    pub fn engine_mut(&mut self) -> Option<&mut Engine> {
        self.engine.as_mut()
    }

    /// Drops the peer session if it has been idle for at least `timeout_ms`.
    pub fn disconnect_if_idle(&mut self, now_ms: u64, timeout_ms: u64) -> bool {
        if self.engine.is_some()
            && now_ms.saturating_sub(self.last_seen_ms) >= timeout_ms
        {
            self.disconnect();
            return true;
        }

        false
    }
}

impl Default for PeerSlot {
    fn default() -> Self {
        Self::new(EngineConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::PeerSlot;

    #[test]
    fn peer_slot_creates_and_drops_engine() {
        let mut peer = PeerSlot::default();

        assert!(!peer.is_connected());

        peer.engine_or_connect(10);
        assert!(peer.is_connected());
        assert_eq!(peer.last_seen_ms(), 10);

        assert!(!peer.disconnect_if_idle(19, 10));
        assert!(peer.is_connected());

        assert!(peer.disconnect_if_idle(20, 10));
        assert!(!peer.is_connected());
    }

    #[test]
    fn reconnect_replaces_engine_state() {
        let mut peer = PeerSlot::default();

        peer.engine_or_connect(1).send(b"hello").unwrap();
        peer.disconnect();
        let engine = peer.engine_or_connect(2);

        assert_eq!(engine.send(b"hello").unwrap().get(), 0);
    }
}
