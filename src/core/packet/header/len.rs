//! Packet header length constants.

use crate::core::message::{CHANNEL_ID_LEN, FRAGMENT_OFFSET_LEN, MESSAGE_ID_LEN, MESSAGE_LEN_LEN};

/// Encoded packet type length in bytes.
pub(crate) const PACKET_TYPE_LEN: usize = core::mem::size_of::<u8>();
/// Encoded packet flags length in bytes.
pub(crate) const PACKET_FLAGS_LEN: usize = core::mem::size_of::<u8>();
/// Encoded message-scoped packet index length in bytes.
pub(crate) const PACKET_INDEX_LEN: usize = core::mem::size_of::<u16>();
/// Encoded legacy packet header length in bytes.
pub(crate) const PACKET_HEADER_LEN: usize = PACKET_TYPE_LEN
    + PACKET_FLAGS_LEN
    + CHANNEL_ID_LEN
    + MESSAGE_ID_LEN
    + PACKET_INDEX_LEN
    + MESSAGE_LEN_LEN
    + FRAGMENT_OFFSET_LEN;
/// Target encoded DATA packet header length after channel removal.
pub const DATA_PACKET_HEADER_LEN: usize = PACKET_TYPE_LEN
    + PACKET_FLAGS_LEN
    + MESSAGE_ID_LEN
    + PACKET_INDEX_LEN
    + MESSAGE_LEN_LEN
    + FRAGMENT_OFFSET_LEN;
/// Target encoded LOG packet header length.
pub const LOG_PACKET_HEADER_LEN: usize =
    PACKET_TYPE_LEN + MESSAGE_ID_LEN + PACKET_INDEX_LEN + MESSAGE_LEN_LEN + FRAGMENT_OFFSET_LEN;
/// Target encoded ACK packet header length.
pub const ACK_PACKET_HEADER_LEN: usize = PACKET_TYPE_LEN + MESSAGE_ID_LEN + PACKET_INDEX_LEN;
/// Target encoded PING/PONG packet header length.
pub const LIVENESS_PACKET_HEADER_LEN: usize = PACKET_TYPE_LEN;

#[cfg(test)]
mod tests {
    use super::{
        ACK_PACKET_HEADER_LEN, DATA_PACKET_HEADER_LEN, LIVENESS_PACKET_HEADER_LEN,
        LOG_PACKET_HEADER_LEN,
    };

    #[test]
    fn target_header_lengths_match_wire_plan() {
        assert_eq!(DATA_PACKET_HEADER_LEN, 12);
        assert_eq!(LOG_PACKET_HEADER_LEN, 11);
        assert_eq!(ACK_PACKET_HEADER_LEN, 7);
        assert_eq!(LIVENESS_PACKET_HEADER_LEN, 1);
    }
}
