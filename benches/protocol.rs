//! Host-side protocol benchmarks for the SRT facade.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use srt::{Config, Engine, Event, MAX_WIRE_BYTES, Receive, Write};

const SMALL_MESSAGE: &[u8] = b"hello srt";
const MEDIUM_MESSAGE: &[u8] =
    b"srt benchmark message split into several packets for host-side regression tracking";
const LARGE_MESSAGE: &[u8] = &[0x55; 192];

fn send_fragmentation(c: &mut Criterion) {
    let mut group = c.benchmark_group("send_fragmentation");

    for (name, message) in [
        ("small", SMALL_MESSAGE),
        ("medium", MEDIUM_MESSAGE),
        ("large", LARGE_MESSAGE),
    ] {
        group.bench_with_input(BenchmarkId::from_parameter(name), message, |b, message| {
            b.iter(|| {
                let mut engine = Engine::new(Config {
                    fragment_bytes: 16,
                    ..Config::default()
                });

                engine.send(message).expect("send benchmark message");
                drain_writes(&mut engine)
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
                let mut receiver = Engine::new(Config::default());

                for write in fixture.writes() {
                    receive_ok(&mut receiver, write.as_bytes());
                }

                drain_messages(&mut receiver)
            });
        });
    }

    group.finish();
}

fn lossless_duplex_roundtrip(c: &mut Criterion) {
    c.bench_function("lossless_duplex_roundtrip", |b| {
        b.iter(|| {
            let mut mac = Engine::new(Config {
                fragment_bytes: 16,
                ..Config::default()
            });
            let mut mcu = Engine::new(Config {
                fragment_bytes: 16,
                ..Config::default()
            });

            mac.send(MEDIUM_MESSAGE).expect("queue mac message");
            mcu.send(LARGE_MESSAGE).expect("queue mcu message");

            let mut mac_messages = 0;
            let mut mcu_messages = 0;

            for _ in 0..64 {
                let progressed = pump_lossless(&mut mac, &mut mcu, &mut mac_messages)
                    | pump_lossless(&mut mcu, &mut mac, &mut mcu_messages);

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
            let mut engine = Engine::new(Config {
                fragment_bytes: 8,
                ..Config::default()
            });

            engine
                .send(&[0xaa; 128])
                .expect("queue retransmit benchmark message");
            let initial_writes = drain_writes(&mut engine);

            assert_eq!(initial_writes, 16);

            engine.tick(1);
            drain_writes(&mut engine)
        });
    });
}

#[derive(Clone, Debug)]
struct Fixture {
    writes: [Option<Write>; 32],
    len: usize,
}

impl Fixture {
    fn new(message: &[u8], fragment_bytes: usize) -> Self {
        let mut sender = Engine::new(Config {
            fragment_bytes,
            ..Config::default()
        });
        let mut fixture = Self {
            writes: [None; 32],
            len: 0,
        };

        sender.send(message).expect("queue fixture message");

        while let Some(event) = sender.poll_event() {
            let Event::Write(write) = event else {
                continue;
            };

            fixture.writes[fixture.len] = Some(write);
            fixture.len += 1;
        }

        fixture
    }

    fn writes(&self) -> impl Iterator<Item = Write> + '_ {
        self.writes[..self.len].iter().flatten().copied()
    }
}

fn pump_lossless(src: &mut Engine, dst: &mut Engine, received_messages: &mut usize) -> bool {
    match src.poll_event() {
        Some(Event::Write(write)) => {
            receive_ok(dst, write.as_bytes());
            true
        }
        Some(Event::Message(_)) => {
            *received_messages += 1;
            true
        }
        Some(Event::SendFailed(failed)) => {
            panic!("benchmark should not fail sends: {failed:?}");
        }
        None => false,
    }
}

fn drain_writes(engine: &mut Engine) -> usize {
    let mut writes = 0;

    while let Some(event) = engine.poll_event() {
        match event {
            Event::Write(_) => writes += 1,
            Event::Message(_) => {}
            Event::SendFailed(failed) => {
                panic!("benchmark should not fail sends: {failed:?}");
            }
        }
    }

    writes
}

fn drain_messages(engine: &mut Engine) -> usize {
    let mut messages = 0;

    while let Some(event) = engine.poll_event() {
        match event {
            Event::Write(_) => {}
            Event::Message(_) => messages += 1,
            Event::SendFailed(failed) => {
                panic!("benchmark should not fail sends: {failed:?}");
            }
        }
    }

    messages
}

fn receive_ok(engine: &mut Engine, bytes: &[u8]) {
    assert!(
        bytes.len() <= MAX_WIRE_BYTES,
        "benchmark fixture write exceeded MAX_WIRE_BYTES"
    );

    match engine.receive(bytes) {
        Receive::Packet { .. } | Receive::Duplicate { .. } | Receive::Ack { .. } => {}
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
