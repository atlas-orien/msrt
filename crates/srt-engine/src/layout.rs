//! Internal MVP packet layout constants.

/// Encoded checksum length in bytes.
pub(crate) const CHECKSUM_LEN: usize = core::mem::size_of::<u16>();
/// Encoded packet number length in bytes.
pub(crate) const PACKET_NUMBER_LEN: usize = core::mem::size_of::<u32>();
/// Encoded message identifier length in bytes.
pub(crate) const MESSAGE_ID_LEN: usize = core::mem::size_of::<u32>();
/// Encoded complete message length field size in bytes.
pub(crate) const MESSAGE_LEN_LEN: usize = core::mem::size_of::<u16>();
/// Encoded fragment offset field size in bytes.
pub(crate) const FRAGMENT_OFFSET_LEN: usize = core::mem::size_of::<u16>();
/// Encoded fragment flags field size in bytes.
pub(crate) const FRAGMENT_FLAGS_LEN: usize = core::mem::size_of::<u8>();
/// Encoded fragment metadata length in bytes.
pub(crate) const FRAGMENT_HEADER_LEN: usize =
    MESSAGE_ID_LEN + MESSAGE_LEN_LEN + FRAGMENT_OFFSET_LEN + FRAGMENT_FLAGS_LEN;
/// Encoded packet metadata length before fragment bytes.
pub(crate) const PACKET_META_LEN: usize = PACKET_NUMBER_LEN + FRAGMENT_HEADER_LEN;

/// Fragment is the first fragment of a message.
pub(crate) const FRAGMENT_FIRST: u8 = 1 << 0;
/// Fragment is the last fragment of a message.
pub(crate) const FRAGMENT_LAST: u8 = 1 << 1;
