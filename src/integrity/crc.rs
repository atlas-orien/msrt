//! CRC integrity backends.

use super::Integrity;

macro_rules! impl_crc_integrity {
    ($crc:ty, $tag:ty) => {
        impl $crc {
            /// Encoded integrity tag length.
            pub const TAG_LEN: usize = core::mem::size_of::<$tag>();
        }

        impl Integrity for $crc {
            fn tag_len(&self) -> usize {
                Self::TAG_LEN
            }

            fn seal(&self, bytes: &[u8], out: &mut [u8]) {
                seal_value(self.calculate(bytes), out);
            }

            fn verify(&self, bytes: &[u8], tag: &[u8]) -> bool {
                verify_value(self.calculate(bytes), tag)
            }
        }
    };
}

/// CRC-8/ATM header integrity.
///
/// This CRC is used by the first-stage wire header to validate the packet
/// length before the complete envelope is available.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Crc8;

impl Crc8 {
    /// Calculates CRC-8/ATM for `bytes`.
    #[must_use]
    pub const fn calculate(self, bytes: &[u8]) -> u8 {
        crc_msb(bytes, 8, 0, 0x07) as u8
    }
}

/// CRC-16/XMODEM packet integrity.
///
/// This backend is intended for random-noise detection, not adversarial
/// authentication. Stronger keyed or AEAD backends can implement [`Integrity`]
/// without changing engine or reliability logic.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Crc16;

impl Crc16 {
    /// Calculates CRC-16/XMODEM for `bytes`.
    #[must_use]
    pub fn calculate(self, bytes: &[u8]) -> u16 {
        crc_msb(bytes, 16, 0, 0x1021) as u16
    }
}

/// CRC-32/ISO-HDLC packet integrity.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Crc32;

impl Crc32 {
    /// Calculates CRC-32/ISO-HDLC for `bytes`.
    #[must_use]
    pub fn calculate(self, bytes: &[u8]) -> u32 {
        !crc_lsb(bytes, 32, 0xffff_ffff, 0xedb8_8320) as u32
    }
}

/// CRC-64/ECMA-182 packet integrity.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Crc64;

impl Crc64 {
    /// Calculates CRC-64/ECMA-182 for `bytes`.
    #[must_use]
    pub fn calculate(self, bytes: &[u8]) -> u64 {
        crc_msb(bytes, 64, 0, 0x42f0_e1eb_a9ea_3693)
    }
}

impl_crc_integrity!(Crc16, u16);
impl_crc_integrity!(Crc32, u32);
impl_crc_integrity!(Crc64, u64);

trait CrcTag: Copy + Eq {
    const LEN: usize;

    fn write_le(self, out: &mut [u8]);
    fn read_le(bytes: &[u8]) -> Self;
}

impl CrcTag for u16 {
    const LEN: usize = core::mem::size_of::<Self>();

    fn write_le(self, out: &mut [u8]) {
        out[..Self::LEN].copy_from_slice(&self.to_le_bytes());
    }

    fn read_le(bytes: &[u8]) -> Self {
        Self::from_le_bytes([bytes[0], bytes[1]])
    }
}

impl CrcTag for u32 {
    const LEN: usize = core::mem::size_of::<Self>();

    fn write_le(self, out: &mut [u8]) {
        out[..Self::LEN].copy_from_slice(&self.to_le_bytes());
    }

    fn read_le(bytes: &[u8]) -> Self {
        Self::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }
}

impl CrcTag for u64 {
    const LEN: usize = core::mem::size_of::<Self>();

    fn write_le(self, out: &mut [u8]) {
        out[..Self::LEN].copy_from_slice(&self.to_le_bytes());
    }

    fn read_le(bytes: &[u8]) -> Self {
        Self::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ])
    }
}

fn seal_value<T: CrcTag>(value: T, out: &mut [u8]) {
    value.write_le(out);
}

fn verify_value<T: CrcTag>(value: T, tag: &[u8]) -> bool {
    let Some(tag) = tag.get(..T::LEN) else {
        return false;
    };

    value == T::read_le(tag)
}

const fn crc_msb(bytes: &[u8], width: u8, initial: u64, poly: u64) -> u64 {
    let top_bit = 1_u64 << (width - 1);
    let mask = crc_mask(width);
    let shift = width - 8;
    let mut checksum = initial & mask;
    let mut index = 0;

    while index < bytes.len() {
        checksum ^= (bytes[index] as u64) << shift;
        checksum = crc_bits_msb(checksum, top_bit, poly, mask);
        index += 1;
    }

    checksum & mask
}

const fn crc_lsb(bytes: &[u8], width: u8, initial: u64, poly: u64) -> u64 {
    let mask = crc_mask(width);
    let mut checksum = initial & mask;
    let mut index = 0;

    while index < bytes.len() {
        checksum ^= bytes[index] as u64;
        checksum = crc_bits_lsb(checksum, poly, mask);
        index += 1;
    }

    checksum & mask
}

const fn crc_bits_msb(mut checksum: u64, top_bit: u64, poly: u64, mask: u64) -> u64 {
    let mut bit = 0;
    while bit < 8 {
        if checksum & top_bit != 0 {
            checksum = (checksum << 1) ^ poly;
        } else {
            checksum <<= 1;
        }
        bit += 1;
    }

    checksum & mask
}

const fn crc_bits_lsb(mut checksum: u64, poly: u64, mask: u64) -> u64 {
    let mut bit = 0;
    while bit < 8 {
        if checksum & 1 != 0 {
            checksum = (checksum >> 1) ^ poly;
        } else {
            checksum >>= 1;
        }
        bit += 1;
    }

    checksum & mask
}

const fn crc_mask(width: u8) -> u64 {
    if width == u64::BITS as u8 {
        u64::MAX
    } else {
        (1_u64 << width) - 1
    }
}

#[cfg(test)]
mod tests {
    use super::{Crc8, Crc16, Crc32, Crc64, Integrity};

    #[test]
    fn crc8_matches_atm_check_value() {
        assert_eq!(Crc8.calculate(b"123456789"), 0xf4);
    }

    #[test]
    fn crc16_verifies_calculated_value() {
        let integrity = Crc16;
        let bytes = [1, 2, 3];
        let mut tag = [0; Crc16::TAG_LEN];

        integrity.seal(&bytes, &mut tag);

        assert!(integrity.verify(&bytes, &tag));
        tag[0] ^= 1;
        assert!(!integrity.verify(&bytes, &tag));
    }

    #[test]
    fn crc16_matches_xmodem_check_value() {
        assert_eq!(Crc16.calculate(b"123456789"), 0x31c3);
    }

    #[test]
    fn crc32_verifies_calculated_value() {
        let integrity = Crc32;
        let bytes = [1, 2, 3];
        let mut tag = [0; Crc32::TAG_LEN];

        integrity.seal(&bytes, &mut tag);

        assert!(integrity.verify(&bytes, &tag));
        tag[0] ^= 1;
        assert!(!integrity.verify(&bytes, &tag));
    }

    #[test]
    fn crc32_matches_iso_hdlc_check_value() {
        assert_eq!(Crc32.calculate(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn crc64_verifies_calculated_value() {
        let integrity = Crc64;
        let bytes = [1, 2, 3];
        let mut tag = [0; Crc64::TAG_LEN];

        integrity.seal(&bytes, &mut tag);

        assert!(integrity.verify(&bytes, &tag));
        tag[0] ^= 1;
        assert!(!integrity.verify(&bytes, &tag));
    }

    #[test]
    fn crc64_matches_ecma_182_check_value() {
        assert_eq!(Crc64.calculate(b"123456789"), 0x6c40_df5f_0b49_7347);
    }
}
