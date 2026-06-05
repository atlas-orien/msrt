//! Lightweight keyed packet integrity backend.
//!
//! This backend authenticates wire bytes with a fixed-size keyed tag. It does
//! not encrypt payload bytes; its purpose is to reject packets that are not
//! valid MSRT data for the configured key.

use super::Integrity;

const DEFAULT_KEY: [u8; Aead::KEY_LEN] = [
    0x6d, 0x73, 0x72, 0x74, 0x2d, 0x61, 0x65, 0x61, 0x64, 0x2d, 0x76, 0x31, 0x2d, 0x74, 0x61, 0x67,
];

/// Lightweight keyed packet integrity backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Aead {
    key: [u8; Self::KEY_LEN],
}

impl Aead {
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
        let left = siphash24(bytes, k0, k1);
        let right = siphash24(
            bytes,
            k0 ^ 0x736f_6d65_7073_6575,
            k1 ^ 0x646f_7261_6e64_6f6d,
        );
        let mut tag = [0; Self::TAG_LEN];

        tag[..8].copy_from_slice(&left.to_le_bytes());
        tag[8..].copy_from_slice(&right.to_le_bytes());

        tag
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

impl Default for Aead {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl Integrity for Aead {
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

fn siphash24(bytes: &[u8], k0: u64, k1: u64) -> u64 {
    let mut state = SipState {
        v0: k0 ^ 0x736f_6d65_7073_6575,
        v1: k1 ^ 0x646f_7261_6e64_6f6d,
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

    fn finish(mut self, len: usize, tail: &[u8]) -> u64 {
        let mut last = (len as u64) << 56;
        let mut index = 0;

        while index < tail.len() {
            last |= u64::from(tail[index]) << (index * 8);
            index += 1;
        }

        self.compress(last);
        self.v2 ^= 0xff;
        self.rounds(4);

        self.v0 ^ self.v1 ^ self.v2 ^ self.v3
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
    use super::{Aead, Integrity, siphash24};

    #[test]
    fn aead_verifies_calculated_value() {
        let integrity = Aead::DEFAULT;
        let bytes = [1, 2, 3];
        let mut tag = [0; Aead::TAG_LEN];

        integrity.seal(&bytes, &mut tag);

        assert!(integrity.verify(&bytes, &tag));
        tag[0] ^= 1;
        assert!(!integrity.verify(&bytes, &tag));
    }

    #[test]
    fn aead_rejects_different_keys() {
        let left = Aead::new([1; Aead::KEY_LEN]);
        let right = Aead::new([2; Aead::KEY_LEN]);
        let bytes = b"msrt";
        let mut tag = [0; Aead::TAG_LEN];

        left.seal(bytes, &mut tag);

        assert!(!right.verify(bytes, &tag));
    }

    #[test]
    fn siphash24_matches_reference_vector() {
        assert_eq!(
            siphash24(&[], 0x0706_0504_0302_0100, 0x0f0e_0d0c_0b0a_0908),
            0x726f_db47_dd0e_0e31
        );
    }
}
