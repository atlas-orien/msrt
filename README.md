# MSRT

MSRT means MSRT.

MSRT is a common serial realtime transport protocol for MCU, robot, drone, and host-side systems. It is message-driven, channel-based, engine-friendly, and designed for realtime links with partial reliability.

This repository is currently in the protocol-standard stage. The `msrt` crate defines the shared protocol boundary only. MCU `no_std` ports and host-side operating-system integrations are intentionally outside this repository for now.

The same standard protocol should be usable from two future implementation environments:

- MCU environments, typically `no_std` and often allocation-constrained.
- Host environments, using normal Rust with an operating system.

Those environments must adapt to the protocol. The protocol must not depend on those environments.

See [SPEC.md](SPEC.md) for the current standard boundary.
See [ROADMAP.md](ROADMAP.md) for the current scaffold scope and next phases.

## Crate

`msrt` is a single no-std crate for the protocol standard. Internal protocol boundaries live as modules:

- `core`: core protocol primitives.
- `error`: shared protocol error types.
- `reliability`: reliability traits and modules.
- `engine`: protocol engine boundary for send, receive, response, and progress.
- `wire`: wire envelope boundaries for byte stream transport.

No MCU HAL, async executor, serial driver, operating-system adapter, simulator, CLI implementation, or separate channel/frame crate is included at this stage.

## Development

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo check
cargo test
cargo run --bin msrt-smoke
```
