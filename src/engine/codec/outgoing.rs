//! Outgoing message fragmentation and packet encoding.

use crate::core::{
    ChannelId, Error, ErrorKind, Flags, MessageId, PacketIndex, PacketKey, PacketType, Result,
    packet::header::{
        ACK_PACKET_HEADER_LEN, LIVENESS_PACKET_HEADER_LEN, PACKET_HEADER_LEN, PacketHeader,
    },
};
use crate::reliability::ReliabilityMode;
use crate::{
    integrity::Integrity,
    wire::{
        EnvelopeHeader, EnvelopeMagic, WIRE_HEADER_CRC_OFFSET, WIRE_HEADER_LEN, WIRE_MAGIC_LEN,
        WIRE_PACKET_LEN_OFFSET,
    },
};

use crate::engine::{
    EngineConfig,
    config::{MAX_EVENTS, MAX_MESSAGE_BYTES, MAX_WIRE_BYTES},
    state::{EngineOutput, EngineState, WriteEvent, recovery::inflight::InFlightPacket},
};

const ACK_PACKET_LEN: usize = ACK_PACKET_HEADER_LEN;
const LIVENESS_PACKET_LEN: usize = LIVENESS_PACKET_HEADER_LEN;

impl EngineState {
    pub(crate) fn send_on_impl(
        &mut self,
        config: &EngineConfig,
        channel_id: ChannelId,
        message: &[u8],
    ) -> Result<MessageId> {
        let fragment_bytes = config
            .fragment_bytes
            .clamp(1, max_fragment_bytes(config.integrity.tag_len()));
        let message_id = self.numbers.alloc_message_id();
        let mode = config.reliability_mode(channel_id);
        self.ensure_can_queue_message(message.len(), fragment_bytes, mode)?;
        self.send_message_fragments(
            channel_id,
            message_id,
            message,
            fragment_bytes,
            mode,
            &config.integrity,
        )?;

        Ok(message_id)
    }

    fn ensure_can_queue_message(
        &self,
        message_len: usize,
        fragment_bytes: usize,
        mode: ReliabilityMode,
    ) -> Result<()> {
        let fragments = fragment_count(message_len, fragment_bytes);

        if fragments > MAX_EVENTS || self.scheduler.available() < fragments {
            return Err(Error::new(ErrorKind::Engine));
        }

        if matches!(mode, ReliabilityMode::Reliable)
            && self.recovery.available_in_flight() < fragments
        {
            return Err(Error::new(ErrorKind::Engine));
        }

        Ok(())
    }

    pub(crate) fn queue_ack(&mut self, acknowledged: PacketKey) -> Result<()> {
        if self.ack.observe(acknowledged) {
            Ok(())
        } else {
            Err(Error::new(ErrorKind::Engine))
        }
    }

    pub(crate) fn send_ping_impl(&mut self, config: &EngineConfig) -> Result<()> {
        let mut wire = [0; MAX_WIRE_BYTES];
        let written = encode_liveness_packet(PacketType::Ping, &mut wire, &config.integrity)?;
        let key = PacketKey::new(MessageId::ZERO, PacketIndex::ZERO);

        self.scheduler.push(EngineOutput::Write(WriteEvent {
            key,
            bytes: wire,
            len: written,
            attempts: 0,
            priority: crate::engine::state::scheduler::WritePriority::NewData,
        }))?;

        Ok(())
    }

    pub(crate) fn queue_pong(&mut self, config: &EngineConfig) -> Result<()> {
        let mut wire = [0; MAX_WIRE_BYTES];
        let written = encode_liveness_packet(PacketType::Pong, &mut wire, &config.integrity)?;
        let key = PacketKey::new(MessageId::ZERO, PacketIndex::ZERO);

        self.scheduler.push(EngineOutput::Write(WriteEvent {
            key,
            bytes: wire,
            len: written,
            attempts: 0,
            priority: crate::engine::state::scheduler::WritePriority::Control,
        }))?;

        Ok(())
    }

    fn send_message_fragments(
        &mut self,
        channel_id: ChannelId,
        message_id: MessageId,
        message: &[u8],
        fragment_bytes: usize,
        mode: ReliabilityMode,
        integrity: &impl Integrity,
    ) -> Result<()> {
        if message.len() > MAX_MESSAGE_BYTES {
            return Err(Error::new(ErrorKind::Engine));
        }

        let mut offset = 0;
        let mut packet_index = PacketIndex::ZERO;

        while offset < message.len() || (message.is_empty() && offset == 0) {
            let end = core::cmp::min(offset + fragment_bytes, message.len());
            let fragment = &message[offset..end];
            let mut wire = [0; MAX_WIRE_BYTES];
            let key = PacketKey::new(message_id, packet_index);
            let header = PacketHeader::data(
                packet_index,
                if matches!(mode, ReliabilityMode::Reliable) {
                    Flags::ACK_ELICITING
                } else {
                    Flags::EMPTY
                },
                channel_id,
                message_id,
                message.len(),
                offset,
            );
            let written = encode_message_fragment(header, fragment, &mut wire, integrity)?;

            self.scheduler.push(EngineOutput::Write(WriteEvent {
                key,
                bytes: wire,
                len: written,
                attempts: 0,
                priority: crate::engine::state::scheduler::WritePriority::NewData,
            }))?;
            if matches!(mode, ReliabilityMode::Reliable) {
                self.recovery.track(InFlightPacket {
                    key,
                    channel_id,
                    message_id,
                    bytes: wire,
                    len: written,
                    attempts: 0,
                    last_sent_ms: self.clock.now_ms(),
                    sent: false,
                })?;
            }

            if message.is_empty() {
                break;
            }

            offset = end;
            packet_index = packet_index.next();
        }

        Ok(())
    }
}

fn encode_message_fragment(
    header: PacketHeader,
    fragment: &[u8],
    out: &mut [u8],
    integrity: &impl Integrity,
) -> Result<usize> {
    let packet_len = PACKET_HEADER_LEN + fragment.len();
    let packet_len = u8::try_from(packet_len).map_err(|_| Error::new(ErrorKind::Engine))?;
    let message_len =
        u16::try_from(header.message_len()).map_err(|_| Error::new(ErrorKind::Engine))?;
    let fragment_offset =
        u16::try_from(header.fragment_offset()).map_err(|_| Error::new(ErrorKind::Engine))?;
    let envelope_header = EnvelopeHeader::new(packet_len);
    let integrity_tag_len = integrity.tag_len();
    let total_len = envelope_header.total_len(integrity_tag_len);

    if out.len() < total_len {
        return Err(Error::buffer_too_small());
    }

    out[..WIRE_MAGIC_LEN].copy_from_slice(&EnvelopeMagic::MSRT.bytes());
    out[WIRE_PACKET_LEN_OFFSET] = envelope_header.packet_len;
    out[WIRE_HEADER_CRC_OFFSET] = envelope_header.header_crc;
    let packet = &mut out[WIRE_HEADER_LEN..];
    packet[0] = header.packet_type.code();
    packet[1] = header.flags().bits();
    packet[2] = header.channel_id().get();
    packet[3..7].copy_from_slice(&header.message_id().get().to_le_bytes());
    packet[7..9].copy_from_slice(&header.packet_index().get().to_le_bytes());
    packet[9..11].copy_from_slice(&message_len.to_le_bytes());
    packet[11..13].copy_from_slice(&fragment_offset.to_le_bytes());
    packet[PACKET_HEADER_LEN..PACKET_HEADER_LEN + fragment.len()].copy_from_slice(fragment);

    let (authenticated, tag) = out[..total_len].split_at_mut(total_len - integrity_tag_len);
    integrity.seal(authenticated, tag);

    Ok(total_len)
}

pub(crate) fn encode_ack_packet(
    key: PacketKey,
    out: &mut [u8],
    integrity: &impl Integrity,
) -> Result<usize> {
    let packet_len = u8::try_from(ACK_PACKET_LEN).map_err(|_| Error::new(ErrorKind::Engine))?;
    let envelope_header = EnvelopeHeader::new(packet_len);
    let integrity_tag_len = integrity.tag_len();
    let total_len = envelope_header.total_len(integrity_tag_len);

    if out.len() < total_len {
        return Err(Error::buffer_too_small());
    }

    out[..WIRE_MAGIC_LEN].copy_from_slice(&EnvelopeMagic::MSRT.bytes());
    out[WIRE_PACKET_LEN_OFFSET] = envelope_header.packet_len;
    out[WIRE_HEADER_CRC_OFFSET] = envelope_header.header_crc;
    let packet = &mut out[WIRE_HEADER_LEN..];
    packet[0] = crate::core::PacketType::Ack.code();
    packet[1..5].copy_from_slice(&key.message_id.get().to_le_bytes());
    packet[5..7].copy_from_slice(&key.packet_index.get().to_le_bytes());

    let (authenticated, tag) = out[..total_len].split_at_mut(total_len - integrity_tag_len);
    integrity.seal(authenticated, tag);

    Ok(total_len)
}

fn encode_liveness_packet(
    packet_type: PacketType,
    out: &mut [u8],
    integrity: &impl Integrity,
) -> Result<usize> {
    let packet_len =
        u8::try_from(LIVENESS_PACKET_LEN).map_err(|_| Error::new(ErrorKind::Engine))?;
    let envelope_header = EnvelopeHeader::new(packet_len);
    let integrity_tag_len = integrity.tag_len();
    let total_len = envelope_header.total_len(integrity_tag_len);

    if out.len() < total_len {
        return Err(Error::buffer_too_small());
    }

    out[..WIRE_MAGIC_LEN].copy_from_slice(&EnvelopeMagic::MSRT.bytes());
    out[WIRE_PACKET_LEN_OFFSET] = envelope_header.packet_len;
    out[WIRE_HEADER_CRC_OFFSET] = envelope_header.header_crc;
    let packet = &mut out[WIRE_HEADER_LEN..];
    packet[0] = packet_type.code();

    let (authenticated, tag) = out[..total_len].split_at_mut(total_len - integrity_tag_len);
    integrity.seal(authenticated, tag);

    Ok(total_len)
}

const fn max_fragment_bytes(integrity_tag_len: usize) -> usize {
    MAX_WIRE_BYTES - crate::wire::WIRE_HEADER_LEN - PACKET_HEADER_LEN - integrity_tag_len
}

const fn fragment_count(message_len: usize, fragment_bytes: usize) -> usize {
    if message_len == 0 {
        return 1;
    }

    message_len.div_ceil(fragment_bytes)
}
