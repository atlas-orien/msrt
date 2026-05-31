//! Receive-side engine boundaries.

use srt_core::{Packet, PacketNumber, Result};
use srt_reliability::DedupDecision;

/// Raw bytes received from the lower link boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReceiveInput<'a> {
    /// Borrowed bytes from the lower layer.
    pub bytes: &'a [u8],
}

impl<'a> ReceiveInput<'a> {
    /// Creates receive input from borrowed bytes.
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    /// Returns whether the input is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.bytes.is_empty()
    }
}

/// Decoded packet input passed into receive-side engine logic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PacketInput<'a> {
    /// Borrowed protocol packet.
    pub packet: Packet<'a>,
}

impl<'a> PacketInput<'a> {
    /// Creates packet input.
    #[must_use]
    pub const fn new(packet: Packet<'a>) -> Self {
        Self { packet }
    }

    /// Returns the packet number.
    #[must_use]
    pub const fn packet_number(self) -> PacketNumber {
        self.packet.header.packet_number
    }
}

/// First receive-side decision for an incoming packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiveAction {
    /// Process the packet and its frames.
    Process {
        /// Packet number accepted for processing.
        packet_number: PacketNumber,
    },
    /// Ignore the packet as duplicate.
    Duplicate {
        /// Duplicate packet number.
        packet_number: PacketNumber,
    },
    /// Drop the packet because it violates receive policy.
    Drop {
        /// Dropped packet number.
        packet_number: PacketNumber,
    },
}

impl ReceiveAction {
    /// Creates a receive action from a deduplication decision.
    #[must_use]
    pub const fn from_dedup(packet_number: PacketNumber, decision: DedupDecision) -> Self {
        match decision {
            DedupDecision::Accept => Self::Process { packet_number },
            DedupDecision::Duplicate => Self::Duplicate { packet_number },
        }
    }
}

/// Receive-side protocol boundary.
pub trait Receiver {
    /// Accepts decoded packet input and advances receive-side protocol state.
    fn receive_packet(&mut self, input: PacketInput<'_>) -> Result<ReceiveAction>;

    /// Accepts lower-layer bytes when a concrete wire boundary is present.
    fn receive_bytes(&mut self, input: ReceiveInput<'_>) -> Result<()>;
}
