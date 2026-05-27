//! Frame codec boundary.

pub mod decoder;
pub mod encoder;
pub mod traits;

pub use decoder::{DecodeStatus, DecoderBuffer};
pub use encoder::{PacketFrameEncoder, encode_packet, encode_packet_header, encode_packet_kind};
pub use traits::{FrameDecoder, FrameEncoder};
