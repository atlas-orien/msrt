# srt

No-std facade crate for the SRT protocol standard.

This crate re-exports the protocol crates and the current MVP engine API from `srt-engine`. Protocol state lives in `srt-engine`, not in this facade crate.

## Re-exports

- `srt::core`
- `srt::error`
- `srt::reliability`
- `srt::engine`
- `srt::wire`

## Non-goals

- does not provide an OS SDK
- does not provide an MCU HAL adapter
- does not provide a tokio adapter
- does not implement a CLI
- does not implement complete reliability or loss recovery
- does not implement complete wire format negotiation

This crate is currently a facade and integration-test target.

## Minimal API

```rust
let mut engine = srt::Engine::new(srt::EngineConfig::default());

engine.send(b"hello srt testing")?;

while let Some(event) = engine.poll_event() {
    if let srt::EngineOutput::Write(write) = event {
        serial.write(write.as_bytes());
    }
}
```

The engine API is non-blocking:

- `send(message)` queues a complete message and returns immediately.
- `receive(bytes)` feeds already-arrived wire bytes and returns immediately.
- `tick(now)` is reserved for timeout and retransmission work.
- `poll_event()` drains protocol outputs such as `Write` and complete `Message` events.

The MVP engine can split one message into multiple packet write events and reassemble those packet fragments into one delivered message. ACK and retransmission behavior is still a boundary, not a complete implementation.

## Smoke Test

```sh
cargo run -p srt --bin srt-smoke
```

The smoke binary sends one complete message, lets the sender engine produce multiple write events, feeds those bytes to another engine, and prints the complete received message.
