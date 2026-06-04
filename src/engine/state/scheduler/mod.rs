//! Engine output scheduling state.

use crate::core::{Error, ErrorKind, PacketKey, Result};

use crate::engine::{
    EnginePoll, SendFailedEvent,
    config::{MAX_EVENTS, MAX_WIRE_BYTES},
    state::{ack::AckState, numbers::NumberState, recovery::RecoveryState},
};

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
        ack: &mut AckState,
        _numbers: &mut NumberState,
        tx_buf: &'a mut [u8],
    ) -> Result<EnginePoll<'a>> {
        poll_pending_ack(ack, _numbers, tx_buf)
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

        poll_event(event, recovery, now_ms, tx_buf)
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

    #[cfg(feature = "std")]
    pub(crate) fn log_snapshot(&self, now_ms: u64, ack_pending: bool) {
        eprintln!(
            "msrt scheduler now={} ack_pending={} control_len={} retransmit_len={} new_data_len={} local_len={}",
            now_ms,
            ack_pending,
            self.control.len(),
            self.retransmit.len(),
            self.new_data.len(),
            self.local.len()
        );
        self.control.log_snapshot(now_ms, "control");
        self.retransmit.log_snapshot(now_ms, "retransmit");
        self.new_data.log_snapshot(now_ms, "new_data");
        self.local.log_snapshot(now_ms, "local");
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EventQueue {
    events: [Option<EngineOutput>; MAX_EVENTS],
    head: usize,
    len: usize,
}

impl EventQueue {
    const fn new() -> Self {
        Self {
            events: [None; MAX_EVENTS],
            head: 0,
            len: 0,
        }
    }

    fn push(&mut self, event: EngineOutput) -> Result<()> {
        if self.len == MAX_EVENTS {
            return Err(Error::new(ErrorKind::Engine));
        }

        let index = (self.head + self.len) % MAX_EVENTS;
        self.events[index] = Some(event);
        self.len += 1;

        Ok(())
    }

    fn pop(&mut self) -> Option<EngineOutput> {
        if self.len == 0 {
            return None;
        }

        let index = self.head;
        let event = self.events[index].take();

        self.head = (self.head + 1) % MAX_EVENTS;
        self.len -= 1;

        event
    }

    const fn physical_index(&self, offset: usize) -> usize {
        (self.head + offset) % MAX_EVENTS
    }

    fn replace_redundant_write(&mut self, event: EngineOutput) -> bool {
        let EngineOutput::Write(write) = event else {
            return false;
        };

        let mut offset = 0;
        while offset < self.len {
            let index = self.physical_index(offset);
            let Some(EngineOutput::Write(current)) = self.events[index] else {
                offset += 1;
                continue;
            };

            if is_redundant_write(current, write) {
                self.events[index] = Some(EngineOutput::Write(write));
                return true;
            }

            offset += 1;
        }

        false
    }

    const fn len(&self) -> usize {
        self.len
    }

    #[cfg(feature = "std")]
    fn log_snapshot(&self, now_ms: u64, name: &str) {
        let mut offset = 0;
        while offset < self.len {
            let index = self.physical_index(offset);
            if let Some(event) = self.events[index].as_ref() {
                log_event(now_ms, name, offset, event);
            }
            offset += 1;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum QueueKind {
    Control,
    Retransmit,
    NewData,
    Local,
}

const fn queue_for_event(event: &EngineOutput) -> QueueKind {
    match event {
        EngineOutput::Write(write) => match write.priority {
            WritePriority::Control => QueueKind::Control,
            WritePriority::Retransmit => QueueKind::Retransmit,
            WritePriority::NewData => QueueKind::NewData,
        },
        EngineOutput::SendFailed(_) => QueueKind::Local,
    }
}

fn poll_pending_ack<'a>(
    ack: &mut AckState,
    _numbers: &mut NumberState,
    tx_buf: &'a mut [u8],
) -> Result<EnginePoll<'a>> {
    let Some(key) = ack.pop() else {
        return Ok(EnginePoll::Idle);
    };

    let written =
        crate::engine::codec::outgoing::encode_ack_packet(key, tx_buf, &crate::wire::Crc16)?;

    Ok(EnginePoll::Transmit {
        bytes: &tx_buf[..written],
        attempts: 0,
    })
}

pub(crate) fn poll_event<'a>(
    event: EngineOutput,
    recovery: &mut RecoveryState,
    now_ms: u64,
    tx_buf: &'a mut [u8],
) -> Result<EnginePoll<'a>> {
    match event {
        EngineOutput::Write(write) => poll_write(write, recovery, now_ms, tx_buf),
        EngineOutput::SendFailed(failed) => Ok(EnginePoll::SendFailed(failed)),
    }
}

fn poll_write<'a>(
    write: WriteEvent,
    recovery: &mut RecoveryState,
    now_ms: u64,
    tx_buf: &'a mut [u8],
) -> Result<EnginePoll<'a>> {
    if tx_buf.len() < write.len {
        return Err(Error::buffer_too_small());
    }

    match write.priority {
        WritePriority::Retransmit => {
            recovery.note_retransmit_sent(write.key, now_ms);
        }
        WritePriority::Control | WritePriority::NewData => {
            recovery.note_sent(write.key, now_ms);
        }
    }

    tx_buf[..write.len].copy_from_slice(write.as_bytes());

    Ok(EnginePoll::Transmit {
        bytes: &tx_buf[..write.len],
        attempts: write.attempts,
    })
}

fn is_redundant_write(current: WriteEvent, incoming: WriteEvent) -> bool {
    current.key == incoming.key
}

#[cfg(feature = "std")]
fn packet_type(bytes: &[u8]) -> Option<crate::core::PacketType> {
    crate::core::PacketType::from_code(*bytes.get(crate::wire::WIRE_HEADER_LEN)?)
}

#[cfg(feature = "std")]
fn log_event(now_ms: u64, queue: &str, offset: usize, event: &EngineOutput) {
    match event {
        EngineOutput::Write(write) => {
            let packet_type = packet_type(write.as_bytes())
                .map(|packet_type| packet_type.code())
                .unwrap_or_default();
            eprintln!(
                "msrt scheduler event now={} queue={} offset={} kind=write packet_type={} ch={} msg={} idx={} attempts={} len={} priority={:?}",
                now_ms,
                queue,
                offset,
                packet_type,
                write.key.channel_id.get(),
                write.key.message_id.get(),
                write.key.packet_index.get(),
                write.attempts,
                write.len,
                write.priority,
            );
        }
        EngineOutput::SendFailed(failed) => {
            eprintln!(
                "msrt scheduler event now={} queue={} offset={} kind=send_failed ch={} msg={}",
                now_ms,
                queue,
                offset,
                failed.channel_id.get(),
                failed.message_id.get(),
            );
        }
    }
}

/// Events produced by engine state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EngineOutput {
    /// Protocol bytes should be written to the serial link.
    Write(WriteEvent),
    /// A message could not be sent reliably.
    SendFailed(SendFailedEvent),
}

/// A non-blocking write request produced by the engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WriteEvent {
    /// Message-scoped packet identity assigned to this write.
    pub key: PacketKey,
    /// Fixed storage containing encoded wire bytes.
    pub bytes: [u8; MAX_WIRE_BYTES],
    /// Number of valid bytes in `bytes`.
    pub len: usize,
    /// Send attempt count: 0 = first send, ≥1 = retransmit.
    pub attempts: u8,
    /// Internal transmit priority used by the scheduler.
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

#[cfg(test)]
mod tests {
    use crate::core::{ChannelId, MessageId, PacketIndex, PacketKey, PacketType};
    use crate::engine::state::{
        EngineOutput, WriteEvent,
        scheduler::{SchedulerState, WritePriority},
    };
    use crate::wire::WIRE_HEADER_LEN;

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
        let key = PacketKey::new(ChannelId::DEFAULT, MessageId::new(1), packet_index);

        WriteEvent {
            key,
            bytes,
            len: WIRE_HEADER_LEN + 1,
            attempts: 0,
            priority,
        }
    }
}
