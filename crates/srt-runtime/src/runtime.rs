//! Protocol runtime traits.

use srt_core::Result;

use crate::{
    event::RuntimeEvent,
    link::RawLink,
    receive::{PacketInput, ReceiveAction, ReceiveInput, Receiver},
    send::{SendIntent, Sender},
    time::Instant,
};

/// Drives SRT protocol communication.
pub trait ProtocolRuntime: Sender + Receiver {
    /// Queues a user message for protocol transmission.
    fn send_message(&mut self, intent: SendIntent<'_>) -> Result<()> {
        self.send(intent)
    }

    /// Accepts bytes read from the lower link and advances receive-side protocol state.
    fn receive(&mut self, input: ReceiveInput<'_>) -> Result<()> {
        self.receive_bytes(input)
    }

    /// Accepts a decoded packet and advances receive-side protocol state.
    fn receive_packet_input(&mut self, input: PacketInput<'_>) -> Result<ReceiveAction> {
        self.receive_packet(input)
    }

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
