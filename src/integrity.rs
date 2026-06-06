//! Packet integrity backends.

pub mod aead;
pub mod crc;

pub use aead::Aead;
pub use crc::{Crc8, Crc16, Crc32, Crc64};

/// Packet integrity backend selected by engine configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegrityConfig {
    /// CRC-16/XMODEM packet integrity.
    Crc16(Crc16),
    /// CRC-32/ISO-HDLC packet integrity.
    Crc32(Crc32),
    /// CRC-64/ECMA-182 packet integrity.
    Crc64(Crc64),
    /// Lightweight keyed packet integrity.
    Aead(Aead),
}

impl IntegrityConfig {
    /// Default packet integrity backend used by [`crate::engine::EngineConfig`].
    pub const DEFAULT: Self = Self::Crc16(Crc16);

    /// Creates the default CRC-16/XMODEM integrity configuration.
    #[must_use]
    pub const fn crc16() -> Self {
        Self::Crc16(Crc16)
    }

    /// Creates a CRC-32/ISO-HDLC integrity configuration.
    #[must_use]
    pub const fn crc32() -> Self {
        Self::Crc32(Crc32)
    }

    /// Creates a CRC-64/ECMA-182 integrity configuration.
    #[must_use]
    pub const fn crc64() -> Self {
        Self::Crc64(Crc64)
    }

    /// Creates a lightweight keyed integrity configuration.
    #[must_use]
    pub const fn aead() -> Self {
        Self::Aead(Aead::DEFAULT)
    }

    /// Creates a lightweight keyed integrity configuration with a custom key.
    #[must_use]
    pub const fn aead_with_key(key: [u8; Aead::KEY_LEN]) -> Self {
        Self::Aead(Aead::new(key))
    }
}

impl Default for IntegrityConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl Integrity for IntegrityConfig {
    fn tag_len(&self) -> usize {
        match self {
            Self::Crc16(integrity) => integrity.tag_len(),
            Self::Crc32(integrity) => integrity.tag_len(),
            Self::Crc64(integrity) => integrity.tag_len(),
            Self::Aead(integrity) => integrity.tag_len(),
        }
    }

    fn seal(&self, bytes: &[u8], out: &mut [u8]) {
        match self {
            Self::Crc16(integrity) => integrity.seal(bytes, out),
            Self::Crc32(integrity) => integrity.seal(bytes, out),
            Self::Crc64(integrity) => integrity.seal(bytes, out),
            Self::Aead(integrity) => integrity.seal(bytes, out),
        }
    }

    fn verify(&self, bytes: &[u8], tag: &[u8]) -> bool {
        match self {
            Self::Crc16(integrity) => integrity.verify(bytes, tag),
            Self::Crc32(integrity) => integrity.verify(bytes, tag),
            Self::Crc64(integrity) => integrity.verify(bytes, tag),
            Self::Aead(integrity) => integrity.verify(bytes, tag),
        }
    }
}

/// Calculates and verifies integrity tags for wire bytes.
pub trait Integrity {
    /// Encoded tag length in bytes.
    fn tag_len(&self) -> usize;

    /// Writes the integrity tag for `bytes` into `out`.
    fn seal(&self, bytes: &[u8], out: &mut [u8]);

    /// Returns whether `tag` matches `bytes`.
    fn verify(&self, bytes: &[u8], tag: &[u8]) -> bool;
}
