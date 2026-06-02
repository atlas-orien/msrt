//! Wire decoder boundary.

use crate::core::{Packet, Result};

use crate::wire::{EnvelopeHeader, WIRE_HEADER_LEN};

/// Result of feeding bytes into a wire decoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodeOutcome<'a> {
    /// More bytes are required before an envelope can be decoded.
    NeedMore {
        /// Minimum number of additional bytes needed if known.
        additional: Option<usize>,
    },
    /// A complete packet was decoded.
    Packet {
        /// Decoded MSRT packet.
        packet: Packet<'a>,
        /// Number of bytes consumed by the wire envelope.
        consumed: usize,
    },
    /// Bytes were rejected and decoder should resynchronize.
    Resync {
        /// Number of bytes consumed or skipped.
        consumed: usize,
    },
}

/// Decodes wire envelope bytes into MSRT packets.
pub trait Decoder {
    /// Attempts to decode one packet from the provided bytes.
    fn decode<'a>(&mut self, bytes: &'a [u8]) -> Result<DecodeOutcome<'a>>;

    /// Returns whether enough bytes exist for a first-stage wire header.
    fn has_header(&self, bytes: &[u8]) -> bool {
        bytes.len() >= WIRE_HEADER_LEN
    }

    /// Returns the expected complete envelope length once a header is known.
    fn expected_len(&self, header: EnvelopeHeader) -> usize {
        header.total_len()
    }
}
