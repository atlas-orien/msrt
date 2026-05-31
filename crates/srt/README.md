# srt

No-std facade crate for the SRT protocol standard.

This crate is the no-std facade for the current SRT MVP API. It re-exports the protocol crates and provides a small endpoint state machine used to validate the user-facing boundary.

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

This crate is currently a facade, integration-test target, and minimal endpoint API prototype.

## Minimal API

```rust
let mut endpoint = srt::Endpoint::new(srt::EndpointConfig::default());

endpoint.send(b"hello srt testing")?;

while let Some(event) = endpoint.poll_event() {
    if let srt::EndpointEvent::Write(write) = event {
        serial.write(write.as_bytes());
    }
}
```

The endpoint API is non-blocking:

- `send(message)` queues a complete message and returns immediately.
- `receive(bytes)` feeds already-arrived wire bytes and returns immediately.
- `tick(now)` is reserved for timeout and retransmission work.
- `poll_event()` drains protocol outputs such as `Write` and complete `Message` events.

The MVP endpoint can split one message into multiple packet write events and reassemble those packet fragments into one delivered message. ACK and retransmission behavior is still a boundary, not a complete implementation.

## Smoke Test

```sh
cargo run -p srt --bin srt-smoke
```

The smoke binary sends one complete message, lets the sender produce multiple write events, feeds those bytes to another endpoint, and prints the complete received message.
