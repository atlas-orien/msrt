//! Passive single-peer endpoint manager.

use crate::core::{MessageId, Result};
use crate::endpoint::{EndpointPoll, PeerSlot};
use crate::engine::{Engine, EngineConfig, ReceiveReport};

/// Passive single-peer endpoint.
///
/// This endpoint owns at most one `Engine`. It does not actively send hello on
/// startup; instead, it creates the engine lazily when bytes arrive from the
/// peer. Disconnect drops the engine so the next peer connection starts with a
/// fresh protocol session.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PassiveEndpoint {
    peer: PeerSlot,
}

impl PassiveEndpoint {
    /// Creates a passive endpoint using `config` for each new peer session.
    #[must_use]
    pub const fn new(config: EngineConfig) -> Self {
        Self {
            peer: PeerSlot::new(config),
        }
    }

    /// Returns the single peer slot.
    #[must_use]
    pub const fn peer(&self) -> &PeerSlot {
        &self.peer
    }

    /// Returns the mutable single peer slot.
    pub fn peer_mut(&mut self) -> &mut PeerSlot {
        &mut self.peer
    }

    /// Drops the active host session if one exists.
    pub fn disconnect(&mut self) {
        self.peer.disconnect();
    }

    /// Returns the active engine, creating a passive session if needed.
    pub fn engine_or_accept(&mut self, now_ms: u64) -> &mut Engine {
        self.peer.engine_or_accept_passive(now_ms)
    }

    /// Returns the active engine if a peer session exists.
    pub fn engine_mut(&mut self) -> Option<&mut Engine> {
        self.peer.engine_mut()
    }

    /// Queues an application message if a host session exists.
    pub fn send(&mut self, message: &[u8]) -> Result<Option<MessageId>> {
        self.peer.send(message)
    }

    /// Creates a passive session if needed, then feeds received bytes into it.
    pub fn receive(&mut self, now_ms: u64, bytes: &[u8]) -> ReceiveReport {
        self.peer.engine_or_accept_passive(now_ms);
        self.peer.receive(now_ms, bytes)
    }

    /// Polls one endpoint action from the active host engine.
    pub fn poll<'a>(&mut self, now_ms: u64, tx_buf: &'a mut [u8]) -> Result<EndpointPoll<'a>> {
        self.peer.poll(now_ms, tx_buf)
    }

    /// Drops the active host session if it has been idle for at least `timeout_ms`.
    pub fn disconnect_if_idle(&mut self, now_ms: u64, timeout_ms: u64) -> bool {
        self.peer.disconnect_if_idle(now_ms, timeout_ms)
    }
}

#[cfg(test)]
mod tests {
    use crate::endpoint::{ClientEndpoint, EndpointPoll, PeerState};

    use super::PassiveEndpoint;

    #[test]
    fn passive_endpoint_starts_without_engine_and_accepts_first_peer_bytes() {
        let mut host = ClientEndpoint::default();
        let mut passive = PassiveEndpoint::default();
        let mut host_tx = [0; 128];
        let mut passive_tx = [0; 128];

        assert!(!passive.peer().has_session());

        host.connect(1).unwrap();
        let EndpointPoll::Transmit {
            bytes: hello_bytes, ..
        } = host.poll(1, &mut host_tx).unwrap()
        else {
            panic!("host should transmit hello");
        };

        passive.receive(2, hello_bytes);
        assert!(passive.peer().has_session());
        assert_eq!(passive.peer().state(), PeerState::Connected);

        let EndpointPoll::Transmit { bytes: ack_bytes, .. } =
            passive.poll(2, &mut passive_tx).unwrap()
        else {
            panic!("passive endpoint should transmit ack");
        };

        host.receive(3, ack_bytes);
        assert_eq!(host.peer().state(), PeerState::Connected);
    }

    #[test]
    fn passive_disconnect_drops_engine_until_next_receive() {
        let mut passive = PassiveEndpoint::default();

        passive.engine_or_accept(1);
        assert!(passive.peer().has_session());

        passive.disconnect();
        assert!(!passive.peer().has_session());
        assert_eq!(passive.peer().state(), PeerState::Disconnected);
    }
}
