//! Internal engine protocol state.

use crate::core::{Error, MessageId, PacketNumber, Result};
use crate::engine::{
    EngineConfig, EnginePoll, MessageEvent, ReceiveReport, SendFailedEvent, config::MAX_WIRE_BYTES,
};

use self::{
    ack::AckState, clock::ClockState, ingress::IngressState, numbers::NumberState,
    reassembly::ReassemblyState, receive::ReceiveState, recovery::RecoveryState,
    scheduler::SchedulerState,
};

pub(crate) mod ack;
pub(crate) mod clock;
pub(crate) mod ingress;
pub(crate) mod numbers;
pub(crate) mod reassembly;
pub(crate) mod receive;
pub(crate) mod recovery;
pub(crate) mod scheduler;
#[cfg(test)]
mod tests;

/// Internal protocol state owned by [`crate::engine::Engine`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EngineState {
    pub(crate) clock: ClockState,
    pub(crate) numbers: NumberState,
    pub(crate) scheduler: SchedulerState,
    pub(crate) recovery: RecoveryState,
    pub(crate) ack: AckState,
    pub(crate) ingress: IngressState,
    pub(crate) receive: ReceiveState,
    pub(crate) reassembly: ReassemblyState,
}

impl EngineState {
    pub(crate) const fn new(
        initial_packet_number: PacketNumber,
        initial_message_id: MessageId,
    ) -> Self {
        Self {
            clock: ClockState::new(),
            numbers: NumberState::new(initial_packet_number, initial_message_id),
            scheduler: SchedulerState::new(),
            recovery: RecoveryState::new(),
            ack: AckState::new(),
            ingress: IngressState::new(),
            receive: ReceiveState::new(),
            reassembly: ReassemblyState::new(),
        }
    }

    pub(crate) fn poll<'a>(
        &mut self,
        config: &EngineConfig,
        now_ms: u64,
        tx_buf: &'a mut [u8],
    ) -> Result<EnginePoll<'a>> {
        self.tick_retransmit(config, now_ms);

        if self.ack.is_pending() {
            return self.poll_pending_ack(tx_buf);
        }

        let Some(event) = self.scheduler.pop() else {
            return Ok(EnginePoll::Idle);
        };

        match event {
            EngineOutput::Write(write) => {
                if tx_buf.len() < write.len {
                    return Err(Error::buffer_too_small());
                }

                match write.priority {
                    WritePriority::Retransmit => {
                        self.recovery
                            .note_retransmit_sent(write.packet_number, now_ms);
                    }
                    WritePriority::Control | WritePriority::NewData => {
                        self.recovery.note_sent(write.packet_number, now_ms);
                    }
                }
                tx_buf[..write.len].copy_from_slice(write.as_bytes());
                Ok(EnginePoll::Transmit {
                    bytes: &tx_buf[..write.len],
                    attempts: write.attempts,
                })
            }
            EngineOutput::Message(message) => Ok(EnginePoll::Message(message)),
            EngineOutput::SendFailed(failed) => Ok(EnginePoll::SendFailed(failed)),
        }
    }

    pub(crate) fn send_on(
        &mut self,
        config: &EngineConfig,
        channel_id: crate::core::ChannelId,
        message: &[u8],
    ) -> Result<MessageId> {
        self.send_on_impl(config, channel_id, message)
    }

    pub(crate) fn send_ping(&mut self) -> Result<MessageId> {
        self.send_ping_impl()
    }

    pub(crate) fn receive(&mut self, config: &EngineConfig, bytes: &[u8]) -> ReceiveReport {
        self.receive_ingress(config, bytes)
    }

    fn poll_pending_ack<'a>(&mut self, tx_buf: &'a mut [u8]) -> Result<EnginePoll<'a>> {
        let packet_number = self.numbers.alloc_packet_number();
        let written = crate::engine::codec::outgoing::encode_ack_packet(
            packet_number,
            self.ack.build_ack(),
            tx_buf,
            &crate::wire::Crc16,
        )?;

        self.ack.on_ack_sent();

        Ok(EnginePoll::Transmit {
            bytes: &tx_buf[..written],
            attempts: 0,
        })
    }

    #[cfg(test)]
    pub(crate) fn poll_event(state: &mut EngineState) -> Option<EngineOutput> {
        state.scheduler.pop()
    }
}

/// Events produced by engine state.
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
    /// Send attempt count: 0 = first send, ≥1 = retransmit.
    pub attempts: u8,
    /// Internal transmit priority used by the engine event queue.
    pub priority: WritePriority,
}

impl WriteEvent {
    /// Returns the valid encoded wire bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8] {
        self.bytes.split_at(self.len).0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum WritePriority {
    Control,
    Retransmit,
    NewData,
}
