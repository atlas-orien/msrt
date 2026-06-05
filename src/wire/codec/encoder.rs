//! Wire encoder boundary.

use crate::core::{Error, Packet, Result};

use crate::wire::{EnvelopeHeader, WireEnvelope};

/// Mutable output target for wire encoding.
pub struct EncodeTarget<'a> {
    bytes: &'a mut [u8],
    written: usize,
}

impl<'a> EncodeTarget<'a> {
    /// Creates an encode target from a caller-provided buffer.
    #[must_use]
    pub const fn new(bytes: &'a mut [u8]) -> Self {
        Self { bytes, written: 0 }
    }

    /// Returns the written bytes.
    #[must_use]
    pub fn written(&self) -> &[u8] {
        &self.bytes[..self.written]
    }

    /// Returns the number of written bytes.
    #[must_use]
    pub const fn written_len(&self) -> usize {
        self.written
    }

    /// Records bytes as written by an encoder implementation.
    pub fn set_written(&mut self, written: usize) -> Result<()> {
        if written > self.bytes.len() {
            return Err(Error::buffer_too_small());
        }

        self.written = written;
        Ok(())
    }

    /// Returns output capacity.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.bytes.len()
    }
}

/// Encodes MSRT packets into wire envelope bytes.
pub trait Encoder {
    /// Encodes a packet into the provided target.
    fn encode_packet(&mut self, packet: Packet<'_>, target: &mut EncodeTarget<'_>) -> Result<()>;

    /// Returns the number of bytes needed for a packet envelope.
    fn encoded_len(&self, packet: Packet<'_>) -> usize {
        EnvelopeHeader::new(packet.payload_len() as u8).total_len(crate::integrity::Crc16::TAG_LEN)
    }

    /// Builds a borrowed wire envelope descriptor for a packet payload.
    fn envelope_for<'a>(&self, packet: Packet<'a>) -> WireEnvelope<'a> {
        WireEnvelope::new(
            EnvelopeHeader::new(packet.payload_len() as u8),
            packet.payload.as_bytes(),
        )
    }
}
