//! Library-side protocol benchmarks for the MSRT facade.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use msrt::{
    Engine, EngineConfig,
    core::{ChannelId, Flags, MessageId, PacketHeader, PacketIndex, PacketKey},
    endpoint::{ClientEndpoint, EndpointPoll, PassiveEndpoint},
    engine::{EnginePoll, ReceiveReport},
    integrity::{Aead, Crc8, Crc16, Crc32, Crc64, Integrity, IntegrityConfig},
    reliability::{
        AckTracker, Dedup, FragmentRange, MessageFragment, PacketAckTracker, PacketDedup,
        RetransmitPolicy, RetryLimitPolicy, TimeoutEvent,
    },
    wire::{EnvelopeHeader, StreamDecodeOutcome, StreamingDecoder, WireEnvelope},
};
use std::hint::black_box;

const SMALL_MESSAGE: &[u8] = b"hello msrt";
const MEDIUM_MESSAGE: &[u8] =
    b"msrt benchmark message split into several packets for host-side regression tracking";
const LARGE_MESSAGE: &[u8] = &[0x55; 192];
const TX_BUF_BYTES: usize = 256;
const WIRE_DECODE_BYTES: usize = 512;

fn core_primitives(c: &mut Criterion) {
    c.bench_function("core_packet_header_key", |b| {
        b.iter(|| {
            let header = PacketHeader::data(
                PacketIndex::new(black_box(7)),
                Flags::ACK_ELICITING,
                ChannelId::new(black_box(3)),
                MessageId::new(black_box(99)),
                black_box(128),
                black_box(32),
            );

            black_box((header.key(), header.is_ack_eliciting()))
        });
    });

    c.bench_function("core_fragment_range_check", |b| {
        b.iter(|| {
            let range = FragmentRange::new(black_box(32), black_box(64));
            black_box((range.end(), range.fits_in(black_box(128))))
        });
    });
}

fn integrity_backends(c: &mut Criterion) {
    let mut group = c.benchmark_group("integrity");
    let bytes = [0x5a; 96];

    group.bench_function("crc8_header", |b| {
        b.iter(|| black_box(Crc8.calculate(black_box(&bytes[..2]))));
    });

    group.bench_function("crc16_seal_verify", |b| {
        let integrity = Crc16;
        let mut tag = [0; Crc16::TAG_LEN];

        b.iter(|| {
            integrity.seal(black_box(&bytes), black_box(&mut tag));
            black_box(integrity.verify(black_box(&bytes), black_box(&tag)))
        });
    });

    group.bench_function("crc32_seal_verify", |b| {
        let integrity = Crc32;
        let mut tag = [0; Crc32::TAG_LEN];

        b.iter(|| {
            integrity.seal(black_box(&bytes), black_box(&mut tag));
            black_box(integrity.verify(black_box(&bytes), black_box(&tag)))
        });
    });

    group.bench_function("crc64_seal_verify", |b| {
        let integrity = Crc64;
        let mut tag = [0; Crc64::TAG_LEN];

        b.iter(|| {
            integrity.seal(black_box(&bytes), black_box(&mut tag));
            black_box(integrity.verify(black_box(&bytes), black_box(&tag)))
        });
    });

    group.bench_function("aead_seal_verify", |b| {
        let integrity = Aead::DEFAULT;
        let mut tag = [0; Aead::TAG_LEN];

        b.iter(|| {
            integrity.seal(black_box(&bytes), black_box(&mut tag));
            black_box(integrity.verify(black_box(&bytes), black_box(&tag)))
        });
    });

    group.bench_function("integrity_config_crc16_dispatch", |b| {
        let integrity = IntegrityConfig::crc16();
        let mut tag = [0; Crc16::TAG_LEN];

        b.iter(|| {
            integrity.seal(black_box(&bytes), black_box(&mut tag));
            black_box(integrity.verify(black_box(&bytes), black_box(&tag)))
        });
    });

    group.finish();
}

fn wire_boundaries(c: &mut Criterion) {
    let fixture = Fixture::new(MEDIUM_MESSAGE, 16);
    let first = fixture.writes[0].expect("fixture should contain one packet");

    c.bench_function("wire_envelope_header", |b| {
        b.iter(|| {
            let header = EnvelopeHeader::new(black_box(64));
            black_box((
                header.has_valid_header_crc(),
                header.total_len(Crc16::TAG_LEN),
            ))
        });
    });

    c.bench_function("wire_envelope_view", |b| {
        b.iter(|| {
            let header = EnvelopeHeader::new(black_box(first.len as u8));
            let envelope = WireEnvelope::new(header, black_box(first.as_bytes()));
            black_box((envelope.total_len(Crc16::TAG_LEN), envelope.has_valid_len()))
        });
    });

    c.bench_function("wire_streaming_decode_packet", |b| {
        b.iter(|| {
            let mut decoder = StreamingDecoder::<WIRE_DECODE_BYTES>::new();
            match decoder
                .feed(
                    black_box(first.as_bytes()),
                    black_box(&IntegrityConfig::DEFAULT),
                )
                .expect("decode fixture packet")
            {
                StreamDecodeOutcome::Packet { consumed, .. } => black_box(consumed),
                other => panic!("wire benchmark expected packet, got {other:?}"),
            }
        });
    });

    c.bench_function("wire_streaming_bytewise_decode_packet", |b| {
        b.iter(|| {
            let mut decoder = StreamingDecoder::<WIRE_DECODE_BYTES>::new();
            let mut consumed = 0;

            for byte in first.as_bytes() {
                if let StreamDecodeOutcome::Packet { consumed: len, .. } = decoder
                    .feed(black_box(&[*byte]), black_box(&IntegrityConfig::DEFAULT))
                    .expect("decode bytewise fixture packet")
                {
                    consumed = len;
                }
            }

            black_box(consumed)
        });
    });
}

fn reliability_primitives(c: &mut Criterion) {
    c.bench_function("reliability_dedup_16_observe", |b| {
        b.iter(|| {
            let mut dedup = PacketDedup::<16>::new();

            for index in 0..16 {
                let key = packet_key(index);
                black_box(dedup.observe_packet(black_box(key)).expect("dedup observe"));
            }

            black_box(dedup.is_duplicate(packet_key(15)))
        });
    });

    c.bench_function("reliability_ack_tracker_16", |b| {
        b.iter(|| {
            let mut tracker = PacketAckTracker::<16>::new();

            for index in 0..16 {
                tracker.on_packet_sent(black_box(packet_key(index)));
            }

            black_box(tracker.on_ack(packet_key(8)))
        });
    });

    c.bench_function("reliability_retry_limit_policy", |b| {
        b.iter(|| {
            let mut policy = RetryLimitPolicy::new(black_box(10));
            let event = TimeoutEvent::new(packet_key(3), black_box(250), black_box(3));
            black_box(policy.on_timeout(event))
        });
    });

    c.bench_function("reliability_message_fragment_from_header", |b| {
        b.iter(|| {
            let header = PacketHeader::data(
                PacketIndex::new(black_box(4)),
                Flags::ACK_ELICITING,
                ChannelId::DEFAULT,
                MessageId::new(black_box(10)),
                black_box(128),
                black_box(64),
            );
            black_box(
                MessageFragment::try_from_packet_header(header, black_box(16))
                    .expect("fragment should fit"),
            )
        });
    });
}

fn send_fragmentation(c: &mut Criterion) {
    let mut group = c.benchmark_group("send_fragmentation");

    for (name, message) in [
        ("small", SMALL_MESSAGE),
        ("medium", MEDIUM_MESSAGE),
        ("large", LARGE_MESSAGE),
    ] {
        group.bench_with_input(BenchmarkId::from_parameter(name), message, |b, message| {
            b.iter(|| {
                let mut engine = Engine::new(EngineConfig {
                    fragment_bytes: 16,
                    ..EngineConfig::default()
                });

                engine.send(message).expect("send benchmark message");
                drain_writes(&mut engine, 0)
            });
        });
    }

    group.finish();
}

fn receive_reassembly(c: &mut Criterion) {
    let mut group = c.benchmark_group("receive_reassembly");

    for (name, message) in [
        ("small", SMALL_MESSAGE),
        ("medium", MEDIUM_MESSAGE),
        ("large", LARGE_MESSAGE),
    ] {
        let fixture = Fixture::new(message, 16);

        group.bench_with_input(BenchmarkId::from_parameter(name), &fixture, |b, fixture| {
            b.iter(|| {
                let mut receiver = Engine::new(EngineConfig::default());

                for write in fixture.writes() {
                    receive_ok(&mut receiver, write.as_bytes());
                }

                drain_messages(&mut receiver, 0)
            });
        });
    }

    group.finish();
}

fn lossless_duplex_roundtrip(c: &mut Criterion) {
    c.bench_function("lossless_duplex_roundtrip", |b| {
        b.iter(|| {
            let mut mac = Engine::new(EngineConfig {
                fragment_bytes: 16,
                ..EngineConfig::default()
            });
            let mut mcu = Engine::new(EngineConfig {
                fragment_bytes: 16,
                ..EngineConfig::default()
            });

            mac.send(MEDIUM_MESSAGE).expect("queue mac message");
            mcu.send(LARGE_MESSAGE).expect("queue mcu message");

            let mut mac_messages = 0;
            let mut mcu_messages = 0;

            for _ in 0..64 {
                let progressed = pump_lossless(&mut mac, &mut mcu, &mut mac_messages, 0)
                    | pump_lossless(&mut mcu, &mut mac, &mut mcu_messages, 0);

                if mac_messages == 1 && mcu_messages == 1 {
                    return mac_messages + mcu_messages;
                }

                if !progressed {
                    break;
                }
            }

            panic!("duplex benchmark did not complete");
        });
    });
}

fn retransmit_scan(c: &mut Criterion) {
    c.bench_function("retransmit_scan_16_in_flight", |b| {
        b.iter(|| {
            let mut engine = Engine::new(EngineConfig {
                fragment_bytes: 8,
                ..EngineConfig::default()
            });

            engine
                .send(&[0xaa; 128])
                .expect("queue retransmit benchmark message");
            let initial_writes = drain_writes(&mut engine, 0);

            assert_eq!(initial_writes, 16);

            drain_writes(&mut engine, 1)
        });
    });
}

fn endpoint_handshake(c: &mut Criterion) {
    c.bench_function("endpoint_client_passive_handshake", |b| {
        b.iter(|| {
            let mut client = ClientEndpoint::default();
            let mut passive = PassiveEndpoint::default();
            let mut client_tx = [0; TX_BUF_BYTES];
            let mut passive_tx = [0; TX_BUF_BYTES];

            client.connect(black_box(1)).expect("client connect");
            let EndpointPoll::Transmit {
                bytes: hello_bytes, ..
            } = client.poll(1, &mut client_tx).expect("client poll")
            else {
                panic!("client should transmit hello");
            };

            passive.receive(2, hello_bytes);
            let EndpointPoll::Transmit {
                bytes: ack_bytes, ..
            } = passive.poll(2, &mut passive_tx).expect("passive poll")
            else {
                panic!("passive should transmit ack");
            };

            client.receive(3, ack_bytes);
            black_box((client.peer().state(), passive.peer().state()))
        });
    });
}

#[derive(Clone, Copy, Debug)]
struct BenchWrite {
    bytes: [u8; TX_BUF_BYTES],
    len: usize,
}

impl BenchWrite {
    fn new(bytes: &[u8]) -> Self {
        assert!(
            bytes.len() <= TX_BUF_BYTES,
            "benchmark fixture write exceeded TX_BUF_BYTES"
        );

        let mut stored = [0; TX_BUF_BYTES];
        stored[..bytes.len()].copy_from_slice(bytes);

        Self {
            bytes: stored,
            len: bytes.len(),
        }
    }

    fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

#[derive(Clone, Debug)]
struct Fixture {
    writes: [Option<BenchWrite>; 32],
    len: usize,
}

impl Fixture {
    fn new(message: &[u8], fragment_bytes: usize) -> Self {
        let mut sender = Engine::new(EngineConfig {
            fragment_bytes,
            ..EngineConfig::default()
        });
        let mut fixture = Self {
            writes: [None; 32],
            len: 0,
        };

        sender.send(message).expect("queue fixture message");

        loop {
            match poll_owned(&mut sender, 0) {
                BenchPoll::Transmit(write) => {
                    fixture.writes[fixture.len] = Some(write);
                    fixture.len += 1;
                }
                BenchPoll::Message => {}
                BenchPoll::Idle => break,
            }
        }

        fixture
    }

    fn writes(&self) -> impl Iterator<Item = BenchWrite> + '_ {
        self.writes[..self.len].iter().flatten().copied()
    }
}

#[derive(Clone, Copy, Debug)]
#[allow(clippy::large_enum_variant)]
enum BenchPoll {
    Transmit(BenchWrite),
    Message,
    Idle,
}

fn pump_lossless(
    src: &mut Engine,
    dst: &mut Engine,
    received_messages: &mut usize,
    now_ms: u64,
) -> bool {
    match poll_owned(src, now_ms) {
        BenchPoll::Transmit(write) => {
            receive_ok(dst, write.as_bytes());
            true
        }
        BenchPoll::Message => {
            *received_messages += 1;
            true
        }
        BenchPoll::Idle => false,
    }
}

fn drain_writes(engine: &mut Engine, now_ms: u64) -> usize {
    let mut writes = 0;

    loop {
        match poll_owned(engine, now_ms) {
            BenchPoll::Transmit(_) => writes += 1,
            BenchPoll::Message => {}
            BenchPoll::Idle => break,
        }
    }

    writes
}

fn drain_messages(engine: &mut Engine, now_ms: u64) -> usize {
    let mut messages = 0;

    loop {
        match poll_owned(engine, now_ms) {
            BenchPoll::Transmit(_) => {}
            BenchPoll::Message => messages += 1,
            BenchPoll::Idle => break,
        }
    }

    messages
}

fn poll_owned(engine: &mut Engine, now_ms: u64) -> BenchPoll {
    let mut tx_buf = [0; TX_BUF_BYTES];

    match engine.poll(now_ms, &mut tx_buf).expect("poll engine") {
        EnginePoll::Transmit { bytes, .. } => BenchPoll::Transmit(BenchWrite::new(bytes)),
        EnginePoll::Message(_) => BenchPoll::Message,
        EnginePoll::SendFailed(failed) => {
            panic!("benchmark should not fail sends: {failed:?}");
        }
        EnginePoll::Idle => BenchPoll::Idle,
    }
}

fn receive_ok(engine: &mut Engine, bytes: &[u8]) {
    match engine.receive(bytes) {
        ReceiveReport::Packet { .. }
        | ReceiveReport::Duplicate { .. }
        | ReceiveReport::Ack { .. }
        | ReceiveReport::Ping { .. }
        | ReceiveReport::Pong { .. } => {}
        other => panic!("unexpected receive report in benchmark fixture: {other:?}"),
    }
}

fn packet_key(index: u16) -> PacketKey {
    PacketKey::new(MessageId::new(7), PacketIndex::new(index))
}

criterion_group!(
    benches,
    core_primitives,
    integrity_backends,
    wire_boundaries,
    reliability_primitives,
    send_fragmentation,
    receive_reassembly,
    lossless_duplex_roundtrip,
    retransmit_scan,
    endpoint_handshake
);
criterion_main!(benches);
