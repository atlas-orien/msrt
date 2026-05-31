//! In-flight packet tracking.

use srt_core::{Error, ErrorKind, PacketNumber, Result};

use crate::{MAX_IN_FLIGHT_PACKETS, MAX_WIRE_BYTES};

/// Encoded packet waiting for acknowledgement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InFlightPacket {
    pub(crate) packet_number: PacketNumber,
    pub(crate) bytes: [u8; MAX_WIRE_BYTES],
    pub(crate) len: usize,
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

    pub(crate) fn ack(&mut self, packet_number: PacketNumber) {
        for slot in &mut self.packets {
            if slot
                .map(|packet| packet.packet_number == packet_number)
                .unwrap_or(false)
            {
                *slot = None;
                self.len = self.len.saturating_sub(1);
                return;
            }
        }
    }

    pub(crate) fn packets(&self) -> impl Iterator<Item = &InFlightPacket> {
        self.packets.iter().flatten()
    }
}
