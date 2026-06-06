//! Public endpoint API poll-path benchmarks.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use msrt::endpoint::{ClientEndpoint, EndpointPoll, EngineConfig, PassiveEndpoint, ReceiveReport};
use std::hint::black_box;

const TX_BUF_BYTES: usize = 256;
const FRAGMENT_BYTES: usize = 48;
const SMALL_MESSAGE: &[u8] = b"hello msrt";
const MEDIUM_MESSAGE: &[u8] =
    b"msrt poll benchmark message split into several packets for host-side api tracking";
const LARGE_MESSAGE: &[u8] = &[0x55; 192];
const MESSAGES: [(&str, &[u8]); 3] = [
    ("small", SMALL_MESSAGE),
    ("medium", MEDIUM_MESSAGE),
    ("large", LARGE_MESSAGE),
];

fn endpoint_poll_idle(c: &mut Criterion) {
    c.bench_function("api_endpoint/poll_idle", |b| {
        b.iter(|| {
            let mut endpoint = ClientEndpoint::default();
            let mut tx_buf = [0; TX_BUF_BYTES];

            black_box(poll_summary_client(&mut endpoint, 0, &mut tx_buf))
        });
    });
}

fn endpoint_send_then_poll_transmit(c: &mut Criterion) {
    let mut group = c.benchmark_group("api_endpoint/send_then_poll_transmit");

    for (name, message) in MESSAGES {
        group.bench_with_input(BenchmarkId::from_parameter(name), message, |b, message| {
            b.iter(|| {
                let mut endpoint = test_client();
                let mut tx_buf = [0; TX_BUF_BYTES];

                endpoint.connect(0).expect("connect client");
                endpoint
                    .send(black_box(message))
                    .expect("queue data message");
                black_box(poll_summary_client(&mut endpoint, 0, &mut tx_buf))
            });
        });
    }

    group.finish();
}

fn endpoint_receive_then_poll_ack(c: &mut Criterion) {
    let mut group = c.benchmark_group("api_endpoint/receive_then_poll_ack");

    for (name, message) in MESSAGES {
        group.bench_with_input(BenchmarkId::from_parameter(name), message, |b, message| {
            b.iter_batched(
                || data_packets_fixture(message),
                |data_packets| {
                    let mut receiver = test_passive();
                    let mut tx_buf = [0; TX_BUF_BYTES];

                    receive_all_packets(&mut receiver, &data_packets);
                    black_box(poll_summary_passive(&mut receiver, 0, &mut tx_buf))
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn endpoint_receive_then_poll_message(c: &mut Criterion) {
    let mut group = c.benchmark_group("api_endpoint/receive_then_poll_message");

    for (name, message) in MESSAGES {
        group.bench_with_input(BenchmarkId::from_parameter(name), message, |b, message| {
            b.iter_batched(
                || data_packets_fixture(message),
                |data_packets| {
                    let mut receiver = test_passive();
                    let mut tx_buf = [0; TX_BUF_BYTES];

                    receive_all_packets(&mut receiver, &data_packets);
                    drain_transmits_passive(&mut receiver, 0);
                    black_box(poll_summary_passive(&mut receiver, 0, &mut tx_buf))
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn endpoint_lossless_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("api_endpoint/lossless_roundtrip");

    for (name, message) in MESSAGES {
        group.bench_with_input(BenchmarkId::from_parameter(name), message, |b, message| {
            b.iter(|| {
                let mut sender = test_client();
                let mut receiver = test_passive();
                let mut delivered = 0;

                sender.connect(0).expect("connect client");
                sender.send(black_box(message)).expect("queue data message");

                loop {
                    let mut progressed = false;

                    for data in drain_transmits_client(&mut sender, 0) {
                        progressed = true;
                        receive_ok(receiver.receive(0, black_box(&data)));
                    }

                    for ack in drain_transmits_passive(&mut receiver, 0) {
                        progressed = true;
                        receive_ok(sender.receive(0, black_box(&ack)));
                    }

                    match poll_message_or_idle_passive(&mut receiver, 0) {
                        Some(len) => {
                            delivered = len;
                            break;
                        }
                        None if !progressed => break,
                        None => {}
                    }
                }

                black_box(delivered)
            });
        });
    }

    group.finish();
}

fn test_client() -> ClientEndpoint {
    ClientEndpoint::new(EngineConfig {
        fragment_bytes: FRAGMENT_BYTES,
        ..EngineConfig::default()
    })
}

fn test_passive() -> PassiveEndpoint {
    PassiveEndpoint::new(EngineConfig {
        fragment_bytes: FRAGMENT_BYTES,
        ..EngineConfig::default()
    })
}

fn data_packets_fixture(message: &[u8]) -> Vec<Vec<u8>> {
    let mut sender = test_client();
    sender.connect(0).expect("connect client");
    sender.send(message).expect("queue data message");
    drain_transmits_client(&mut sender, 0)
}

fn receive_all_packets(receiver: &mut PassiveEndpoint, packets: &[Vec<u8>]) {
    for packet in packets {
        receive_ok(receiver.receive(0, black_box(packet)));
    }
}

fn drain_transmits_client(endpoint: &mut ClientEndpoint, now_ms: u64) -> Vec<Vec<u8>> {
    let mut packets = Vec::new();

    while let Some(packet) = poll_transmit_bytes_client(endpoint, now_ms) {
        packets.push(packet);
    }

    packets
}

fn drain_transmits_passive(endpoint: &mut PassiveEndpoint, now_ms: u64) -> Vec<Vec<u8>> {
    let mut packets = Vec::new();

    while let Some(packet) = poll_transmit_bytes_passive(endpoint, now_ms) {
        packets.push(packet);
    }

    packets
}

fn poll_transmit_bytes_client(endpoint: &mut ClientEndpoint, now_ms: u64) -> Option<Vec<u8>> {
    let mut tx_buf = [0; TX_BUF_BYTES];
    match endpoint.poll(now_ms, &mut tx_buf).unwrap() {
        EndpointPoll::Transmit { bytes, .. } => Some(bytes.to_vec()),
        EndpointPoll::Message(_) | EndpointPoll::SendFailed(_) | EndpointPoll::Idle => None,
    }
}

fn poll_transmit_bytes_passive(endpoint: &mut PassiveEndpoint, now_ms: u64) -> Option<Vec<u8>> {
    let mut tx_buf = [0; TX_BUF_BYTES];
    match endpoint.poll(now_ms, &mut tx_buf).unwrap() {
        EndpointPoll::Transmit { bytes, .. } => Some(bytes.to_vec()),
        EndpointPoll::Message(_) | EndpointPoll::SendFailed(_) | EndpointPoll::Idle => None,
    }
}

fn poll_message_or_idle_passive(endpoint: &mut PassiveEndpoint, now_ms: u64) -> Option<usize> {
    let mut tx_buf = [0; TX_BUF_BYTES];
    match endpoint.poll(now_ms, &mut tx_buf).unwrap() {
        EndpointPoll::Message(message) => Some(message.len),
        EndpointPoll::Transmit { .. } | EndpointPoll::SendFailed(_) | EndpointPoll::Idle => None,
    }
}

fn poll_summary_client(
    endpoint: &mut ClientEndpoint,
    now_ms: u64,
    tx_buf: &mut [u8],
) -> (u8, usize) {
    match endpoint.poll(black_box(now_ms), black_box(tx_buf)).unwrap() {
        EndpointPoll::Transmit { bytes, attempts } => (attempts, bytes.len()),
        EndpointPoll::Message(message) => (10, message.len),
        EndpointPoll::SendFailed(_) => (20, 0),
        EndpointPoll::Idle => (30, 0),
    }
}

fn poll_summary_passive(
    endpoint: &mut PassiveEndpoint,
    now_ms: u64,
    tx_buf: &mut [u8],
) -> (u8, usize) {
    match endpoint.poll(black_box(now_ms), black_box(tx_buf)).unwrap() {
        EndpointPoll::Transmit { bytes, attempts } => (attempts, bytes.len()),
        EndpointPoll::Message(message) => (10, message.len),
        EndpointPoll::SendFailed(_) => (20, 0),
        EndpointPoll::Idle => (30, 0),
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
    endpoint_poll_idle,
    endpoint_send_then_poll_transmit,
    endpoint_receive_then_poll_ack,
    endpoint_receive_then_poll_message,
    endpoint_lossless_roundtrip
);
criterion_main!(benches);
