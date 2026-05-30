//! Smoke test binary for the SRT facade crate.

extern crate alloc;

use alloc::{
    collections::{BTreeSet, VecDeque},
    vec::Vec,
};

use srt::{
    core::{Flags, Packet, PacketHeader, PacketNumber, PacketType},
    reliability::{DedupDecision, PacketReliabilityEvent},
    wire::{Checksum, Crc16, EnvelopeHeader, EnvelopeMagic, WIRE_HEADER_LEN, WireFlags},
};

const CHECKSUM_LEN: usize = 2;

fn main() {
    let checksum = Crc16;
    let mut endpoint_a = Endpoint::new();
    let mut endpoint_b = Endpoint::new();
    let mut link = NoisyLink::new();

    for raw in 0..5 {
        let packet_number = PacketNumber::new(raw);
        let packet_bytes = encode_packet(packet_number, &[raw as u8, 0xaa], &checksum);
        endpoint_a.on_sent(packet_number);
        link.push(packet_bytes);
    }

    link.drop_packet(PacketNumber::new(2));
    link.duplicate_packet(PacketNumber::new(3));
    link.inject_noise([0xde, 0xad, 0xbe, 0xef]);
    link.corrupt_packet(PacketNumber::new(4));

    let mut acked = Vec::new();
    drain_link(&mut link, &mut endpoint_b, &checksum, &mut acked);
    endpoint_a.apply_acks(&acked);

    let retransmits = endpoint_a.retransmit_candidates();

    for packet_number in retransmits {
        let packet_bytes =
            encode_packet(packet_number, &[packet_number.get() as u8, 0xbb], &checksum);
        endpoint_a.on_retransmit(packet_number);
        link.push(packet_bytes);
    }

    let mut retransmit_acks = Vec::new();
    drain_link(&mut link, &mut endpoint_b, &checksum, &mut retransmit_acks);
    endpoint_a.apply_acks(&retransmit_acks);

    println!(
        "srt smoke ok: sent={}, delivered={}, acked={}, duplicates={}, noise={}, corrupted={}, retransmits={}",
        endpoint_a.sent_count,
        endpoint_b.delivered_count,
        endpoint_a.acked_count(),
        endpoint_b.duplicates,
        endpoint_b.noise_bytes,
        endpoint_b.corrupted,
        endpoint_a.retransmit_count
    );

    assert_eq!(endpoint_b.delivered_count, 5);
    assert_eq!(endpoint_a.acked_count(), 5);
    assert_eq!(endpoint_b.duplicates, 1);
    assert!(endpoint_b.noise_bytes >= 4);
    assert_eq!(endpoint_b.corrupted, 1);
    assert_eq!(endpoint_a.retransmit_count, 2);
}

#[derive(Debug)]
struct Endpoint {
    sent: BTreeSet<u32>,
    acked: BTreeSet<u32>,
    delivered: BTreeSet<u32>,
    sent_count: usize,
    delivered_count: usize,
    duplicates: usize,
    corrupted: usize,
    noise_bytes: usize,
    retransmit_count: usize,
    events: Vec<PacketReliabilityEvent>,
}

impl Endpoint {
    fn new() -> Self {
        Self {
            sent: BTreeSet::new(),
            acked: BTreeSet::new(),
            delivered: BTreeSet::new(),
            sent_count: 0,
            delivered_count: 0,
            duplicates: 0,
            corrupted: 0,
            noise_bytes: 0,
            retransmit_count: 0,
            events: Vec::new(),
        }
    }

    fn on_sent(&mut self, packet_number: PacketNumber) {
        self.sent.insert(packet_number.get());
        self.sent_count += 1;
        self.events
            .push(PacketReliabilityEvent::Sent { packet_number });
    }

    fn on_retransmit(&mut self, packet_number: PacketNumber) {
        self.retransmit_count += 1;
        self.events
            .push(PacketReliabilityEvent::Retransmit { packet_number });
    }

    fn observe_receive(&mut self, packet_number: PacketNumber) -> DedupDecision {
        if self.delivered.contains(&packet_number.get()) {
            self.duplicates += 1;
            self.events
                .push(PacketReliabilityEvent::Duplicate { packet_number });
            DedupDecision::Duplicate
        } else {
            self.delivered.insert(packet_number.get());
            self.delivered_count += 1;
            DedupDecision::Accept
        }
    }

    fn apply_acks(&mut self, acked: &[PacketNumber]) {
        for packet_number in acked {
            self.acked.insert(packet_number.get());
            self.events.push(PacketReliabilityEvent::Acked {
                packet_number: *packet_number,
            });
        }
    }

    fn retransmit_candidates(&self) -> Vec<PacketNumber> {
        self.sent
            .difference(&self.acked)
            .copied()
            .map(PacketNumber::new)
            .collect()
    }

    fn acked_count(&self) -> usize {
        self.acked.len()
    }
}

#[derive(Debug, Default)]
struct NoisyLink {
    packets: VecDeque<Vec<u8>>,
}

impl NoisyLink {
    fn new() -> Self {
        Self::default()
    }

    fn push(&mut self, bytes: Vec<u8>) {
        self.packets.push_back(bytes);
    }

    fn drop_packet(&mut self, packet_number: PacketNumber) {
        self.packets
            .retain(|bytes| packet_number_from_wire(bytes) != Some(packet_number));
    }

    fn duplicate_packet(&mut self, packet_number: PacketNumber) {
        if let Some(bytes) = self
            .packets
            .iter()
            .find(|bytes| packet_number_from_wire(bytes) == Some(packet_number))
            .cloned()
        {
            self.packets.push_back(bytes);
        }
    }

    fn corrupt_packet(&mut self, packet_number: PacketNumber) {
        if let Some(bytes) = self
            .packets
            .iter_mut()
            .find(|bytes| packet_number_from_wire(bytes) == Some(packet_number))
            && let Some(last) = bytes.last_mut()
        {
            *last ^= 0xff;
        }
    }

    fn inject_noise<const N: usize>(&mut self, noise: [u8; N]) {
        self.packets.push_front(noise.to_vec());
    }
}

fn drain_link(
    link: &mut NoisyLink,
    receiver: &mut Endpoint,
    checksum: &impl Checksum,
    acked: &mut Vec<PacketNumber>,
) {
    while let Some(bytes) = link.packets.pop_front() {
        match decode_packet(&bytes, checksum) {
            DecodeResult::Packet(packet_number) => match receiver.observe_receive(packet_number) {
                DedupDecision::Accept | DedupDecision::Duplicate => acked.push(packet_number),
            },
            DecodeResult::Noise(skipped) => {
                receiver.noise_bytes += skipped;
            }
            DecodeResult::Corrupted => {
                receiver.corrupted += 1;
            }
        }
    }
}

enum DecodeResult {
    Packet(PacketNumber),
    Noise(usize),
    Corrupted,
}

fn encode_packet(packet_number: PacketNumber, payload: &[u8], checksum: &impl Checksum) -> Vec<u8> {
    let header = PacketHeader::new(PacketType::Data, packet_number, Flags::ACK_ELICITING);
    let packet = Packet::new(header, payload);
    let envelope_header = EnvelopeHeader::new(
        encoded_packet_len(packet) as u16,
        WireFlags::CHECKSUM_PRESENT,
    );
    let mut bytes = Vec::with_capacity(envelope_header.total_len());

    bytes.extend_from_slice(&EnvelopeMagic::SRT.bytes());
    bytes.push(envelope_header.version);
    bytes.push(envelope_header.header_len);
    bytes.extend_from_slice(&envelope_header.packet_len.to_le_bytes());
    bytes.push(envelope_header.flags.bits());
    bytes.push(envelope_header.reserved);
    bytes.extend_from_slice(&packet.header.packet_number.get().to_le_bytes());
    bytes.extend_from_slice(packet.payload.as_bytes());

    let checksum_value = checksum.calculate(&bytes);
    bytes.extend_from_slice(&checksum_value.to_le_bytes());

    bytes
}

fn decode_packet(bytes: &[u8], checksum: &impl Checksum) -> DecodeResult {
    let Some(offset) = find_magic(bytes) else {
        return DecodeResult::Noise(bytes.len());
    };

    if offset > 0 {
        return DecodeResult::Noise(offset);
    }

    if bytes.len() < WIRE_HEADER_LEN + CHECKSUM_LEN {
        return DecodeResult::Noise(bytes.len());
    }

    let packet_len = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
    let total_len = WIRE_HEADER_LEN + packet_len + CHECKSUM_LEN;

    if bytes.len() < total_len {
        return DecodeResult::Noise(bytes.len());
    }

    let expected = u16::from_le_bytes([bytes[total_len - 2], bytes[total_len - 1]]);

    if !checksum.verify(&bytes[..total_len - CHECKSUM_LEN], expected) {
        return DecodeResult::Corrupted;
    }

    DecodeResult::Packet(packet_number_from_wire(bytes).expect("valid packet has packet number"))
}

fn find_magic(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(EnvelopeMagic::SRT.bytes().len())
        .position(|window| window == EnvelopeMagic::SRT.bytes())
}

fn encoded_packet_len(packet: Packet<'_>) -> usize {
    core::mem::size_of::<u32>() + packet.payload_len()
}

fn packet_number_from_wire(bytes: &[u8]) -> Option<PacketNumber> {
    let start = WIRE_HEADER_LEN;
    let end = start + core::mem::size_of::<u32>();
    let raw = bytes.get(start..end)?;
    let raw = u32::from_le_bytes(raw.try_into().ok()?);

    Some(PacketNumber::new(raw))
}
