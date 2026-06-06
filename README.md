# MSRT

MSRT is a portable realtime message transport core for byte-stream links. It is built for MCU, robot, drone, device, and host-side systems that need reliable message delivery over UART, USB CDC, TCP-like byte streams, UDP adapters, or custom hardware transports.

The public crate boundary is intentionally small:

- `msrt::endpoint`: client, passive MCU-style, and fixed-capacity server endpoints.
- `msrt::error`: shared error and result types.

Protocol state machines, packet codecs, reliability tracking, and wire framing are internal implementation details. Runtime adapters own real I/O; MSRT owns protocol state.

## What This Crate Provides

- Message-oriented send and receive.
- Byte-stream wire framing with resynchronization.
- Packet integrity selected by `endpoint::EngineConfig`.
- Reliable `Data` messages with ACK, duplicate handling, retransmit, and `SendFailed`.
- Best-effort internal `Log` packet support.
- Internal `Ping` / `Pong` liveness packets.
- Endpoint helpers for client, passive single-peer, and fixed-capacity server use.
- Optional dynamic RTT/PTO recovery for jittery network links.

This crate does not provide UART drivers, DMA drivers, RTOS tasks, Tokio adapters, C headers, C ABI wrappers, or platform HAL code. Those belong in adapter projects such as `msrt-adapters`.

## Endpoint API

`ClientEndpoint` is for the active side:

```rust
use msrt::endpoint::{ClientEndpoint, EndpointPoll};

fn main() -> msrt::error::Result<()> {
    let mut endpoint = ClientEndpoint::default();
    let mut tx_buf = [0u8; 256];

    endpoint.connect(0)?;
    endpoint.send(b"hello")?;

    match endpoint.poll(0, &mut tx_buf)? {
        EndpointPoll::Transmit { bytes, attempts } => {
            let _ = (bytes, attempts); // write bytes to the adapter
        }
        EndpointPoll::Message(message) => {
            let _payload = message.as_bytes();
        }
        EndpointPoll::SendFailed(failed) => {
            let _ = failed;
        }
        EndpointPoll::Idle => {}
    }

    Ok(())
}
```

Incoming bytes are fed through the endpoint:

```rust
let mut endpoint = msrt::endpoint::PassiveEndpoint::default();
let report = endpoint.receive(0, &[0x00, 0x01, 0x02]);
let _ = report;
```

`receive` never blocks and does not require packet-aligned input. It accepts one byte, half a packet, one full packet, sticky packets, or noise followed by valid packets.

Endpoint choices:

- `ClientEndpoint`: actively creates one peer session.
- `PassiveEndpoint`: lazily accepts one peer session when bytes arrive.
- `ServerEndpoint<P, N>`: maps fixed-capacity peer ids to endpoint sessions.

## Packet Model

MSRT is message-oriented. A message is split into packet fragments. A packet is identified by:

```text
message_id + packet_index
```

There is no global packet stream, no ACK range, and no protocol-level channel field.

Current packet kinds:

- `Data`: reliable application message fragment.
- `Log`: best-effort message fragment.
- `Ack`: confirms one `Data` packet key.
- `Ping`: internal liveness probe.
- `Pong`: internal liveness response.

Application routing belongs inside the payload format, not in the MSRT packet header.

## Integrity

Packet integrity is selected when an endpoint is created. No config file is required.

Default:

```rust
use msrt::endpoint::{ClientEndpoint, EngineConfig};

let endpoint = ClientEndpoint::new(EngineConfig::default());
let _ = endpoint;
```

`EngineConfig::default()` uses CRC-16/XMODEM. Other built-in choices:

```rust
use msrt::endpoint::{ClientEndpoint, EngineConfig, IntegrityConfig};

let endpoint = ClientEndpoint::new(EngineConfig {
    integrity: IntegrityConfig::crc32(),
    ..EngineConfig::default()
});
let _ = endpoint;
```

Available integrity backends:

- `IntegrityConfig::crc16()`
- `IntegrityConfig::crc32()`
- `IntegrityConfig::crc64()`
- `IntegrityConfig::aead()`
- `IntegrityConfig::aead_with_key(key)`

Both peers must use the same integrity configuration.

## Recovery

Default recovery is fixed RTO plus fixed retry limit. This is simple and predictable for MCU, UART, USB CDC, and short stable links.

The optional `dynamic-recovery` feature enables a lightweight RTT/PTO estimator inspired by QUIC recovery. Use fixed recovery for stable embedded links. Use dynamic recovery when delay and jitter change significantly, such as UDP or network adapters.

## Cargo Features

```toml
[features]
default = ["std"]
std = []
dynamic-recovery = []
tracing = ["dep:tracing"]
```

- `std`: enabled by default for host ergonomics.
- `dynamic-recovery`: enables RTT/PTO based recovery.
- `tracing`: enables internal library diagnostic events.

`tracing` is not part of the default feature set. The library emits tracing events only when the feature is enabled; applications, examples, binaries, and adapters decide how to configure subscribers, file output, line numbers, and filtering.

## no_std

The protocol core can be built without default features:

```toml
msrt = { version = "0.1", default-features = false }
```

Runtime adapters, HAL integrations, serial drivers, and C FFI wrappers should live outside this crate.

## Examples

The repository includes small endpoint examples:

```bash
cargo run --example mcu
cargo run --example std_client
cargo run --example std_server
```

These examples intentionally do not implement real UART, UDP, or serial drivers. They show where an adapter feeds received bytes into MSRT and where it writes bytes produced by `poll`.

## Development

```bash
cargo fmt --check
cargo check
cargo check --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo test --features tracing,dynamic-recovery
```

Benchmarks:

```bash
cargo bench --bench protocol
cargo bench --bench poll
```

## Architecture Notes

The architecture documents are the best place to understand protocol boundaries:

- [architectures/README.md](architectures/README.md)
- [architectures/core.md](architectures/core.md)
- [architectures/engine.md](architectures/engine.md)
- [architectures/reliability.md](architectures/reliability.md)
- [architectures/wire.md](architectures/wire.md)
- [architectures/endpoint.md](architectures/endpoint.md)
- [architectures/header-redesign.md](architectures/header-redesign.md)
- [architectures/stress-testing.md](architectures/stress-testing.md)
