# srt

No-std facade crate for the SRT protocol standard.

This crate exposes the current MVP SRT API and hides most workspace internals behind a small no-std facade. Protocol state lives in `srt-engine`, but normal users should start from the top-level `srt` types.

## Re-exports

- `srt::Engine`
- `srt::Config`
- `srt::Event`
- `srt::Message`
- `srt::Write`
- `srt::Receive`
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
let mut engine = srt::Engine::new(srt::Config::default());

engine.send(b"hello srt testing")?;

while let Some(event) = engine.poll_event() {
    if let srt::Event::Write(write) = event {
        serial.write(write.as_bytes());
    }
}
```

The engine API is non-blocking:

- `send(message)` queues a complete message and returns immediately.
- `receive(bytes)` feeds already-arrived wire bytes and returns immediately.
- `tick(now)` is reserved for timeout and retransmission work.
- `poll_event()` drains protocol outputs such as `Write` and complete `Message` events.

The MVP engine can split one message into multiple packet write events, reassemble those packet fragments into one delivered message, generate minimal ACK packets, track in-flight packets, and retransmit them when `tick(now)` is called. This is still an MVP reliability loop, not the final reliability algorithm.

## Smoke Test

```sh
cargo run -p srt --bin srt-smoke
```

The smoke binary simulates Mac-to-MCU communication with noise, CRC corruption, packet drop, ACK, retransmission, and bidirectional complete messages.
