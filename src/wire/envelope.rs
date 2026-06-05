//! Wire envelope primitives.

pub mod flags;
pub mod header;
pub mod magic;

pub use flags::WireFlags;
pub use header::{
    EnvelopeHeader, WIRE_HEADER_CRC_OFFSET, WIRE_HEADER_LEN, WIRE_MAGIC_LEN,
    WIRE_PACKET_LEN_OFFSET, header_crc,
};
pub use magic::EnvelopeMagic;

/// Borrowed wire envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WireEnvelope<'a> {
    /// Wire envelope header.
    pub header: EnvelopeHeader,
    /// Encoded MSRT packet bytes.
    pub packet_bytes: &'a [u8],
}

impl<'a> WireEnvelope<'a> {
    /// Creates a borrowed wire envelope.
    #[must_use]
    pub const fn new(header: EnvelopeHeader, packet_bytes: &'a [u8]) -> Self {
        Self {
            header,
            packet_bytes,
        }
    }

    /// Returns total envelope length using the fixed first-stage header size.
    #[must_use]
    pub const fn total_len(self, integrity_tag_len: usize) -> usize {
        WIRE_HEADER_LEN + self.packet_bytes.len() + integrity_tag_len
    }

    /// Returns whether the packet bytes length matches the header.
    #[must_use]
    pub fn has_valid_len(self) -> bool {
        usize::from(self.header.packet_len) == self.packet_bytes.len()
    }
}
