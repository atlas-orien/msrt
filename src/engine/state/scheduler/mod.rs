//! Engine output scheduling state.

use crate::core::{Error, ErrorKind, PacketNumber, PacketType, Result};

use crate::{
    engine::{
        EnginePoll, MessageEvent, SendFailedEvent,
        config::{MAX_EVENTS, MAX_WIRE_BYTES},
        state::{ack::AckState, numbers::NumberState, recovery::RecoveryState},
    },
    wire::WIRE_HEADER_LEN,
};

/// Fixed-capacity output scheduler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SchedulerState {
    events: [Option<EngineOutput>; MAX_EVENTS],
    head: usize,
    len: usize,
}

impl SchedulerState {
    pub(crate) const fn new() -> Self {
        Self {
            events: [None; MAX_EVENTS],
            head: 0,
            len: 0,
        }
    }

    pub(crate) fn push(&mut self, event: EngineOutput) -> Result<()> {
        if self.replace_redundant_write(event) {
            return Ok(());
        }

        if self.len == MAX_EVENTS {
            return Err(Error::new(ErrorKind::Engine));
        }

        let index = (self.head + self.len) % MAX_EVENTS;
        self.events[index] = Some(event);
        self.len += 1;

        Ok(())
    }

    pub(crate) const fn available(&self) -> usize {
        MAX_EVENTS - self.len
    }

    pub(crate) fn pop(&mut self) -> Option<EngineOutput> {
        if self.len == 0 {
            return None;
        }

        let offset = self.highest_priority_offset();
        let index = self.physical_index(offset);
        let event = self.events[index].take();

        let mut current = offset;
        while current + 1 < self.len {
            let to = self.physical_index(current);
            let from = self.physical_index(current + 1);
            self.events[to] = self.events[from].take();
            current += 1;
        }

        let tail = self.physical_index(self.len - 1);
        self.events[tail] = None;
        self.len -= 1;

        event
    }

    pub(crate) fn poll<'a>(
        &mut self,
        ack: &mut AckState,
        numbers: &mut NumberState,
        recovery: &mut RecoveryState,
        now_ms: u64,
        tx_buf: &'a mut [u8],
    ) -> Result<EnginePoll<'a>> {
        if ack.is_pending() {
            return poll_pending_ack(ack, numbers, tx_buf);
        }

        let Some(event) = self.pop() else {
            return Ok(EnginePoll::Idle);
        };

        match event {
            EngineOutput::Write(write) => poll_write(write, recovery, now_ms, tx_buf),
            EngineOutput::Message(message) => Ok(EnginePoll::Message(message)),
            EngineOutput::SendFailed(failed) => Ok(EnginePoll::SendFailed(failed)),
        }
    }

    fn highest_priority_offset(&self) -> usize {
        let mut best_offset = 0;
        let mut best_priority =
            EventPriority::for_event(self.events[self.head].as_ref().expect("queue is not empty"));
        let mut offset = 1;

        while offset < self.len {
            let index = self.physical_index(offset);
            let Some(event) = self.events[index].as_ref() else {
                offset += 1;
                continue;
            };
            let priority = EventPriority::for_event(event);

            if priority < best_priority {
                best_offset = offset;
                best_priority = priority;
            }

            offset += 1;
        }

        best_offset
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
}

fn poll_pending_ack<'a>(
    ack: &mut AckState,
    numbers: &mut NumberState,
    tx_buf: &'a mut [u8],
) -> Result<EnginePoll<'a>> {
    let packet_number = numbers.alloc_packet_number();
    let written = crate::engine::codec::outgoing::encode_ack_packet(
        packet_number,
        ack.build_ack(),
        tx_buf,
        &crate::wire::Crc16,
    )?;

    ack.on_ack_sent();

    Ok(EnginePoll::Transmit {
        bytes: &tx_buf[..written],
        attempts: 0,
    })
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
            recovery.note_retransmit_sent(write.packet_number, now_ms);
        }
        WritePriority::Control | WritePriority::NewData => {
            recovery.note_sent(write.packet_number, now_ms);
        }
    }

    tx_buf[..write.len].copy_from_slice(write.as_bytes());

    Ok(EnginePoll::Transmit {
        bytes: &tx_buf[..write.len],
        attempts: write.attempts,
    })
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum EventPriority {
    Control,
    Retransmit,
    NewWrite,
    Local,
}

fn is_redundant_write(current: WriteEvent, incoming: WriteEvent) -> bool {
    match (
        packet_type(current.as_bytes()),
        packet_type(incoming.as_bytes()),
    ) {
        (Some(PacketType::Ack), Some(PacketType::Ack)) => true,
        _ => current.packet_number == incoming.packet_number,
    }
}

fn packet_type(bytes: &[u8]) -> Option<PacketType> {
    PacketType::from_code(*bytes.get(WIRE_HEADER_LEN)?)
}

impl EventPriority {
    fn for_event(event: &EngineOutput) -> Self {
        match event {
            EngineOutput::Write(write) => match write.priority {
                crate::engine::state::WritePriority::Control => Self::Control,
                crate::engine::state::WritePriority::Retransmit => Self::Retransmit,
                crate::engine::state::WritePriority::NewData => Self::NewWrite,
            },
            EngineOutput::SendFailed(_) => Self::Local,
            EngineOutput::Message(_) => Self::Local,
        }
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
    use crate::core::{PacketNumber, PacketType};
    use crate::engine::state::{
        EngineOutput, WriteEvent, WritePriority, scheduler::SchedulerState,
    };
    use crate::wire::WIRE_HEADER_LEN;

    #[test]
    fn queue_replaces_old_ack_with_new_ack() {
        let mut queue = SchedulerState::new();
        let old_ack = write_event(
            PacketType::Ack,
            PacketNumber::new(1),
            WritePriority::Control,
        );
        let new_ack = write_event(
            PacketType::Ack,
            PacketNumber::new(2),
            WritePriority::Control,
        );

        queue.push(EngineOutput::Write(old_ack)).unwrap();
        queue.push(EngineOutput::Write(new_ack)).unwrap();

        assert_eq!(queue.pop(), Some(EngineOutput::Write(new_ack)));
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn queue_replaces_duplicate_packet_number() {
        let mut queue = SchedulerState::new();
        let first = write_event(
            PacketType::Data,
            PacketNumber::new(7),
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

    fn write_event(
        packet_type: PacketType,
        packet_number: PacketNumber,
        priority: WritePriority,
    ) -> WriteEvent {
        let mut bytes = [0; crate::engine::config::MAX_WIRE_BYTES];
        bytes[WIRE_HEADER_LEN] = packet_type.code();

        WriteEvent {
            packet_number,
            bytes,
            len: WIRE_HEADER_LEN + 1,
            attempts: 0,
            priority,
        }
    }
}
