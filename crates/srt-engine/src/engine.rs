//! Protocol engine traits.

use srt_core::Result;

use crate::{
    event::EngineEvent,
    link::RawLink,
    receive::{FeedProgress, PacketInput, ReceiveAction, ReceiveInput, ReceiveProgress, Receiver},
    send::{SendIntent, Sender},
    time::Instant,
};

/// Drives SRT protocol communication.
pub trait ProtocolEngine: Sender + Receiver {
    /// Queues a user message for protocol transmission.
    fn send_message(&mut self, intent: SendIntent<'_>) -> Result<()> {
        self.send(intent)
    }

    /// Reads available bytes from a link and advances receive-side protocol state.
    fn receive<L>(&mut self, link: &mut L, scratch: &mut [u8]) -> Result<ReceiveProgress>
    where
        L: crate::LinkRead,
    {
        Receiver::receive(self, link, scratch)
    }

    /// Feeds already-read bytes into the internal ingress pipeline.
    fn feed_input(&mut self, input: ReceiveInput<'_>) -> Result<FeedProgress> {
        self.feed(input)
    }

    /// Accepts a decoded packet and advances receive-side protocol state.
    fn receive_packet_input(&mut self, input: PacketInput<'_>) -> Result<ReceiveAction> {
        self.receive_packet(input)
    }

    /// Advances timers, retransmission decisions, acknowledgements, and response generation.
    fn tick(&mut self, now: Instant) -> Result<()>;

    /// Attempts to produce the next protocol event.
    fn poll_event(&mut self) -> Result<Option<EngineEvent>>;
}

/// Connects a protocol engine to a raw link without defining either implementation.
pub trait EngineDriver<R, L>
where
    R: ProtocolEngine,
    L: RawLink,
{
    /// Runs one unit of protocol progress.
    fn drive_once(&mut self, engine: &mut R, link: &mut L, now: Instant) -> Result<()>;
}
