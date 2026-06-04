//! Internal engine protocol state.

use crate::core::{MessageId, PacketNumber, Result};
use crate::engine::{EngineConfig, EnginePoll, ReceiveReport};

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

pub(crate) use scheduler::{EngineOutput, WriteEvent, WritePriority};

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

        self.scheduler.poll(
            &mut self.ack,
            &mut self.numbers,
            &mut self.recovery,
            now_ms,
            tx_buf,
        )
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

    #[cfg(test)]
    pub(crate) fn poll_event(state: &mut EngineState) -> Option<EngineOutput> {
        state.scheduler.pop()
    }
}
