//! Public Engine API and poll-path benchmarks.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use msrt::{
    Engine, EngineConfig,
    engine::{EnginePoll, ReceiveReport},
};
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

fn engine_poll_idle(c: &mut Criterion) {
    c.bench_function("api_engine/poll_idle", |b| {
        b.iter(|| {
            let mut engine = Engine::default();
            let mut tx_buf = [0; TX_BUF_BYTES];

            black_box(poll_summary(&mut engine, 0, &mut tx_buf))
        });
    });
}

fn engine_send_then_poll_transmit(c: &mut Criterion) {
    let mut group = c.benchmark_group("api_engine/send_then_poll_transmit");

    for (name, message) in MESSAGES {
        group.bench_with_input(BenchmarkId::from_parameter(name), message, |b, message| {
            b.iter(|| {
                let mut engine = test_engine();
                let mut tx_buf = [0; TX_BUF_BYTES];

                engine.send(black_box(message)).expect("queue data message");
                black_box(poll_summary(&mut engine, 0, &mut tx_buf))
            });
        });
    }

    group.finish();
}

fn engine_receive_then_poll_ack(c: &mut Criterion) {
    let mut group = c.benchmark_group("api_engine/receive_then_poll_ack");

    for (name, message) in MESSAGES {
        group.bench_with_input(BenchmarkId::from_parameter(name), message, |b, message| {
            b.iter_batched(
                || data_packets_fixture(message),
                |data_packets| {
                    let mut receiver = test_engine();
                    let mut tx_buf = [0; TX_BUF_BYTES];

                    receive_all_packets(&mut receiver, &data_packets);
                    black_box(poll_summary(&mut receiver, 0, &mut tx_buf))
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn engine_receive_then_poll_message(c: &mut Criterion) {
    let mut group = c.benchmark_group("api_engine/receive_then_poll_message");

    for (name, message) in MESSAGES {
        group.bench_with_input(BenchmarkId::from_parameter(name), message, |b, message| {
            b.iter_batched(
                || data_packets_fixture(message),
                |data_packets| {
                    let mut receiver = test_engine();
                    let mut tx_buf = [0; TX_BUF_BYTES];

                    receive_all_packets(&mut receiver, &data_packets);
                    drain_transmits(&mut receiver, 0);
                    black_box(poll_summary(&mut receiver, 0, &mut tx_buf))
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn engine_retransmit_workflow(c: &mut Criterion) {
    let mut group = c.benchmark_group("api_engine/retransmit_workflow");

    for (name, message) in MESSAGES {
        group.bench_with_input(BenchmarkId::from_parameter(name), message, |b, message| {
            b.iter_batched(
                || {
                    let mut engine = Engine::new(EngineConfig {
                        fragment_bytes: FRAGMENT_BYTES,
                        retransmit_timeout_ms: 1,
                        ..EngineConfig::default()
                    });
                    engine.send(message).expect("queue data message");
                    let _ = next_transmit_bytes(&mut engine, 0);
                    engine
                },
                |mut engine| {
                    let mut tx_buf = [0; TX_BUF_BYTES];
                    black_box(poll_summary(&mut engine, 1, &mut tx_buf))
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn engine_lossless_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("api_engine/lossless_roundtrip");

    for (name, message) in MESSAGES {
        group.bench_with_input(BenchmarkId::from_parameter(name), message, |b, message| {
            b.iter(|| {
                let mut sender = test_engine();
                let mut receiver = test_engine();
                let mut delivered = 0;

                sender.send(black_box(message)).expect("queue data message");

                loop {
                    let mut progressed = false;

                    for data in drain_transmits(&mut sender, 0) {
                        progressed = true;
                        assert!(matches!(
                            receiver.receive(black_box(&data)),
                            ReceiveReport::Packet { .. }
                        ));
                    }

                    for ack in drain_transmits(&mut receiver, 0) {
                        progressed = true;
                        assert!(matches!(
                            sender.receive(black_box(&ack)),
                            ReceiveReport::Ack { .. }
                        ));
                    }

                    match poll_message_or_idle(&mut receiver, 0) {
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

fn test_engine() -> Engine {
    Engine::new(EngineConfig {
        fragment_bytes: FRAGMENT_BYTES,
        ..EngineConfig::default()
    })
}

fn data_packets_fixture(message: &[u8]) -> Vec<Vec<u8>> {
    let mut sender = test_engine();
    sender.send(message).expect("queue data message");
    drain_transmits(&mut sender, 0)
}

fn receive_all_packets(receiver: &mut Engine, packets: &[Vec<u8>]) {
    for packet in packets {
        assert!(matches!(
            receiver.receive(black_box(packet)),
            ReceiveReport::Packet { .. }
        ));
    }
}

fn drain_transmits(engine: &mut Engine, now_ms: u64) -> Vec<Vec<u8>> {
    let mut packets = Vec::new();

    while let Some(packet) = poll_transmit_bytes(engine, now_ms) {
        packets.push(packet);
    }

    packets
}

fn next_transmit_bytes(engine: &mut Engine, now_ms: u64) -> Vec<u8> {
    poll_transmit_bytes(engine, now_ms).expect("engine should produce transmit bytes")
}

fn poll_transmit_bytes(engine: &mut Engine, now_ms: u64) -> Option<Vec<u8>> {
    let mut tx_buf = [0; TX_BUF_BYTES];
    match engine.poll(now_ms, &mut tx_buf).unwrap() {
        EnginePoll::Transmit { bytes, .. } => Some(bytes.to_vec()),
        EnginePoll::Message(_) | EnginePoll::SendFailed(_) | EnginePoll::Idle => None,
    }
}

fn poll_message_or_idle(engine: &mut Engine, now_ms: u64) -> Option<usize> {
    let mut tx_buf = [0; TX_BUF_BYTES];
    match engine.poll(now_ms, &mut tx_buf).unwrap() {
        EnginePoll::Message(message) => Some(message.len),
        EnginePoll::Transmit { .. } | EnginePoll::SendFailed(_) | EnginePoll::Idle => None,
    }
}

fn poll_summary(engine: &mut Engine, now_ms: u64, tx_buf: &mut [u8]) -> (u8, usize) {
    match engine.poll(black_box(now_ms), black_box(tx_buf)).unwrap() {
        EnginePoll::Transmit { bytes, attempts } => (attempts, bytes.len()),
        EnginePoll::Message(message) => (10, message.len),
        EnginePoll::SendFailed(_) => (20, 0),
        EnginePoll::Idle => (30, 0),
    }
}

criterion_group!(
    benches,
    engine_poll_idle,
    engine_send_then_poll_transmit,
    engine_receive_then_poll_ack,
    engine_receive_then_poll_message,
    engine_retransmit_workflow,
    engine_lossless_roundtrip
);
criterion_main!(benches);
