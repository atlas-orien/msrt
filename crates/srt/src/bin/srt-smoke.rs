//! Smoke test binary for the SRT facade crate.

use srt::{
    core::{Flags, Packet, PacketHeader, PacketNumber, PacketType},
    wire::{EnvelopeHeader, WireEnvelope, WireFlags},
};

fn main() {
    let payload = [0x01, 0x02, 0x03];
    let header = PacketHeader::new(PacketType::Data, PacketNumber::new(1), Flags::ACK_ELICITING);
    let packet = Packet::new(header, &payload);
    let envelope_header =
        EnvelopeHeader::new(packet.payload_len() as u16, WireFlags::CHECKSUM_PRESENT);
    let envelope = WireEnvelope::new(envelope_header, packet.payload.as_bytes(), 0);

    println!(
        "srt smoke ok: packet_number={}, payload_len={}, wire_len={}",
        packet.header.packet_number.get(),
        packet.payload_len(),
        envelope.total_len()
    );
}
