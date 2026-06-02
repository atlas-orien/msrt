//! Wire envelope primitives.

pub mod flags;
pub mod header;
pub mod magic;

pub use flags::WireFlags;
pub use header::{CHECKSUM_LEN, EnvelopeHeader, WIRE_HEADER_LEN};
pub use magic::EnvelopeMagic;

/// Borrowed wire envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WireEnvelope<'a> {
    /// Wire envelope header.
    pub header: EnvelopeHeader,
    /// Encoded SRT packet bytes.
    pub packet_bytes: &'a [u8],
    /// Checksum over the envelope bytes selected by the wire format.
    pub checksum: u16,
}

impl<'a> WireEnvelope<'a> {
    /// Creates a borrowed wire envelope.
    #[must_use]
    pub const fn new(header: EnvelopeHeader, packet_bytes: &'a [u8], checksum: u16) -> Self {
        Self {
            header,
            packet_bytes,
            checksum,
        }
    }

    /// Returns total envelope length using the fixed first-stage header size.
    #[must_use]
    pub const fn total_len(self) -> usize {
        WIRE_HEADER_LEN + self.packet_bytes.len() + CHECKSUM_LEN
    }

    /// Returns whether the packet bytes length matches the header.
    #[must_use]
    pub fn has_valid_len(self) -> bool {
        usize::from(self.header.packet_len) == self.packet_bytes.len()
    }
}
