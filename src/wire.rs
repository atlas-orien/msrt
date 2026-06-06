#![doc = "Wire envelope boundaries for MSRT."]

pub mod codec;
pub mod envelope;
pub mod resync;

pub use codec::{
    DecodeOutcome, Decoder, EncodeTarget, Encoder, StreamDecodeOutcome, StreamingDecoder,
};
pub use envelope::{
    EnvelopeHeader, EnvelopeMagic, WIRE_HEADER_CRC_OFFSET, WIRE_HEADER_LEN, WIRE_MAGIC_LEN,
    WIRE_PACKET_LEN_OFFSET, WireEnvelope, WireFlags, header_crc,
};
pub use resync::ResyncState;
