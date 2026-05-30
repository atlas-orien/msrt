#![no_std]
#![doc = "Wire envelope boundaries for Serial Realtime Transport."]

pub mod checksum;
pub mod codec;
pub mod envelope;
pub mod resync;

pub use checksum::{Checksum, Crc16};
pub use codec::{DecodeOutcome, Decoder, EncodeTarget, Encoder};
pub use envelope::{EnvelopeHeader, EnvelopeMagic, WIRE_HEADER_LEN, WireEnvelope, WireFlags};
pub use resync::ResyncState;
