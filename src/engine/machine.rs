//! Internal engine state machinery.

use crate::core::{Error, MessageId, PacketNumber, Result};
use crate::engine::{
    Engine, EnginePoll, MessageEvent, ReceiveReport, SendFailedEvent,
    config::{MAX_IN_FLIGHT_PACKETS, MAX_INGRESS_BYTES, MAX_WIRE_BYTES},
};
use crate::reliability::PacketDedup;
use crate::wire::StreamingDecoder;

use self::{
    ack::AckRanges, inflight::InFlightPackets, queue::EventQueue, reassembly::ReassemblyBuffer,
};

pub(crate) mod ack;
pub(crate) mod inflight;
pub(crate) mod ingress;
pub(crate) mod outgoing;
pub(crate) mod packet;
pub(crate) mod queue;
pub(crate) mod reassembly;
pub(crate) mod retransmit;

/// Internal protocol state owned by [`crate::engine::Engine`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Machine {
    pub(crate) next_packet_number: PacketNumber,
    pub(crate) next_message_id: MessageId,
    pub(crate) now_ms: u64,
    pub(crate) events: EventQueue,
    pub(crate) in_flight: InFlightPackets,
    pub(crate) ack_ranges: AckRanges,
    pub(crate) ingress: StreamingDecoder<MAX_INGRESS_BYTES>,
    pub(crate) dedup: PacketDedup<MAX_IN_FLIGHT_PACKETS>,
    pub(crate) reassembly: ReassemblyBuffer,
}

impl Machine {
    pub(crate) const fn new(
        initial_packet_number: PacketNumber,
        initial_message_id: MessageId,
    ) -> Self {
        Self {
            next_packet_number: initial_packet_number,
            next_message_id: initial_message_id,
            now_ms: 0,
            events: EventQueue::new(),
            in_flight: InFlightPackets::new(),
            ack_ranges: AckRanges::new(),
            ingress: StreamingDecoder::new(),
            dedup: PacketDedup::new(),
            reassembly: ReassemblyBuffer::new(),
        }
    }

    pub(crate) fn poll<'a>(
        engine: &mut Engine,
        now_ms: u64,
        tx_buf: &'a mut [u8],
    ) -> Result<EnginePoll<'a>> {
        Self::tick(engine, now_ms);

        let Some(event) = engine.machine.events.pop() else {
            return Ok(EnginePoll::Idle);
        };

        match event {
            EngineOutput::Write(write) => {
                if tx_buf.len() < write.len {
                    return Err(Error::buffer_too_small());
                }

                tx_buf[..write.len].copy_from_slice(write.as_bytes());
                Ok(EnginePoll::Transmit(&tx_buf[..write.len]))
            }
            EngineOutput::Message(message) => Ok(EnginePoll::Message(message)),
            EngineOutput::SendFailed(failed) => Ok(EnginePoll::SendFailed(failed)),
        }
    }

    pub(crate) fn send_on(
        engine: &mut Engine,
        channel_id: crate::core::ChannelId,
        message: &[u8],
    ) -> Result<MessageId> {
        outgoing::send_on(engine, channel_id, message)
    }

    pub(crate) fn receive(engine: &mut Engine, bytes: &[u8]) -> ReceiveReport {
        ingress::receive(engine, bytes)
    }

    fn tick(engine: &mut Engine, now_ms: u64) {
        retransmit::tick(engine, now_ms);
    }

    #[cfg(test)]
    pub(crate) fn poll_event(engine: &mut Engine) -> Option<EngineOutput> {
        engine.machine.events.pop()
    }
}

/// Events produced by the engine machinery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EngineOutput {
    /// Protocol bytes should be written to the serial link.
    Write(WriteEvent),
    /// A complete application message has been reassembled.
    Message(MessageEvent),
    /// A message could not be sent reliably.
    SendFailed(SendFailedEvent),
}

/// A non-blocking write request produced by the engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WriteEvent {
    /// Packet number assigned to this write.
    pub packet_number: PacketNumber,
    /// Fixed storage containing encoded wire bytes.
    pub bytes: [u8; MAX_WIRE_BYTES],
    /// Number of valid bytes in `bytes`.
    pub len: usize,
}

impl WriteEvent {
    /// Returns the valid encoded wire bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8] {
        self.bytes.split_at(self.len).0
    }
}
