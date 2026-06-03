//! Peer session slot shared by endpoint managers.

use crate::core::{MessageId, Result};
use crate::engine::{Engine, EngineConfig, EnginePoll, ReceiveReport};

const HELLO_MESSAGE: &[u8] = &[0];

/// Endpoint-level peer connection state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PeerState {
    /// No local engine exists for this peer.
    #[default]
    Disconnected,
    /// A local engine exists and a hello message is waiting for peer confirmation.
    Connecting,
    /// At least one valid packet has been observed from the peer.
    Connected,
}

/// Result of polling one endpoint peer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum EndpointPoll<'a> {
    /// Protocol bytes should be transmitted on the link.
    Transmit {
        /// Encoded protocol bytes to write to the link.
        bytes: &'a [u8],
        /// Send attempt count: 0 for a first send and greater than 0 for retransmits.
        attempts: u8,
    },
    /// A complete application message has been reassembled.
    Message(crate::engine::MessageEvent),
    /// A message could not be sent reliably.
    SendFailed(crate::engine::SendFailedEvent),
    /// The peer endpoint has no pending action.
    Idle,
}

/// A single peer session slot.
///
/// `Engine` represents one active peer session. `PeerSlot` owns that optional
/// engine and tracks whether the peer has actually confirmed connectivity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeerSlot {
    config: EngineConfig,
    engine: Option<Engine>,
    state: PeerState,
    last_seen_ms: u64,
}

impl PeerSlot {
    /// Creates an empty peer slot using `config` for future sessions.
    #[must_use]
    pub const fn new(config: EngineConfig) -> Self {
        Self {
            config,
            engine: None,
            state: PeerState::Disconnected,
            last_seen_ms: 0,
        }
    }

    /// Returns the endpoint-level peer state.
    #[must_use]
    pub const fn state(&self) -> PeerState {
        self.state
    }

    /// Returns whether this peer has confirmed connectivity.
    #[must_use]
    pub const fn is_connected(&self) -> bool {
        matches!(self.state, PeerState::Connected)
    }

    /// Returns whether a local engine currently exists for this peer.
    #[must_use]
    pub const fn has_session(&self) -> bool {
        self.engine.is_some()
    }

    /// Returns the last activity time recorded for the active session.
    #[must_use]
    pub const fn last_seen_ms(&self) -> u64 {
        self.last_seen_ms
    }

    /// Creates a fresh engine and queues a small hello message.
    pub fn connect(&mut self, now_ms: u64) -> Result<MessageId> {
        self.engine = Some(Engine::new(self.config));
        self.state = PeerState::Connecting;
        self.last_seen_ms = now_ms;
        self.engine
            .as_mut()
            .expect("engine was just inserted")
            .send(HELLO_MESSAGE)
    }

    /// Creates a fresh engine without queueing a hello message.
    pub fn accept_passive(&mut self, now_ms: u64) {
        self.engine = Some(Engine::new(self.config));
        self.state = PeerState::Connecting;
        self.last_seen_ms = now_ms;
    }

    /// Drops the current peer engine.
    pub fn disconnect(&mut self) {
        self.engine = None;
        self.state = PeerState::Disconnected;
        self.last_seen_ms = 0;
    }

    /// Creates an engine if needed and returns the active engine.
    pub fn engine_or_connect(&mut self, now_ms: u64) -> Result<&mut Engine> {
        if self.engine.is_none() {
            self.connect(now_ms)?;
        }

        self.last_seen_ms = now_ms;
        Ok(self.engine.as_mut().expect("engine exists"))
    }

    /// Creates a passive engine if needed and returns the active engine.
    pub fn engine_or_accept_passive(&mut self, now_ms: u64) -> &mut Engine {
        if self.engine.is_none() {
            self.accept_passive(now_ms);
        }

        self.last_seen_ms = now_ms;
        self.engine.as_mut().expect("engine exists")
    }

    /// Returns the active engine if a session exists.
    pub fn engine_mut(&mut self) -> Option<&mut Engine> {
        self.engine.as_mut()
    }

    /// Queues an application message on the active engine.
    pub fn send(&mut self, message: &[u8]) -> Result<Option<MessageId>> {
        let Some(engine) = self.engine.as_mut() else {
            return Ok(None);
        };

        engine.send(message).map(Some)
    }

    /// Feeds received bytes and updates peer state from valid peer traffic.
    pub fn receive(&mut self, now_ms: u64, bytes: &[u8]) -> ReceiveReport {
        let Some(engine) = self.engine.as_mut() else {
            return ReceiveReport::Incomplete { needed: None };
        };

        let report = engine.receive(bytes);
        if matches!(
            report,
            ReceiveReport::Packet { .. }
                | ReceiveReport::Duplicate { .. }
                | ReceiveReport::Ack { .. }
        ) {
            self.state = PeerState::Connected;
            self.last_seen_ms = now_ms;
        }

        report
    }

    /// Polls the active engine and disconnects the peer on reliable send failure.
    pub fn poll<'a>(&mut self, now_ms: u64, tx_buf: &'a mut [u8]) -> Result<EndpointPoll<'a>> {
        let Some(engine) = self.engine.as_mut() else {
            return Ok(EndpointPoll::Idle);
        };

        match engine.poll(now_ms, tx_buf)? {
            EnginePoll::Transmit { bytes, attempts } => {
                Ok(EndpointPoll::Transmit { bytes, attempts })
            }
            EnginePoll::Message(message) => {
                self.state = PeerState::Connected;
                self.last_seen_ms = now_ms;
                Ok(EndpointPoll::Message(message))
            }
            EnginePoll::SendFailed(event) => {
                self.disconnect();
                Ok(EndpointPoll::SendFailed(event))
            }
            EnginePoll::Idle => Ok(EndpointPoll::Idle),
        }
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

        assert!(!peer.has_session());
        assert!(!peer.is_connected());

        peer.engine_or_connect(10).unwrap();
        assert!(peer.has_session());
        assert_eq!(peer.state(), super::PeerState::Connecting);
        assert_eq!(peer.last_seen_ms(), 10);

        assert!(!peer.disconnect_if_idle(19, 10));
        assert!(peer.has_session());

        assert!(peer.disconnect_if_idle(20, 10));
        assert!(!peer.has_session());
    }

    #[test]
    fn reconnect_replaces_engine_state() {
        let mut peer = PeerSlot::default();

        peer.engine_or_connect(1).unwrap().send(b"hello").unwrap();
        peer.disconnect();
        let engine = peer.engine_or_connect(2).unwrap();

        assert_eq!(engine.send(b"hello").unwrap().get(), 1);
    }
}
