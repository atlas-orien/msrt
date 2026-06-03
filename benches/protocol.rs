//! Host-side protocol benchmarks for the MSRT facade.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use msrt::{
    Engine, EngineConfig,
    engine::{EnginePoll, ReceiveReport},
};

const SMALL_MESSAGE: &[u8] = b"hello msrt";
const MEDIUM_MESSAGE: &[u8] =
    b"msrt benchmark message split into several packets for host-side regression tracking";
const LARGE_MESSAGE: &[u8] = &[0x55; 192];
const TX_BUF_BYTES: usize = 256;

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
        | ReceiveReport::Ack { .. } => {}
        other => panic!("unexpected receive report in benchmark fixture: {other:?}"),
    }
}

criterion_group!(
    benches,
    send_fragmentation,
    receive_reassembly,
    lossless_duplex_roundtrip,
    retransmit_scan
);
criterion_main!(benches);
