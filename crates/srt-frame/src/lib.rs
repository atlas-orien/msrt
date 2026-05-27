#![no_std]
#![doc = "Frame encoder and decoder boundaries for Serial Realtime Transport."]

pub mod codec;
pub mod crc;
pub mod frame;

pub use codec::{
    DecodeStatus, DecoderBuffer, FrameDecoder, FrameEncoder, PacketFrameEncoder, encode_packet,
    encode_packet_header, encode_packet_kind,
};
pub use crc::{Crc16, Crc16CcittFalse};
pub use frame::{
    CRC_LEN, DEFAULT_FRAME_CAPACITY, Frame, FrameBuf, FrameHeader, HEADER_LEN, LENGTH_LEN, MAGIC,
    MAGIC_LEN, MIN_FRAME_LEN,
};
