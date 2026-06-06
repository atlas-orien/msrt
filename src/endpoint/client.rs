//! Client-side endpoint manager.

use crate::core::{MessageId, Result};
use crate::endpoint::{EndpointPoll, PeerSlot};
use crate::engine::{Engine, EngineConfig, ReceiveReport};

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
    pub fn connect(&mut self, now_ms: u64) -> Result<MessageId> {
        self.peer.connect(now_ms)
    }

    /// Drops the active session if one exists.
    pub fn disconnect(&mut self) {
        self.peer.disconnect();
    }

    /// Returns the active engine, creating a fresh session if needed.
    pub fn engine_or_connect(&mut self, now_ms: u64) -> Result<&mut Engine> {
        self.peer.engine_or_connect(now_ms)
    }

    /// Returns the active engine if the client is connected.
    pub fn engine_mut(&mut self) -> Option<&mut Engine> {
        self.peer.engine_mut()
    }

    /// Feeds received bytes into the active peer engine.
    pub fn receive(&mut self, now_ms: u64, bytes: &[u8]) -> ReceiveReport {
        self.peer.receive(now_ms, bytes)
    }

    /// Polls one endpoint action from the active peer engine.
    pub fn poll<'a>(&mut self, now_ms: u64, tx_buf: &'a mut [u8]) -> Result<EndpointPoll<'a>> {
        self.peer.poll(now_ms, tx_buf)
    }

    /// Drops the active session if it has been idle for at least `timeout_ms`.
    pub fn disconnect_if_idle(&mut self, now_ms: u64, timeout_ms: u64) -> bool {
        self.peer.disconnect_if_idle(now_ms, timeout_ms)
    }
}

#[cfg(test)]
mod tests {
    use crate::endpoint::{EndpointPoll, PeerState, ServerEndpoint};

    use super::ClientEndpoint;

    #[test]
    fn client_reconnect_creates_fresh_engine() {
        let mut endpoint = ClientEndpoint::default();

        let first_engine = endpoint
            .engine_or_connect(1)
            .unwrap();
        first_engine.send(b"hello").unwrap();
        let expected_after_reconnect = first_engine.send(b"hello").unwrap();
        endpoint.disconnect();
        let engine = endpoint.engine_or_connect(2).unwrap();

        assert_eq!(engine.send(b"hello").unwrap(), expected_after_reconnect);
    }

    #[test]
    fn hello_ack_marks_both_endpoints_connected() {
        let mut client = ClientEndpoint::default();
        let mut server = ServerEndpoint::<u8, 1>::default();
        let mut client_tx = [0; 128];
        let mut server_tx = [0; 128];

        client.connect(1).unwrap();
        let EndpointPoll::Transmit {
            bytes: hello_bytes, ..
        } = client.poll(1, &mut client_tx).unwrap()
        else {
            panic!("client should transmit hello");
        };

        server.engine_or_accept(7, 1).unwrap();
        server.receive(7, 2, hello_bytes).unwrap();
        assert_eq!(server.peer_mut(7).unwrap().state(), PeerState::Connected);

        let EndpointPoll::Transmit {
            bytes: ack_bytes, ..
        } = server.poll(7, 2, &mut server_tx).unwrap().unwrap()
        else {
            panic!("server should transmit ack");
        };

        client.receive(3, ack_bytes);
        assert_eq!(client.peer().state(), PeerState::Connected);
    }

    #[test]
    fn idle_connected_endpoint_sends_ping_and_accepts_pong() {
        let mut client = ClientEndpoint::default();
        let mut passive = crate::endpoint::PassiveEndpoint::default();
        let mut client_tx = [0; 128];
        let mut passive_tx = [0; 128];

        client.connect(1).unwrap();
        let EndpointPoll::Transmit { bytes, .. } = client.poll(1, &mut client_tx).unwrap() else {
            panic!("client should transmit hello");
        };
        passive.receive(2, bytes);
        let EndpointPoll::Transmit { bytes, .. } = passive.poll(2, &mut passive_tx).unwrap() else {
            panic!("passive endpoint should transmit ack");
        };
        client.receive(3, bytes);

        assert_eq!(client.peer().state(), PeerState::Connected);

        let EndpointPoll::Transmit {
            bytes: ping_bytes, ..
        } = client.poll(5_004, &mut client_tx).unwrap()
        else {
            panic!("client should transmit ping after idle interval");
        };

        assert!(matches!(
            passive.receive(1_005, ping_bytes),
            crate::engine::ReceiveReport::Ping
        ));

        let pong_bytes = loop {
            match passive.poll(5_005, &mut passive_tx).unwrap() {
                EndpointPoll::Transmit { bytes, .. } => break bytes,
                EndpointPoll::Message(_) => {}
                other => panic!("passive endpoint should transmit pong, got {other:?}"),
            }
        };

        assert!(matches!(
            client.receive(5_006, pong_bytes),
            crate::engine::ReceiveReport::Pong
        ));
        assert_eq!(client.peer().state(), PeerState::Connected);
    }

    #[test]
    fn missing_pong_disconnects_endpoint_after_idle_timeout() {
        let mut client = ClientEndpoint::default();
        let mut passive = crate::endpoint::PassiveEndpoint::default();
        let mut client_tx = [0; 128];
        let mut passive_tx = [0; 128];

        client.connect(1).unwrap();
        let EndpointPoll::Transmit { bytes, .. } = client.poll(1, &mut client_tx).unwrap() else {
            panic!("client should transmit hello");
        };
        passive.receive(2, bytes);
        let EndpointPoll::Transmit { bytes, .. } = passive.poll(2, &mut passive_tx).unwrap() else {
            panic!("passive endpoint should transmit ack");
        };
        client.receive(3, bytes);

        assert!(matches!(
            client.poll(5_004, &mut client_tx).unwrap(),
            EndpointPoll::Transmit { .. }
        ));
        assert!(matches!(
            client.poll(5_014, &mut client_tx).unwrap(),
            EndpointPoll::Idle
        ));
        assert!(client.disconnect_if_idle(10_004, 10_001));
        assert_eq!(client.peer().state(), PeerState::Disconnected);
    }
}
