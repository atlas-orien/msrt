#![doc = "Protocol engine boundaries for MSRT."]

pub(crate) mod config;
pub(crate) mod layout;
pub(crate) mod machine;

pub use config::{ChannelProfile, ChannelSpec, EngineConfig};

use crate::core::{ChannelId, Error, MessageId, PacketNumber, Result};
use machine::Machine;

/// Minimal non-blocking MSRT protocol engine.
///
/// The engine owns protocol state. It splits outgoing messages into packet
/// write events, accepts incoming wire bytes, and emits complete messages once
/// reassembly succeeds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Engine {
    pub(crate) config: EngineConfig,
    pub(crate) machine: Machine,
}

impl Engine {
    /// Creates an engine.
    #[must_use]
    pub const fn new(config: EngineConfig) -> Self {
        Self {
            config,
            machine: Machine::new(config.initial_packet_number, config.initial_message_id),
        }
    }

    /// Polls one high-level engine action.
    ///
    /// Write events are copied into `tx_buf` and returned as a borrowed byte
    /// slice so callers can pass the buffer directly to their link layer.
    pub fn poll<'a>(&mut self, now_ms: u64, tx_buf: &'a mut [u8]) -> Result<EnginePoll<'a>> {
        Machine::poll(self, now_ms, tx_buf)
    }

    /// Queues a complete message for non-blocking protocol transmission.
    ///
    /// The caller submits the complete message once. The engine splits it into
    /// packet-sized write events internally.
    pub fn send(&mut self, message: &[u8]) -> Result<MessageId> {
        self.send_on(ChannelId::DEFAULT, message)
    }

    /// Queues a complete message on a logical channel.
    ///
    /// This is the channel-aware form of [`Engine::send`].
    pub fn send_on(&mut self, channel_id: ChannelId, message: &[u8]) -> Result<MessageId> {
        Machine::send_on(self, channel_id, message)
    }

    /// Feeds already-arrived wire bytes into the engine.
    ///
    /// This method never waits for more bytes. It handles the current input and
    /// queues events if a complete message becomes available.
    pub fn receive(&mut self, bytes: &[u8]) -> ReceiveReport {
        Machine::receive(self, bytes)
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new(EngineConfig::default())
    }
}

/// High-level action returned by [`Engine::poll`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnginePoll<'a> {
    /// Protocol bytes should be transmitted on the link.
    Transmit(&'a [u8]),
    /// A complete application message has been reassembled.
    Message(MessageEvent),
    /// A message could not be sent reliably.
    SendFailed(SendFailedEvent),
    /// The engine has no pending action.
    Idle,
}

/// A complete message delivered by the engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MessageEvent {
    /// Logical channel that carried the message.
    pub channel_id: ChannelId,
    /// Protocol-level purpose associated with the channel.
    pub profile: ChannelProfile,
    /// Message identifier scoped to this engine.
    pub message_id: MessageId,
    /// Fixed storage containing complete message bytes.
    pub bytes: [u8; 256],
    /// Number of valid message bytes in `bytes`.
    pub len: usize,
}

impl MessageEvent {
    /// Returns the valid message bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8] {
        self.bytes.split_at(self.len).0
    }
}

/// A reliable send failure produced by the engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SendFailedEvent {
    /// Logical channel whose message failed.
    pub channel_id: ChannelId,
    /// Message identifier that failed.
    pub message_id: MessageId,
    /// Failure reason.
    pub reason: SendFailureReason,
}

/// Reason a reliable send failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SendFailureReason {
    /// At least one packet for the message reached the configured retransmission attempt limit.
    RetryLimitReached,
}

/// Result of `Engine::receive`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiveReport {
    /// A packet envelope was accepted.
    Packet {
        /// Packet number decoded from the envelope.
        packet_number: PacketNumber,
    },
    /// A duplicate packet envelope was acknowledged but not processed again.
    Duplicate {
        /// Duplicate packet number.
        packet_number: PacketNumber,
    },
    /// An ACK packet was accepted.
    Ack {
        /// Packet number acknowledged by the peer.
        packet_number: PacketNumber,
    },
    /// The input did not contain a valid magic prefix at offset zero.
    Noise {
        /// Number of bytes treated as noise.
        skipped: usize,
    },
    /// The envelope checksum failed.
    Corrupted,
    /// The envelope is incomplete.
    Incomplete {
        /// Number of bytes required if known.
        needed: Option<usize>,
    },
    /// The packet was valid but could not be applied to engine state.
    Error(Error),
}
