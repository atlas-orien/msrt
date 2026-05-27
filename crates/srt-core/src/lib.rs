#![no_std]
#![doc = "Core protocol primitives for Serial Realtime Transport."]

pub use srt_error::{Error, ErrorKind, Result};

/// A logical stream identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StreamId(pub u16);

/// A packet sequence number.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Seq(pub u32);

/// Protocol flags carried by packets or frames.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Flags(pub u16);

/// Coarse frame categories reserved by the core protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameKind {
    /// Application data frame.
    Data,
    /// Acknowledgement frame.
    Ack,
    /// Runtime or control frame.
    Control,
}

/// Coarse packet categories reserved by the core protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketKind {
    /// User message packet.
    Message,
    /// Transport control packet.
    Control,
}
