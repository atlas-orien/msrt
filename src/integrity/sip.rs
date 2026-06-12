//! Keyed packet integrity tag backend (SipHash-2-4-128).
//!
//! This backend authenticates wire bytes with a 128-bit keyed tag. It does
//! not encrypt payload bytes and it does not provide confidentiality; its
//! purpose is to reject packets that are not valid MSRT data for the
//! configured key.
//!
//! Why this exists: stress testing showed that CRC-16 accepts a corrupted
//! packet roughly once per ten million noisy packets (the expected 2^-16
//! collision rate). An accepted corrupt packet can never be retransmitted,
//! so reliable delivery silently breaks. A 128-bit keyed tag pushes the
//! false-accept probability to 2^-128, which is effectively never.
//!
//! The tag is the official 128-bit output variant of SipHash-2-4. The
//! library default key is a fixed public constant: it provides corruption
//! rejection and cross-protocol discrimination, not protection against an
//! active attacker. Applications that want sender authentication must supply
//! their own key via [`crate::integrity::IntegrityConfig::sip_tag_with_key`].

use super::Integrity;

const DEFAULT_KEY: [u8; SipTag::KEY_LEN] = [
    0x6d, 0x73, 0x72, 0x74, 0x2d, 0x61, 0x65, 0x61, 0x64, 0x2d, 0x76, 0x31, 0x2d, 0x74, 0x61, 0x67,
];

/// Keyed packet integrity tag backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SipTag {
    key: [u8; Self::KEY_LEN],
}

impl SipTag {
    /// Encoded authentication tag length.
    pub const TAG_LEN: usize = 16;

    /// Key length in bytes.
    pub const KEY_LEN: usize = 16;

    /// Default library-configured keyed integrity backend.
    pub const DEFAULT: Self = Self { key: DEFAULT_KEY };

    /// Creates a keyed integrity backend.
    #[must_use]
    pub const fn new(key: [u8; Self::KEY_LEN]) -> Self {
        Self { key }
    }

    /// Calculates the keyed authentication tag for `bytes`.
    #[must_use]
    pub fn calculate(self, bytes: &[u8]) -> [u8; Self::TAG_LEN] {
        let (k0, k1) = self.keys();
        siphash24_128(bytes, k0, k1)
    }

    fn keys(self) -> (u64, u64) {
        let k0 = u64::from_le_bytes([
            self.key[0],
            self.key[1],
            self.key[2],
            self.key[3],
            self.key[4],
            self.key[5],
            self.key[6],
            self.key[7],
        ]);
        let k1 = u64::from_le_bytes([
            self.key[8],
            self.key[9],
            self.key[10],
            self.key[11],
            self.key[12],
            self.key[13],
            self.key[14],
            self.key[15],
        ]);

        (k0, k1)
    }
}

impl Default for SipTag {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl Integrity for SipTag {
    fn tag_len(&self) -> usize {
        Self::TAG_LEN
    }

    fn seal(&self, bytes: &[u8], out: &mut [u8]) {
        out[..Self::TAG_LEN].copy_from_slice(&self.calculate(bytes));
    }

    fn verify(&self, bytes: &[u8], tag: &[u8]) -> bool {
        let Some(tag) = tag.get(..Self::TAG_LEN) else {
            return false;
        };
        let expected = self.calculate(bytes);
        let mut diff = 0;

        for index in 0..Self::TAG_LEN {
            diff |= expected[index] ^ tag[index];
        }

        diff == 0
    }
}

/// Official SipHash-2-4 with 128-bit output.
fn siphash24_128(bytes: &[u8], k0: u64, k1: u64) -> [u8; 16] {
    let mut state = SipState {
        v0: k0 ^ 0x736f_6d65_7073_6575,
        v1: k1 ^ 0x646f_7261_6e64_6f6d ^ 0xee,
        v2: k0 ^ 0x6c79_6765_6e65_7261,
        v3: k1 ^ 0x7465_6462_7974_6573,
    };

    let mut chunks = bytes.chunks_exact(8);
    for chunk in chunks.by_ref() {
        let word = u64::from_le_bytes([
            chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
        ]);
        state.compress(word);
    }

    state.finish(bytes.len(), chunks.remainder())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SipState {
    v0: u64,
    v1: u64,
    v2: u64,
    v3: u64,
}

impl SipState {
    fn compress(&mut self, word: u64) {
        self.v3 ^= word;
        self.rounds(2);
        self.v0 ^= word;
    }

    fn finish(mut self, len: usize, tail: &[u8]) -> [u8; 16] {
        let mut last = (len as u64) << 56;
        let mut index = 0;

        while index < tail.len() {
            last |= u64::from(tail[index]) << (index * 8);
            index += 1;
        }

        self.compress(last);
        self.v2 ^= 0xee;
        self.rounds(4);
        let left = self.v0 ^ self.v1 ^ self.v2 ^ self.v3;

        self.v1 ^= 0xdd;
        self.rounds(4);
        let right = self.v0 ^ self.v1 ^ self.v2 ^ self.v3;

        let mut tag = [0; 16];
        tag[..8].copy_from_slice(&left.to_le_bytes());
        tag[8..].copy_from_slice(&right.to_le_bytes());

        tag
    }

    fn rounds(&mut self, count: u8) {
        let mut round = 0;

        while round < count {
            self.round();
            round += 1;
        }
    }

    fn round(&mut self) {
        self.v0 = self.v0.wrapping_add(self.v1);
        self.v1 = self.v1.rotate_left(13);
        self.v1 ^= self.v0;
        self.v0 = self.v0.rotate_left(32);

        self.v2 = self.v2.wrapping_add(self.v3);
        self.v3 = self.v3.rotate_left(16);
        self.v3 ^= self.v2;

        self.v0 = self.v0.wrapping_add(self.v3);
        self.v3 = self.v3.rotate_left(21);
        self.v3 ^= self.v0;

        self.v2 = self.v2.wrapping_add(self.v1);
        self.v1 = self.v1.rotate_left(17);
        self.v1 ^= self.v2;
        self.v2 = self.v2.rotate_left(32);
    }
}

#[cfg(test)]
mod tests {
    use super::{Integrity, SipTag, siphash24_128};

    #[test]
    fn sip_tag_verifies_calculated_value() {
        let integrity = SipTag::DEFAULT;
        let bytes = [1, 2, 3];
        let mut tag = [0; SipTag::TAG_LEN];

        integrity.seal(&bytes, &mut tag);

        assert!(integrity.verify(&bytes, &tag));
        tag[0] ^= 1;
        assert!(!integrity.verify(&bytes, &tag));
    }

    #[test]
    fn sip_tag_rejects_different_keys() {
        let left = SipTag::new([1; SipTag::KEY_LEN]);
        let right = SipTag::new([2; SipTag::KEY_LEN]);
        let bytes = b"msrt";
        let mut tag = [0; SipTag::TAG_LEN];

        left.seal(bytes, &mut tag);

        assert!(!right.verify(bytes, &tag));
    }

    #[test]
    fn siphash24_128_matches_reference_vectors() {
        // Reference vectors from the SipHash reference implementation
        // (vectors_sip128) with key 00 01 .. 0f.
        let k0 = 0x0706_0504_0302_0100;
        let k1 = 0x0f0e_0d0c_0b0a_0908;
        let input: [u8; 15] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14];

        assert_eq!(
            siphash24_128(&[], k0, k1),
            [
                0xa3, 0x81, 0x7f, 0x04, 0xba, 0x25, 0xa8, 0xe6, 0x6d, 0xf6, 0x72, 0x14, 0xc7, 0x55,
                0x02, 0x93,
            ]
        );
        assert_eq!(
            siphash24_128(&[0], k0, k1),
            [
                0xda, 0x87, 0xc1, 0xd8, 0x6b, 0x99, 0xaf, 0x44, 0x34, 0x76, 0x59, 0x11, 0x9b, 0x22,
                0xfc, 0x45,
            ]
        );
        assert_eq!(
            siphash24_128(&input, k0, k1),
            [
                0x54, 0x93, 0xe9, 0x99, 0x33, 0xb0, 0xa8, 0x11, 0x7e, 0x08, 0xec, 0x0f, 0x97, 0xcf,
                0xc3, 0xd9,
            ]
        );
    }
}
