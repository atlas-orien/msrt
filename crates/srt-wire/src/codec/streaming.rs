//! Fixed-buffer streaming wire decoder.

use srt_core::{Error, Result};

use crate::{CHECKSUM_LEN, Checksum, EnvelopeHeader, EnvelopeMagic, WIRE_HEADER_LEN, WireFlags};

/// Result of feeding bytes into a streaming wire decoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamDecodeOutcome<'a> {
    /// More bytes are required before a complete envelope can be decoded.
    NeedMore {
        /// Minimum number of additional bytes needed if known.
        additional: Option<usize>,
    },
    /// A complete encoded SRT packet was decoded from the wire envelope.
    Packet {
        /// Encoded packet bytes inside the accepted wire envelope.
        packet_bytes: &'a [u8],
        /// Number of wire bytes consumed by this envelope.
        consumed: usize,
    },
    /// Non-envelope bytes were skipped while searching for magic.
    Noise {
        /// Number of bytes skipped.
        skipped: usize,
    },
    /// A candidate envelope was complete but failed validation.
    Corrupted {
        /// Number of bytes consumed by the rejected candidate.
        consumed: usize,
    },
    /// A candidate envelope header is unsupported and decoder resynchronized.
    Resync {
        /// Number of bytes skipped while resynchronizing.
        skipped: usize,
    },
}

/// Streaming decoder for serial byte streams.
///
/// The decoder owns a fixed-size buffer and never allocates. It accepts any
/// byte chunks supplied by the caller, including half packets, sticky packets,
/// noise, and multiple packets in one receive pass.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamingDecoder<const N: usize> {
    bytes: [u8; N],
    len: usize,
    pending_consume: usize,
}

impl<const N: usize> StreamingDecoder<N> {
    /// Creates an empty streaming decoder.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bytes: [0; N],
            len: 0,
            pending_consume: 0,
        }
    }

    /// Feeds bytes into the decoder and attempts to decode one packet.
    pub fn feed<'a>(
        &'a mut self,
        bytes: &[u8],
        checksum: &impl Checksum,
    ) -> Result<StreamDecodeOutcome<'a>> {
        self.consume_pending();
        self.append(bytes)?;
        self.decode_buffer(checksum)
    }

    /// Returns the number of buffered bytes.
    #[must_use]
    pub const fn buffered_len(&self) -> usize {
        self.len
    }

    /// Returns fixed buffer capacity.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Clears all buffered bytes and any pending consumed envelope.
    pub const fn clear(&mut self) {
        self.len = 0;
        self.pending_consume = 0;
    }

    fn append(&mut self, bytes: &[u8]) -> Result<()> {
        if self.len + bytes.len() > N {
            return Err(Error::buffer_too_small());
        }

        let end = self.len + bytes.len();
        self.bytes[self.len..end].copy_from_slice(bytes);
        self.len = end;

        Ok(())
    }

    fn decode_buffer<'a>(
        &'a mut self,
        checksum: &impl Checksum,
    ) -> Result<StreamDecodeOutcome<'a>> {
        if self.len == 0 {
            return Ok(StreamDecodeOutcome::NeedMore { additional: None });
        }

        let Some(offset) = find_magic(&self.bytes[..self.len]) else {
            let skipped = self.len;
            self.len = 0;
            return Ok(StreamDecodeOutcome::Noise { skipped });
        };

        if offset > 0 {
            self.consume(offset);
            return Ok(StreamDecodeOutcome::Noise { skipped: offset });
        }

        if self.len < WIRE_HEADER_LEN {
            return Ok(StreamDecodeOutcome::NeedMore {
                additional: Some(WIRE_HEADER_LEN - self.len),
            });
        }

        let Some(header) = header_from_bytes(&self.bytes[..WIRE_HEADER_LEN]) else {
            self.consume(1);
            return Ok(StreamDecodeOutcome::Resync { skipped: 1 });
        };

        if !header.is_supported_version()
            || !header.has_supported_header_len()
            || !header.flags.contains(WireFlags::CHECKSUM_PRESENT)
        {
            self.consume(1);
            return Ok(StreamDecodeOutcome::Resync { skipped: 1 });
        }

        let total_len = header.total_len();

        if total_len > N {
            self.consume(1);
            return Ok(StreamDecodeOutcome::Resync { skipped: 1 });
        }

        if self.len < total_len {
            return Ok(StreamDecodeOutcome::NeedMore {
                additional: Some(total_len - self.len),
            });
        }

        let expected = u16::from_le_bytes([
            self.bytes[total_len - CHECKSUM_LEN],
            self.bytes[total_len - CHECKSUM_LEN + 1],
        ]);

        if !checksum.verify(&self.bytes[..total_len - CHECKSUM_LEN], expected) {
            self.pending_consume = total_len;
            return Ok(StreamDecodeOutcome::Corrupted {
                consumed: total_len,
            });
        }

        let packet_start = WIRE_HEADER_LEN;
        let packet_end = packet_start + usize::from(header.packet_len);
        self.pending_consume = total_len;

        Ok(StreamDecodeOutcome::Packet {
            packet_bytes: &self.bytes[packet_start..packet_end],
            consumed: total_len,
        })
    }

    fn consume_pending(&mut self) {
        if self.pending_consume > 0 {
            let pending = self.pending_consume;
            self.pending_consume = 0;
            self.consume(pending);
        }
    }

    fn consume(&mut self, count: usize) {
        let count = core::cmp::min(count, self.len);
        self.bytes.copy_within(count..self.len, 0);
        self.len -= count;
    }
}

impl<const N: usize> Default for StreamingDecoder<N> {
    fn default() -> Self {
        Self::new()
    }
}

fn find_magic(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(EnvelopeMagic::SRT.bytes().len())
        .position(|window| window == EnvelopeMagic::SRT.bytes())
}

fn header_from_bytes(bytes: &[u8]) -> Option<EnvelopeHeader> {
    let magic = [*bytes.first()?, *bytes.get(1)?];

    if magic != EnvelopeMagic::SRT.bytes() {
        return None;
    }

    Some(EnvelopeHeader {
        magic: EnvelopeMagic::SRT,
        version: *bytes.get(2)?,
        header_len: *bytes.get(3)?,
        packet_len: u16::from_le_bytes([*bytes.get(4)?, *bytes.get(5)?]),
        flags: WireFlags::from_bits(*bytes.get(6)?),
        reserved: *bytes.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::{StreamDecodeOutcome, StreamingDecoder};
    use crate::{Checksum, Crc16, EnvelopeHeader, EnvelopeMagic, WireFlags};

    const BUFFER_BYTES: usize = 64;

    #[test]
    fn decoder_waits_for_half_packet() {
        let mut decoder = StreamingDecoder::<BUFFER_BYTES>::new();
        let packet = wire_packet(b"hello");
        let split = 4;

        assert_eq!(
            decoder.feed(&packet.as_bytes()[..split], &Crc16).unwrap(),
            StreamDecodeOutcome::NeedMore {
                additional: Some(crate::WIRE_HEADER_LEN - split)
            }
        );

        assert_eq!(
            decoder.feed(&packet.as_bytes()[split..], &Crc16).unwrap(),
            StreamDecodeOutcome::Packet {
                packet_bytes: b"hello",
                consumed: packet.len
            }
        );
    }

    #[test]
    fn decoder_handles_sticky_packets() {
        let mut decoder = StreamingDecoder::<BUFFER_BYTES>::new();
        let first = wire_packet(b"one");
        let second = wire_packet(b"two");
        let mut sticky = [0; BUFFER_BYTES];
        sticky[..first.len].copy_from_slice(first.as_bytes());
        sticky[first.len..first.len + second.len].copy_from_slice(second.as_bytes());

        assert_eq!(
            decoder
                .feed(&sticky[..first.len + second.len], &Crc16)
                .unwrap(),
            StreamDecodeOutcome::Packet {
                packet_bytes: b"one",
                consumed: first.len
            }
        );
        assert_eq!(
            decoder.feed(&[], &Crc16).unwrap(),
            StreamDecodeOutcome::Packet {
                packet_bytes: b"two",
                consumed: second.len
            }
        );
    }

    #[test]
    fn decoder_skips_noise_before_packet() {
        let mut decoder = StreamingDecoder::<BUFFER_BYTES>::new();
        let packet = wire_packet(b"ok");
        let mut bytes = [0; BUFFER_BYTES];
        bytes[..3].copy_from_slice(b"bad");
        bytes[3..3 + packet.len].copy_from_slice(packet.as_bytes());

        assert_eq!(
            decoder.feed(&bytes[..3 + packet.len], &Crc16).unwrap(),
            StreamDecodeOutcome::Noise { skipped: 3 }
        );
        assert_eq!(
            decoder.feed(&[], &Crc16).unwrap(),
            StreamDecodeOutcome::Packet {
                packet_bytes: b"ok",
                consumed: packet.len
            }
        );
    }

    #[test]
    fn decoder_reports_crc_error_and_resumes() {
        let mut decoder = StreamingDecoder::<BUFFER_BYTES>::new();
        let mut first = wire_packet(b"bad");
        let second = wire_packet(b"good");
        let mut sticky = [0; BUFFER_BYTES];
        first.bytes[9] ^= 0xAA;
        sticky[..first.len].copy_from_slice(first.as_bytes());
        sticky[first.len..first.len + second.len].copy_from_slice(second.as_bytes());

        assert_eq!(
            decoder
                .feed(&sticky[..first.len + second.len], &Crc16)
                .unwrap(),
            StreamDecodeOutcome::Corrupted {
                consumed: first.len
            }
        );
        assert_eq!(
            decoder.feed(&[], &Crc16).unwrap(),
            StreamDecodeOutcome::Packet {
                packet_bytes: b"good",
                consumed: second.len
            }
        );
    }

    struct TestWirePacket {
        bytes: [u8; 16],
        len: usize,
    }

    impl TestWirePacket {
        fn as_bytes(&self) -> &[u8] {
            &self.bytes[..self.len]
        }
    }

    fn wire_packet(payload: &[u8]) -> TestWirePacket {
        let header = EnvelopeHeader::new(payload.len() as u16, WireFlags::CHECKSUM_PRESENT);
        let total_len = header.total_len();
        let mut bytes = [0; 16];

        bytes[..2].copy_from_slice(&EnvelopeMagic::SRT.bytes());
        bytes[2] = header.version;
        bytes[3] = header.header_len;
        bytes[4..6].copy_from_slice(&header.packet_len.to_le_bytes());
        bytes[6] = header.flags.bits();
        bytes[7] = header.reserved;
        bytes[8..8 + payload.len()].copy_from_slice(payload);

        let checksum = Crc16.calculate(&bytes[..total_len - 2]);
        bytes[total_len - 2..total_len].copy_from_slice(&checksum.to_le_bytes());

        TestWirePacket {
            bytes,
            len: total_len,
        }
    }
}
