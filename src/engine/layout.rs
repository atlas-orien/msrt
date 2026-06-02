//! Internal v1 draft packet layout constants.

/// Encoded checksum length in bytes.
pub(crate) const CHECKSUM_LEN: usize = core::mem::size_of::<u16>();
/// Encoded packet type length in bytes.
pub(crate) const PACKET_TYPE_LEN: usize = core::mem::size_of::<u8>();
/// Encoded packet flags length in bytes.
pub(crate) const PACKET_FLAGS_LEN: usize = core::mem::size_of::<u8>();
/// Encoded packet number length in bytes.
pub(crate) const PACKET_NUMBER_LEN: usize = core::mem::size_of::<u32>();
/// Encoded packet header length in bytes.
pub(crate) const PACKET_HEADER_LEN: usize = PACKET_TYPE_LEN + PACKET_FLAGS_LEN + PACKET_NUMBER_LEN;
/// Encoded frame type length in bytes.
pub(crate) const FRAME_TYPE_LEN: usize = core::mem::size_of::<u8>();
/// Encoded channel identifier length in bytes.
pub(crate) const CHANNEL_ID_LEN: usize = core::mem::size_of::<u8>();
/// Encoded message identifier length in bytes.
pub(crate) const MESSAGE_ID_LEN: usize = core::mem::size_of::<u32>();
/// Encoded complete message length field size in bytes.
pub(crate) const MESSAGE_LEN_LEN: usize = core::mem::size_of::<u16>();
/// Encoded fragment offset field size in bytes.
pub(crate) const FRAGMENT_OFFSET_LEN: usize = core::mem::size_of::<u16>();
/// Encoded fragment flags field size in bytes.
pub(crate) const FRAGMENT_FLAGS_LEN: usize = core::mem::size_of::<u8>();
/// Encoded MESSAGE frame header length in bytes.
pub(crate) const MESSAGE_FRAME_HEADER_LEN: usize = FRAME_TYPE_LEN
    + CHANNEL_ID_LEN
    + MESSAGE_ID_LEN
    + MESSAGE_LEN_LEN
    + FRAGMENT_OFFSET_LEN
    + FRAGMENT_FLAGS_LEN;
/// Encoded ACK frame length in bytes.
pub(crate) const ACK_FRAME_LEN: usize = FRAME_TYPE_LEN
    + PACKET_NUMBER_LEN
    + core::mem::size_of::<u8>()
    + crate::core::MAX_ACK_RANGES * 2 * PACKET_NUMBER_LEN;
/// Encoded ACK packet length.
pub(crate) const ACK_PACKET_LEN: usize = PACKET_HEADER_LEN + ACK_FRAME_LEN;
