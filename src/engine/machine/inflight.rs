//! In-flight packet tracking.

use crate::core::{AckFrame, ChannelId, Error, ErrorKind, MessageId, PacketNumber, Result};

use crate::engine::config::{MAX_IN_FLIGHT_PACKETS, MAX_WIRE_BYTES};

/// Encoded packet waiting for acknowledgement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InFlightPacket {
    pub(crate) packet_number: PacketNumber,
    pub(crate) channel_id: ChannelId,
    pub(crate) message_id: MessageId,
    pub(crate) bytes: [u8; MAX_WIRE_BYTES],
    pub(crate) len: usize,
    pub(crate) attempts: u8,
    pub(crate) last_sent_ms: u64,
}

/// Fixed-capacity in-flight packet set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InFlightPackets {
    packets: [Option<InFlightPacket>; MAX_IN_FLIGHT_PACKETS],
    len: usize,
}

impl InFlightPackets {
    pub(crate) const fn new() -> Self {
        Self {
            packets: [None; MAX_IN_FLIGHT_PACKETS],
            len: 0,
        }
    }

    pub(crate) fn track(&mut self, packet: InFlightPacket) -> Result<()> {
        for slot in &mut self.packets {
            if slot
                .map(|current| current.packet_number == packet.packet_number)
                .unwrap_or(false)
            {
                *slot = Some(packet);
                return Ok(());
            }
        }

        for slot in &mut self.packets {
            if slot.is_none() {
                *slot = Some(packet);
                self.len += 1;
                return Ok(());
            }
        }

        Err(Error::new(ErrorKind::Engine))
    }

    pub(crate) fn ack_frame(&mut self, frame: AckFrame) {
        for slot in &mut self.packets {
            let Some(packet) = *slot else {
                continue;
            };

            if frame.acknowledges(packet.packet_number) {
                *slot = None;
                self.len = self.len.saturating_sub(1);
            }
        }
    }

    pub(crate) fn packets(&self) -> impl Iterator<Item = &InFlightPacket> {
        self.packets.iter().flatten()
    }

    pub(crate) fn remove_message(&mut self, channel_id: ChannelId, message_id: MessageId) {
        for slot in &mut self.packets {
            if slot
                .map(|packet| packet.channel_id == channel_id && packet.message_id == message_id)
                .unwrap_or(false)
            {
                *slot = None;
                self.len = self.len.saturating_sub(1);
            }
        }
    }

    pub(crate) fn note_retransmit(&mut self, packet_number: PacketNumber, now_ms: u64) {
        for packet in self.packets.iter_mut().flatten() {
            if packet.packet_number == packet_number {
                packet.attempts = packet.attempts.saturating_add(1);
                packet.last_sent_ms = now_ms;
                return;
            }
        }
    }
}
