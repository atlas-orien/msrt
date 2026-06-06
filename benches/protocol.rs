//! Library-side protocol benchmarks for the public MSRT endpoint facade.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use msrt::endpoint::{ClientEndpoint, EndpointPoll, EngineConfig, PassiveEndpoint, ReceiveReport};
use std::hint::black_box;

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
                let mut endpoint = client_with_fragment_bytes(16);

                endpoint.connect(0).expect("client connect");
                endpoint.send(message).expect("send benchmark message");
                drain_client_writes(&mut endpoint, 0)
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
                let mut receiver = PassiveEndpoint::default();

                for write in fixture.writes() {
                    receive_ok(receiver.receive(0, write.as_bytes()));
                }

                drain_passive_messages(&mut receiver, 0)
            });
        });
    }

    group.finish();
}

fn lossless_duplex_roundtrip(c: &mut Criterion) {
    c.bench_function("lossless_duplex_roundtrip", |b| {
        b.iter(|| {
            let mut mac = client_with_fragment_bytes(16);
            let mut mcu = passive_with_fragment_bytes(16);

            mac.connect(0).expect("client connect");
            mac.send(MEDIUM_MESSAGE).expect("queue mac message");

            let mut mac_messages = 0;
            let mut mcu_messages = 0;
            let mut mcu_queued = false;

            for _ in 0..64 {
                let progressed = pump_client_to_passive(&mut mac, &mut mcu, &mut mac_messages, 0)
                    | pump_passive_to_client(&mut mcu, &mut mac, &mut mcu_messages, 0);

                if !mcu_queued && mcu.peer().is_connected() {
                    mcu.send(LARGE_MESSAGE).expect("queue mcu message");
                    mcu_queued = true;
                }

                if mac_messages == 1 && mcu_messages == 1 {
                    return mac_messages + mcu_messages;
                }

                if !progressed && mcu_queued {
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
            let mut endpoint = client_with_fragment_bytes(8);

            endpoint.connect(0).expect("client connect");
            endpoint
                .send(&[0xaa; 128])
                .expect("queue retransmit benchmark message");
            let initial_writes = drain_client_writes(&mut endpoint, 0);

            assert_eq!(initial_writes, 17);

            drain_client_writes(&mut endpoint, 1)
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
        let mut sender = client_with_fragment_bytes(fragment_bytes);
        let mut fixture = Self {
            writes: [None; 32],
            len: 0,
        };

        sender.connect(0).expect("client connect");
        sender.send(message).expect("queue fixture message");

        loop {
            match poll_client_owned(&mut sender, 0) {
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

fn client_with_fragment_bytes(fragment_bytes: usize) -> ClientEndpoint {
    ClientEndpoint::new(EngineConfig {
        fragment_bytes,
        ..EngineConfig::default()
    })
}

fn passive_with_fragment_bytes(fragment_bytes: usize) -> PassiveEndpoint {
    PassiveEndpoint::new(EngineConfig {
        fragment_bytes,
        ..EngineConfig::default()
    })
}

fn pump_client_to_passive(
    src: &mut ClientEndpoint,
    dst: &mut PassiveEndpoint,
    received_messages: &mut usize,
    now_ms: u64,
) -> bool {
    match poll_client_owned(src, now_ms) {
        BenchPoll::Transmit(write) => {
            receive_ok(dst.receive(now_ms, write.as_bytes()));
            true
        }
        BenchPoll::Message => {
            *received_messages += 1;
            true
        }
        BenchPoll::Idle => false,
    }
}

fn pump_passive_to_client(
    src: &mut PassiveEndpoint,
    dst: &mut ClientEndpoint,
    received_messages: &mut usize,
    now_ms: u64,
) -> bool {
    match poll_passive_owned(src, now_ms) {
        BenchPoll::Transmit(write) => {
            receive_ok(dst.receive(now_ms, write.as_bytes()));
            true
        }
        BenchPoll::Message => {
            *received_messages += 1;
            true
        }
        BenchPoll::Idle => false,
    }
}

fn drain_client_writes(endpoint: &mut ClientEndpoint, now_ms: u64) -> usize {
    let mut writes = 0;

    loop {
        match poll_client_owned(endpoint, now_ms) {
            BenchPoll::Transmit(_) => writes += 1,
            BenchPoll::Message => {}
            BenchPoll::Idle => break,
        }
    }

    writes
}

fn drain_passive_messages(endpoint: &mut PassiveEndpoint, now_ms: u64) -> usize {
    let mut messages = 0;

    loop {
        match poll_passive_owned(endpoint, now_ms) {
            BenchPoll::Transmit(_) => {}
            BenchPoll::Message => messages += 1,
            BenchPoll::Idle => break,
        }
    }

    messages
}

fn poll_client_owned(endpoint: &mut ClientEndpoint, now_ms: u64) -> BenchPoll {
    let mut tx_buf = [0; TX_BUF_BYTES];

    match endpoint.poll(now_ms, &mut tx_buf).expect("poll client") {
        EndpointPoll::Transmit { bytes, .. } => BenchPoll::Transmit(BenchWrite::new(bytes)),
        EndpointPoll::Message(_) => BenchPoll::Message,
        EndpointPoll::SendFailed(failed) => {
            panic!("benchmark should not fail sends: {failed:?}");
        }
        EndpointPoll::Idle => BenchPoll::Idle,
    }
}

fn poll_passive_owned(endpoint: &mut PassiveEndpoint, now_ms: u64) -> BenchPoll {
    let mut tx_buf = [0; TX_BUF_BYTES];

    match endpoint.poll(now_ms, &mut tx_buf).expect("poll passive") {
        EndpointPoll::Transmit { bytes, .. } => BenchPoll::Transmit(BenchWrite::new(bytes)),
        EndpointPoll::Message(_) => BenchPoll::Message,
        EndpointPoll::SendFailed(failed) => {
            panic!("benchmark should not fail sends: {failed:?}");
        }
        EndpointPoll::Idle => BenchPoll::Idle,
    }
}

fn receive_ok(report: ReceiveReport) {
    match report {
        ReceiveReport::Packet { .. }
        | ReceiveReport::Duplicate { .. }
        | ReceiveReport::Ack { .. }
        | ReceiveReport::Ping
        | ReceiveReport::Pong => {}
        other => panic!("unexpected receive report in benchmark fixture: {other:?}"),
    }
}

criterion_group!(
    benches,
    send_fragmentation,
    receive_reassembly,
    lossless_duplex_roundtrip,
    retransmit_scan,
    endpoint_handshake
);
criterion_main!(benches);
