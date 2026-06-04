//! In-flight packet tracking.

use crate::core::{ChannelId, Error, ErrorKind, MessageId, PacketKey, Result};

use crate::engine::config::{MAX_IN_FLIGHT_PACKETS, MAX_WIRE_BYTES};

/// Encoded packet waiting for acknowledgement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InFlightPacket {
    pub(crate) key: PacketKey,
    pub(crate) channel_id: ChannelId,
    pub(crate) message_id: MessageId,
    pub(crate) bytes: [u8; MAX_WIRE_BYTES],
    pub(crate) len: usize,
    pub(crate) attempts: u8,
    pub(crate) last_sent_ms: u64,
    pub(crate) sent: bool,
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
                .map(|current| current.key == packet.key)
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

    pub(crate) fn apply_ack(&mut self, key: PacketKey) {
        for slot in &mut self.packets {
            let Some(packet) = *slot else {
                continue;
            };

            if packet.key == key {
                *slot = None;
                self.len = self.len.saturating_sub(1);
                return;
            }
        }
    }

    pub(crate) fn packets(&self) -> impl Iterator<Item = &InFlightPacket> {
        self.packets.iter().flatten()
    }

    #[cfg_attr(not(feature = "std"), allow(dead_code))]
    pub(crate) const fn len(&self) -> usize {
        self.len
    }

    pub(crate) const fn available(&self) -> usize {
        MAX_IN_FLIGHT_PACKETS - self.len
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

    pub(crate) fn note_sent(&mut self, key: PacketKey, now_ms: u64) {
        for packet in self.packets.iter_mut().flatten() {
            if packet.key == key {
                packet.sent = true;
                packet.last_sent_ms = now_ms;
                return;
            }
        }
    }

    pub(crate) fn note_retransmit_sent(&mut self, key: PacketKey, now_ms: u64) {
        for packet in self.packets.iter_mut().flatten() {
            if packet.key == key {
                packet.sent = true;
                packet.attempts = packet.attempts.saturating_add(1);
                packet.last_sent_ms = now_ms;
                return;
            }
        }
    }
}
