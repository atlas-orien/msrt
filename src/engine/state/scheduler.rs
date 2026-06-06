//! Engine output scheduling state.

use crate::core::{Error, ErrorKind, Result};

use crate::engine::{
    EngineConfig, EnginePoll,
    config::MAX_EVENTS,
    state::{ack::AckState, numbers::NumberState, recovery::RecoveryState},
};

mod event;
#[cfg(feature = "tracing")]
pub(super) mod log;
mod poll;
mod queue;

pub(crate) use event::{EngineOutput, WriteEvent, WritePriority};
pub(crate) use poll::poll_event;
use queue::{EventQueue, QueueKind, queue_for_event};

/// Fixed-capacity output scheduler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SchedulerState {
    control: EventQueue,
    retransmit: EventQueue,
    new_data: EventQueue,
    local: EventQueue,
}

impl SchedulerState {
    pub(crate) const fn new() -> Self {
        Self {
            control: EventQueue::new(),
            retransmit: EventQueue::new(),
            new_data: EventQueue::new(),
            local: EventQueue::new(),
        }
    }

    pub(crate) fn push(&mut self, event: EngineOutput) -> Result<()> {
        if self.replace_redundant_write(event) {
            return Ok(());
        }

        if self.len() == MAX_EVENTS {
            return Err(Error::new(ErrorKind::Engine));
        }

        match queue_for_event(&event) {
            QueueKind::Control => self.control.push(event),
            QueueKind::Retransmit => self.retransmit.push(event),
            QueueKind::NewData => self.new_data.push(event),
            QueueKind::Local => self.local.push(event),
        }
    }

    pub(crate) const fn available(&self) -> usize {
        MAX_EVENTS - self.len()
    }

    #[cfg(test)]
    pub(crate) fn pop(&mut self) -> Option<EngineOutput> {
        if let Some(event) = self.control.pop() {
            return Some(event);
        }

        if let Some(event) = self.retransmit.pop() {
            return Some(event);
        }

        if let Some(event) = self.local.pop() {
            return Some(event);
        }

        self.new_data.pop()
    }

    pub(crate) fn poll_ack<'a>(
        &mut self,
        config: &EngineConfig,
        ack: &mut AckState,
        numbers: &mut NumberState,
        tx_buf: &'a mut [u8],
    ) -> Result<EnginePoll<'a>> {
        poll::poll_pending_ack(config, ack, numbers, tx_buf)
    }

    pub(crate) fn pop_urgent(&mut self) -> Option<EngineOutput> {
        if let Some(event) = self.control.pop() {
            return Some(event);
        }

        if let Some(event) = self.retransmit.pop() {
            return Some(event);
        }

        self.local.pop()
    }

    pub(crate) fn poll_new_data<'a>(
        &mut self,
        recovery: &mut RecoveryState,
        now_ms: u64,
        tx_buf: &'a mut [u8],
    ) -> Result<EnginePoll<'a>> {
        let Some(event) = self.new_data.pop() else {
            return Ok(EnginePoll::Idle);
        };

        poll::poll_event(event, recovery, now_ms, tx_buf)
    }

    const fn len(&self) -> usize {
        self.control.len() + self.retransmit.len() + self.new_data.len() + self.local.len()
    }

    fn replace_redundant_write(&mut self, event: EngineOutput) -> bool {
        if self.control.replace_redundant_write(event) {
            return true;
        }

        if self.retransmit.replace_redundant_write(event) {
            return true;
        }

        if self.new_data.replace_redundant_write(event) {
            return true;
        }

        self.local.replace_redundant_write(event)
    }

    #[cfg(feature = "tracing")]
    pub(crate) fn log_snapshot(&self, now_ms: u64, ack_pending: bool) {
        tracing::debug!(
            target: "msrt::scheduler",
            now_ms,
            ack_pending,
            control_len = self.control.len(),
            retransmit_len = self.retransmit.len(),
            new_data_len = self.new_data.len(),
            local_len = self.local.len(),
            "msrt scheduler snapshot",
        );
        self.control.log_snapshot(now_ms, "control");
        self.retransmit.log_snapshot(now_ms, "retransmit");
        self.new_data.log_snapshot(now_ms, "new_data");
        self.local.log_snapshot(now_ms, "local");
    }

    #[cfg(not(feature = "tracing"))]
    #[allow(dead_code)]
    pub(crate) fn log_snapshot(&self, _now_ms: u64, _ack_pending: bool) {}
}

#[cfg(test)]
mod tests {
    use crate::core::{MessageId, PacketIndex, PacketKey, PacketType};
    use crate::engine::state::{EngineOutput, WriteEvent};
    use crate::wire::WIRE_HEADER_LEN;

    use super::{SchedulerState, WritePriority};

    #[test]
    fn queue_polls_control_before_data() {
        let mut queue = SchedulerState::new();
        let data = write_event(
            PacketType::Data,
            PacketIndex::new(1),
            WritePriority::NewData,
        );
        let control = write_event(
            PacketType::Pong,
            PacketIndex::new(2),
            WritePriority::Control,
        );

        queue.push(EngineOutput::Write(data)).unwrap();
        queue.push(EngineOutput::Write(control)).unwrap();

        assert_eq!(queue.pop(), Some(EngineOutput::Write(control)));
        assert_eq!(queue.pop(), Some(EngineOutput::Write(data)));
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn queue_replaces_duplicate_packet_key() {
        let mut queue = SchedulerState::new();
        let first = write_event(
            PacketType::Data,
            PacketIndex::new(7),
            WritePriority::NewData,
        );
        let retransmit = WriteEvent {
            attempts: 2,
            priority: WritePriority::Retransmit,
            ..first
        };

        queue.push(EngineOutput::Write(first)).unwrap();
        queue.push(EngineOutput::Write(retransmit)).unwrap();

        assert_eq!(queue.pop(), Some(EngineOutput::Write(retransmit)));
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn queue_polls_retransmit_before_new_data() {
        let mut queue = SchedulerState::new();
        let data = write_event(
            PacketType::Data,
            PacketIndex::new(1),
            WritePriority::NewData,
        );
        let retransmit = write_event(
            PacketType::Data,
            PacketIndex::new(2),
            WritePriority::Retransmit,
        );

        queue.push(EngineOutput::Write(data)).unwrap();
        queue.push(EngineOutput::Write(retransmit)).unwrap();

        assert_eq!(queue.pop(), Some(EngineOutput::Write(retransmit)));
        assert_eq!(queue.pop(), Some(EngineOutput::Write(data)));
        assert_eq!(queue.pop(), None);
    }

    fn write_event(
        packet_type: PacketType,
        packet_index: PacketIndex,
        priority: WritePriority,
    ) -> WriteEvent {
        let mut bytes = [0; crate::engine::config::MAX_WIRE_BYTES];
        bytes[WIRE_HEADER_LEN] = packet_type.code();
        let key = PacketKey::new(MessageId::new(1), packet_index);

        WriteEvent {
            key,
            bytes,
            len: WIRE_HEADER_LEN + 1,
            attempts: 0,
            priority,
        }
    }
}
