//! Minimal protocol engine implementation.

use srt_core::{
    Error, ErrorKind, Flags, MessageId, Packet, PacketHeader, PacketNumber, PacketType, Result,
};
use srt_wire::{Checksum, Crc16, EnvelopeHeader, EnvelopeMagic, WIRE_HEADER_LEN, WireFlags};

use crate::{
    EngineConfig, MAX_EVENTS, MAX_MESSAGE_BYTES, MAX_WIRE_BYTES,
    layout::{CHECKSUM_LEN, FRAGMENT_FIRST, FRAGMENT_LAST, PACKET_META_LEN, PACKET_NUMBER_LEN},
};

/// Minimal non-blocking SRT protocol engine.
///
/// The engine owns protocol state. It splits outgoing messages into packet
/// write events, accepts incoming wire bytes, and emits complete messages once
/// reassembly succeeds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Engine {
    next_packet_number: PacketNumber,
    next_message_id: MessageId,
    fragment_bytes: usize,
    events: EventQueue,
    reassembly: ReassemblyBuffer,
}

impl Engine {
    /// Creates an engine.
    #[must_use]
    pub const fn new(config: EngineConfig) -> Self {
        Self {
            next_packet_number: config.initial_packet_number,
            next_message_id: config.initial_message_id,
            fragment_bytes: config.fragment_bytes,
            events: EventQueue::new(),
            reassembly: ReassemblyBuffer::new(),
        }
    }

    /// Queues a complete message for non-blocking protocol transmission.
    ///
    /// The caller submits the complete message once. The engine splits it into
    /// packet-sized write events internally.
    pub fn send(&mut self, message: &[u8]) -> Result<MessageId> {
        let fragment_bytes = self.fragment_bytes.clamp(1, max_fragment_bytes());
        let message_id = self.next_message_id;
        self.send_message_fragments(message_id, message, fragment_bytes)?;
        self.next_message_id = MessageId::new(self.next_message_id.get().wrapping_add(1));

        Ok(message_id)
    }

    /// Feeds already-arrived wire bytes into the engine.
    ///
    /// This method never waits for more bytes. It handles the current input and
    /// queues events if a complete message becomes available.
    pub fn receive(&mut self, bytes: &[u8]) -> ReceiveReport {
        self.receive_bytes(bytes)
    }

    fn receive_bytes(&mut self, bytes: &[u8]) -> ReceiveReport {
        match decode_message_fragment(bytes, &Crc16) {
            FragmentDecode::Fragment(fragment) => {
                let packet_number = fragment.packet_number;
                match self.reassembly.observe(fragment) {
                    Ok(Some(message)) => {
                        if self.events.push(EngineOutput::Message(message)).is_err() {
                            return ReceiveReport::Error(Error::new(ErrorKind::Engine));
                        }
                        ReceiveReport::Packet { packet_number }
                    }
                    Ok(None) => ReceiveReport::Packet { packet_number },
                    Err(error) => ReceiveReport::Error(error),
                }
            }
            FragmentDecode::Noise { skipped } => ReceiveReport::Noise { skipped },
            FragmentDecode::Corrupted => ReceiveReport::Corrupted,
            FragmentDecode::Incomplete { needed } => ReceiveReport::Incomplete { needed },
        }
    }

    /// Polls one queued engine output event.
    pub fn poll_event(&mut self) -> Option<EngineOutput> {
        self.events.pop()
    }

    /// Advances time-driven protocol work.
    ///
    /// The MVP engine keeps this as a boundary for future ACK timeout and
    /// retransmission logic.
    pub fn tick(&mut self, _now_ms: u64) {}

    /// Returns the next packet number that will be assigned.
    #[must_use]
    pub const fn next_packet_number(&self) -> PacketNumber {
        self.next_packet_number
    }

    fn send_message_fragments(
        &mut self,
        message_id: MessageId,
        message: &[u8],
        fragment_bytes: usize,
    ) -> Result<()> {
        if message.len() > MAX_MESSAGE_BYTES {
            return Err(Error::new(ErrorKind::Engine));
        }

        let mut offset = 0;

        while offset < message.len() || (message.is_empty() && offset == 0) {
            let end = core::cmp::min(offset + fragment_bytes, message.len());
            let fragment = &message[offset..end];
            let packet_number = self.next_packet_number;
            let mut wire = [0; MAX_WIRE_BYTES];
            let flags = fragment_flags(offset, end, message.len());
            let encoded = FragmentToEncode {
                packet_number,
                message_id,
                message_len: message.len(),
                fragment_offset: offset,
                flags,
                fragment,
            };
            let written = encode_message_fragment(encoded, &mut wire, &Crc16)?;

            self.events.push(EngineOutput::Write(WriteEvent {
                packet_number,
                bytes: wire,
                len: written,
            }))?;
            self.next_packet_number = self.next_packet_number.next();

            if message.is_empty() {
                break;
            }

            offset = end;
        }

        Ok(())
    }
}

/// Events produced by the minimal engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EngineOutput {
    /// Protocol bytes should be written to the serial link.
    Write(WriteEvent),
    /// A complete application message has been reassembled.
    Message(MessageEvent),
}

/// A non-blocking write request produced by the engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WriteEvent {
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

/// A complete message delivered by the engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MessageEvent {
    /// Message identifier scoped to this engine.
    pub message_id: MessageId,
    /// Fixed storage containing complete message bytes.
    pub bytes: [u8; MAX_MESSAGE_BYTES],
    /// Number of valid message bytes in `bytes`.
    pub len: usize,
}

impl MessageEvent {
    /// Returns the valid message bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8] {
        self.bytes.split_at(self.len).0
    }
}

/// Result of `Engine::receive`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiveReport {
    /// A packet envelope was accepted.
    Packet {
        /// Packet number decoded from the envelope.
        packet_number: PacketNumber,
    },
    /// The input did not contain a valid magic prefix at offset zero.
    Noise {
        /// Number of bytes treated as noise.
        skipped: usize,
    },
    /// The envelope checksum failed.
    Corrupted,
    /// The envelope is incomplete.
    Incomplete {
        /// Number of bytes required if known.
        needed: Option<usize>,
    },
    /// The packet was valid but could not be applied to engine state.
    Error(Error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FragmentDecode<'a> {
    Fragment(DecodedFragment<'a>),
    Noise { skipped: usize },
    Corrupted,
    Incomplete { needed: Option<usize> },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DecodedFragment<'a> {
    packet_number: PacketNumber,
    message_id: MessageId,
    message_len: usize,
    fragment_offset: usize,
    flags: u8,
    bytes: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FragmentToEncode<'a> {
    packet_number: PacketNumber,
    message_id: MessageId,
    message_len: usize,
    fragment_offset: usize,
    flags: u8,
    fragment: &'a [u8],
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

        let event = self.events[self.head].take();
        self.head = (self.head + 1) % MAX_EVENTS;
        self.len -= 1;

        event
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReassemblyBuffer {
    active: bool,
    message_id: MessageId,
    expected_len: usize,
    received_len: usize,
    bytes: [u8; MAX_MESSAGE_BYTES],
}

impl ReassemblyBuffer {
    const fn new() -> Self {
        Self {
            active: false,
            message_id: MessageId::ZERO,
            expected_len: 0,
            received_len: 0,
            bytes: [0; MAX_MESSAGE_BYTES],
        }
    }

    fn observe(&mut self, fragment: DecodedFragment<'_>) -> Result<Option<MessageEvent>> {
        if fragment.message_len > MAX_MESSAGE_BYTES {
            return Err(Error::new(ErrorKind::Engine));
        }

        if !self.active || fragment.flags & FRAGMENT_FIRST != 0 {
            self.active = true;
            self.message_id = fragment.message_id;
            self.expected_len = fragment.message_len;
            self.received_len = 0;
            self.bytes = [0; MAX_MESSAGE_BYTES];
        }

        if self.message_id != fragment.message_id || self.expected_len != fragment.message_len {
            return Err(Error::new(ErrorKind::Engine));
        }

        let end = fragment.fragment_offset + fragment.bytes.len();

        if end > self.expected_len || end > MAX_MESSAGE_BYTES {
            return Err(Error::new(ErrorKind::Engine));
        }

        self.bytes[fragment.fragment_offset..end].copy_from_slice(fragment.bytes);
        self.received_len = core::cmp::max(self.received_len, end);

        if fragment.flags & FRAGMENT_LAST != 0 && self.received_len == self.expected_len {
            let message = MessageEvent {
                message_id: self.message_id,
                bytes: self.bytes,
                len: self.expected_len,
            };
            *self = Self::new();

            Ok(Some(message))
        } else {
            Ok(None)
        }
    }
}

fn encode_message_fragment(
    fragment_to_encode: FragmentToEncode<'_>,
    out: &mut [u8],
    checksum: &impl Checksum,
) -> Result<usize> {
    let header = PacketHeader::new(
        PacketType::Data,
        fragment_to_encode.packet_number,
        Flags::ACK_ELICITING,
    );
    let packet = Packet::new(header, fragment_to_encode.fragment);
    let packet_len = PACKET_META_LEN + packet.payload_len();
    let packet_len = u16::try_from(packet_len).map_err(|_| Error::new(ErrorKind::Engine))?;
    let message_len =
        u16::try_from(fragment_to_encode.message_len).map_err(|_| Error::new(ErrorKind::Engine))?;
    let fragment_offset = u16::try_from(fragment_to_encode.fragment_offset)
        .map_err(|_| Error::new(ErrorKind::Engine))?;
    let envelope_header = EnvelopeHeader::new(packet_len, WireFlags::CHECKSUM_PRESENT);
    let total_len = envelope_header.total_len();

    if out.len() < total_len {
        return Err(Error::buffer_too_small());
    }

    out[..2].copy_from_slice(&EnvelopeMagic::SRT.bytes());
    out[2] = envelope_header.version;
    out[3] = envelope_header.header_len;
    out[4..6].copy_from_slice(&envelope_header.packet_len.to_le_bytes());
    out[6] = envelope_header.flags.bits();
    out[7] = envelope_header.reserved;
    out[8..12].copy_from_slice(&packet.header.packet_number.get().to_le_bytes());
    out[12..16].copy_from_slice(&fragment_to_encode.message_id.get().to_le_bytes());
    out[16..18].copy_from_slice(&message_len.to_le_bytes());
    out[18..20].copy_from_slice(&fragment_offset.to_le_bytes());
    out[20] = fragment_to_encode.flags;
    out[21..21 + packet.payload_len()].copy_from_slice(packet.payload.as_bytes());

    let checksum_value = checksum.calculate(&out[..total_len - CHECKSUM_LEN]);
    out[total_len - CHECKSUM_LEN..total_len].copy_from_slice(&checksum_value.to_le_bytes());

    Ok(total_len)
}

fn decode_message_fragment<'a>(bytes: &'a [u8], checksum: &impl Checksum) -> FragmentDecode<'a> {
    let Some(offset) = find_magic(bytes) else {
        return FragmentDecode::Noise {
            skipped: bytes.len(),
        };
    };

    if offset > 0 {
        return FragmentDecode::Noise { skipped: offset };
    }

    if bytes.len() < WIRE_HEADER_LEN + CHECKSUM_LEN {
        return FragmentDecode::Incomplete {
            needed: Some(WIRE_HEADER_LEN + CHECKSUM_LEN),
        };
    }

    let packet_len = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
    let total_len = WIRE_HEADER_LEN + packet_len + CHECKSUM_LEN;

    if bytes.len() < total_len {
        return FragmentDecode::Incomplete {
            needed: Some(total_len),
        };
    }

    let expected = u16::from_le_bytes([bytes[total_len - 2], bytes[total_len - 1]]);

    if !checksum.verify(&bytes[..total_len - CHECKSUM_LEN], expected) {
        return FragmentDecode::Corrupted;
    }

    match fragment_from_wire(bytes, packet_len) {
        Some(fragment) => FragmentDecode::Fragment(fragment),
        None => FragmentDecode::Incomplete {
            needed: Some(WIRE_HEADER_LEN + PACKET_META_LEN + CHECKSUM_LEN),
        },
    }
}

fn fragment_from_wire(bytes: &[u8], packet_len: usize) -> Option<DecodedFragment<'_>> {
    let packet_number = packet_number_from_wire(bytes)?;
    let message_id = MessageId::new(u32::from_le_bytes(bytes.get(12..16)?.try_into().ok()?));
    let message_len = u16::from_le_bytes(bytes.get(16..18)?.try_into().ok()?) as usize;
    let fragment_offset = u16::from_le_bytes(bytes.get(18..20)?.try_into().ok()?) as usize;
    let flags = *bytes.get(20)?;
    let start = WIRE_HEADER_LEN + PACKET_META_LEN;
    let end = WIRE_HEADER_LEN + packet_len;
    let fragment = bytes.get(start..end)?;

    Some(DecodedFragment {
        packet_number,
        message_id,
        message_len,
        fragment_offset,
        flags,
        bytes: fragment,
    })
}

fn find_magic(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(EnvelopeMagic::SRT.bytes().len())
        .position(|window| window == EnvelopeMagic::SRT.bytes())
}

fn packet_number_from_wire(bytes: &[u8]) -> Option<PacketNumber> {
    let start = WIRE_HEADER_LEN;
    let end = start + PACKET_NUMBER_LEN;
    let raw = bytes.get(start..end)?;
    let raw = u32::from_le_bytes(raw.try_into().ok()?);

    Some(PacketNumber::new(raw))
}

const fn max_fragment_bytes() -> usize {
    MAX_WIRE_BYTES - WIRE_HEADER_LEN - PACKET_META_LEN - CHECKSUM_LEN
}

const fn fragment_flags(offset: usize, end: usize, message_len: usize) -> u8 {
    let mut flags = 0;

    if offset == 0 {
        flags |= FRAGMENT_FIRST;
    }

    if end == message_len {
        flags |= FRAGMENT_LAST;
    }

    flags
}

#[cfg(test)]
mod tests {
    use super::{Engine, EngineConfig, EngineOutput, ReceiveReport};

    #[test]
    fn engine_sends_one_message_as_multiple_write_events() {
        let mut engine = Engine::new(EngineConfig {
            fragment_bytes: 5,
            ..EngineConfig::default()
        });

        let message_id = engine.send(b"hello srt testing").unwrap();
        let mut writes = 0;

        while let Some(event) = engine.poll_event() {
            match event {
                EngineOutput::Write(_) => writes += 1,
                EngineOutput::Message(_) => panic!("sender should not receive its own message"),
            }
        }

        assert_eq!(message_id.get(), 0);
        assert_eq!(writes, 4);
    }

    #[test]
    fn engine_receives_fragments_as_one_message_event() {
        let mut a = Engine::new(EngineConfig {
            fragment_bytes: 5,
            ..EngineConfig::default()
        });
        let mut b = Engine::new(EngineConfig::default());

        a.send(b"hello srt testing").unwrap();

        while let Some(event) = a.poll_event() {
            let EngineOutput::Write(write) = event else {
                continue;
            };

            assert!(matches!(
                b.receive(write.as_bytes()),
                ReceiveReport::Packet { .. }
            ));
        }

        let Some(EngineOutput::Message(message)) = b.poll_event() else {
            panic!("receiver should emit a complete message");
        };

        assert_eq!(message.as_bytes(), b"hello srt testing");
    }
}
