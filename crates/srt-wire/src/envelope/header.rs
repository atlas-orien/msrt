//! Wire envelope header.

use super::{EnvelopeMagic, WireFlags};

/// Fixed first-stage wire header length.
pub const WIRE_HEADER_LEN: usize = 8;

/// Wire envelope header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnvelopeHeader {
    /// Magic used to locate an envelope in a byte stream.
    pub magic: EnvelopeMagic,
    /// Wire format version.
    pub version: u8,
    /// Wire envelope header length.
    pub header_len: u8,
    /// Encoded SRT packet length.
    pub packet_len: u16,
    /// Wire flags.
    pub flags: WireFlags,
    /// Reserved byte for future extension.
    pub reserved: u8,
}

impl EnvelopeHeader {
    /// Current first-stage wire version.
    pub const VERSION: u8 = 1;

    /// Creates a wire envelope header.
    #[must_use]
    pub const fn new(packet_len: u16, flags: WireFlags) -> Self {
        Self {
            magic: EnvelopeMagic::SRT,
            version: Self::VERSION,
            header_len: WIRE_HEADER_LEN as u8,
            packet_len,
            flags,
            reserved: 0,
        }
    }

    /// Returns whether this header has the supported version.
    #[must_use]
    pub const fn is_supported_version(self) -> bool {
        self.version == Self::VERSION
    }

    /// Returns whether this header length matches the first-stage format.
    #[must_use]
    pub const fn has_supported_header_len(self) -> bool {
        self.header_len as usize == WIRE_HEADER_LEN
    }

    /// Returns the complete envelope length including checksum bytes.
    #[must_use]
    pub const fn total_len(self) -> usize {
        WIRE_HEADER_LEN + self.packet_len as usize + core::mem::size_of::<u16>()
    }
}

#[cfg(test)]
mod tests {
    use super::{EnvelopeHeader, WIRE_HEADER_LEN};
    use crate::WireFlags;

    #[test]
    fn header_total_len_includes_checksum() {
        let header = EnvelopeHeader::new(9, WireFlags::CHECKSUM_PRESENT);

        assert_eq!(usize::from(header.header_len), WIRE_HEADER_LEN);
        assert_eq!(header.total_len(), WIRE_HEADER_LEN + 9 + 2);
    }
}
