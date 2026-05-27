#![no_std]
#![doc = "Protocol runtime boundaries for Serial Realtime Transport."]

use srt_core::{Result, Seq, StreamId};
use srt_stream::{Priority, Qos};

/// A monotonic protocol time value supplied by the embedding runtime.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Instant(pub u64);

/// A protocol duration value supplied by the embedding runtime.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Duration(pub u64);

/// Outbound message metadata understood by the SRT protocol runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SendOptions {
    /// Target logical stream.
    pub stream_id: StreamId,
    /// Requested quality-of-service behavior.
    pub qos: Qos,
    /// Relative scheduling priority.
    pub priority: Priority,
}

/// Events emitted by the protocol runtime to its embedding environment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeEvent {
    /// A message payload is available for a stream.
    Message {
        /// Stream that owns the received message.
        stream_id: StreamId,
        /// Sequence number associated with the received message.
        seq: Seq,
    },
    /// A protocol response should be written to the raw link.
    LinkWrite,
    /// A retransmission became due.
    Retransmit {
        /// Sequence number selected for retransmission.
        seq: Seq,
    },
    /// The runtime needs to be ticked again at a later instant.
    WakeAt(Instant),
}

/// Raw byte link used by the protocol runtime.
///
/// Implementations may be backed by UART, USB CDC, TCP, tests, or any other byte stream.
pub trait RawLink {
    /// Attempts to read bytes from the raw link into `buf`.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;

    /// Attempts to write bytes from `buf` to the raw link.
    fn write(&mut self, buf: &[u8]) -> Result<usize>;
}

/// Drives SRT protocol communication over a raw byte link.
pub trait ProtocolRuntime {
    /// Queues a user message for protocol transmission.
    fn send(&mut self, payload: &[u8], options: SendOptions) -> Result<()>;

    /// Accepts bytes read from the raw link and advances receive-side protocol state.
    fn receive(&mut self, bytes: &[u8]) -> Result<()>;

    /// Advances timers, retransmission decisions, acknowledgements, and response generation.
    fn tick(&mut self, now: Instant) -> Result<()>;

    /// Attempts to produce the next protocol event.
    fn poll_event(&mut self) -> Result<Option<RuntimeEvent>>;
}

/// Connects a protocol runtime to a raw link without defining either implementation.
pub trait RuntimeDriver<R, L>
where
    R: ProtocolRuntime,
    L: RawLink,
{
    /// Runs one unit of protocol progress.
    fn drive_once(&mut self, runtime: &mut R, link: &mut L, now: Instant) -> Result<()>;
}
