//! Server-side endpoint manager.

use crate::endpoint::{EndpointPoll, PeerSlot};
use crate::core::Error;
use crate::engine::{Engine, EngineConfig, ReceiveReport};

/// Error returned when a server endpoint cannot accept a peer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceptError {
    /// All peer slots are occupied.
    Full,
    /// The peer engine failed to queue the endpoint hello message.
    Engine(Error),
}

/// One accepted server-side peer session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeerEntry<P> {
    peer_id: P,
    slot: PeerSlot,
}

impl<P> PeerEntry<P> {
    /// Returns the adapter-defined peer identifier.
    #[must_use]
    pub const fn peer_id(&self) -> &P {
        &self.peer_id
    }

    /// Returns the peer session slot.
    #[must_use]
    pub const fn slot(&self) -> &PeerSlot {
        &self.slot
    }

    /// Returns the mutable peer session slot.
    pub fn slot_mut(&mut self) -> &mut PeerSlot {
        &mut self.slot
    }
}

/// Server-side endpoint manager for a fixed number of peers.
///
/// `P` is a transport-adapter peer key, such as an index, UART port id, or UDP
/// remote address wrapper. This type does not listen on sockets and does not
/// perform IO; it only maps each peer key to one `Engine`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerEndpoint<P, const N: usize> {
    config: EngineConfig,
    peers: [Option<PeerEntry<P>>; N],
}

impl<P, const N: usize> ServerEndpoint<P, N>
where
    P: Copy + Eq,
{
    /// Creates an empty server endpoint.
    #[must_use]
    pub fn new(config: EngineConfig) -> Self {
        Self {
            config,
            peers: core::array::from_fn(|_| None),
        }
    }

    /// Accepts `peer_id`, replacing any existing session for that peer.
    ///
    /// Use this when the adapter has decided that the remote side started a new
    /// logical connection and old protocol state must be dropped.
    pub fn accept(
        &mut self,
        peer_id: P,
        now_ms: u64,
    ) -> core::result::Result<&mut Engine, AcceptError> {
        if let Some(index) = self.find_index(peer_id) {
            return self.reconnect_at(index, now_ms);
        }

        let index = self.vacant_index().ok_or(AcceptError::Full)?;
        self.peers[index] = Some(PeerEntry {
            peer_id,
            slot: PeerSlot::new(self.config),
        });

        self.reconnect_at(index, now_ms)
    }

    /// Returns the peer engine, accepting the peer if it is not already known.
    ///
    /// Use this for connectionless transports where receiving bytes from an
    /// unknown peer should lazily create that peer session.
    pub fn engine_or_accept(
        &mut self,
        peer_id: P,
        now_ms: u64,
    ) -> core::result::Result<&mut Engine, AcceptError> {
        if let Some(index) = self.find_index(peer_id) {
            let entry = self.peers[index].as_mut().expect("peer index exists");
            return entry.slot.engine_or_connect(now_ms).map_err(AcceptError::Engine);
        }

        self.accept(peer_id, now_ms)
    }

    /// Drops the peer session and frees the server slot.
    pub fn disconnect(&mut self, peer_id: P) -> bool {
        let Some(index) = self.find_index(peer_id) else {
            return false;
        };

        self.peers[index] = None;
        true
    }

    /// Returns the active engine for `peer_id` if the peer is known.
    pub fn engine_mut(&mut self, peer_id: P) -> Option<&mut Engine> {
        let index = self.find_index(peer_id)?;
        self.peers[index].as_mut()?.slot.engine_mut()
    }

    /// Feeds received bytes into the engine for `peer_id`.
    pub fn receive(&mut self, peer_id: P, now_ms: u64, bytes: &[u8]) -> Option<ReceiveReport> {
        let index = self.find_index(peer_id)?;
        Some(self.peers[index].as_mut()?.slot.receive(now_ms, bytes))
    }

    /// Polls one endpoint action for `peer_id`.
    pub fn poll<'a>(
        &mut self,
        peer_id: P,
        now_ms: u64,
        tx_buf: &'a mut [u8],
    ) -> Option<crate::core::Result<EndpointPoll<'a>>> {
        let index = self.find_index(peer_id)?;
        Some(self.peers[index].as_mut()?.slot.poll(now_ms, tx_buf))
    }

    /// Returns the peer slot for `peer_id` if the peer is known.
    pub fn peer_mut(&mut self, peer_id: P) -> Option<&mut PeerSlot> {
        let index = self.find_index(peer_id)?;
        Some(&mut self.peers[index].as_mut()?.slot)
    }

    /// Drops every peer session idle for at least `timeout_ms`.
    pub fn disconnect_idle(&mut self, now_ms: u64, timeout_ms: u64) -> usize {
        let mut disconnected = 0;

        for peer in &mut self.peers {
            let Some(entry) = peer else {
                continue;
            };

            if entry.slot.disconnect_if_idle(now_ms, timeout_ms) {
                *peer = None;
                disconnected += 1;
            }
        }

        disconnected
    }

    /// Returns all currently accepted peers.
    pub fn peers(&self) -> impl Iterator<Item = &PeerEntry<P>> {
        self.peers.iter().filter_map(Option::as_ref)
    }

    fn reconnect_at(
        &mut self,
        index: usize,
        now_ms: u64,
    ) -> core::result::Result<&mut Engine, AcceptError> {
        let entry = self.peers[index].as_mut().expect("peer index exists");
        entry.slot.connect(now_ms).map_err(AcceptError::Engine)?;
        Ok(entry.slot.engine_mut().expect("engine was just inserted"))
    }

    fn find_index(&self, peer_id: P) -> Option<usize> {
        self.peers
            .iter()
            .position(|peer| peer.as_ref().is_some_and(|entry| entry.peer_id == peer_id))
    }

    fn vacant_index(&self) -> Option<usize> {
        self.peers.iter().position(Option::is_none)
    }
}

impl<P, const N: usize> Default for ServerEndpoint<P, N>
where
    P: Copy + Eq,
{
    fn default() -> Self {
        Self::new(EngineConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::{AcceptError, ServerEndpoint};

    #[test]
    fn server_accepts_multiple_peers() {
        let mut endpoint = ServerEndpoint::<u8, 2>::default();

        endpoint.engine_or_accept(1, 10).unwrap();
        endpoint.engine_or_accept(2, 20).unwrap();

        assert!(endpoint.peer_mut(1).unwrap().has_session());
        assert!(endpoint.peer_mut(2).unwrap().has_session());
        assert_eq!(endpoint.peers().count(), 2);
    }

    #[test]
    fn server_reports_full_when_slots_are_occupied() {
        let mut endpoint = ServerEndpoint::<u8, 1>::default();

        endpoint.engine_or_accept(1, 10).unwrap();

        assert_eq!(endpoint.engine_or_accept(2, 20), Err(AcceptError::Full));
    }

    #[test]
    fn accept_replaces_existing_peer_engine() {
        let mut endpoint = ServerEndpoint::<u8, 1>::default();

        endpoint
            .engine_or_accept(1, 1)
            .unwrap()
            .send(b"hello")
            .unwrap();
        let engine = endpoint.accept(1, 2).unwrap();

        assert_eq!(engine.send(b"hello").unwrap().get(), 1);
    }

    #[test]
    fn server_disconnects_idle_peers() {
        let mut endpoint = ServerEndpoint::<u8, 2>::default();

        endpoint.engine_or_accept(1, 10).unwrap();
        endpoint.engine_or_accept(2, 20).unwrap();

        assert_eq!(endpoint.disconnect_idle(19, 10), 0);
        assert_eq!(endpoint.disconnect_idle(20, 10), 1);
        assert!(endpoint.peer_mut(1).is_none());
        assert!(endpoint.peer_mut(2).is_some());
    }
}
