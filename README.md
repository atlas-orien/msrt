# SRT

SRT means Serial Realtime Transport.

SRT is a common serial realtime transport protocol for MCU, robot, drone, and host-side systems. It is message-driven, stream-based, runtime-friendly, and designed for realtime links with partial reliability.

This repository is currently in the protocol-standard stage. The crates define the shared protocol boundary only. MCU `no_std` ports and host-side operating-system integrations are intentionally outside the workspace for now.

The same standard protocol should be usable from two future runtime families:

- MCU runtimes, typically `no_std` and often allocation-constrained.
- Host runtimes, using normal Rust with an operating system.

Those runtimes must adapt to the protocol. The protocol must not depend on those runtimes.

See [SPEC.md](SPEC.md) for the current standard boundary.

## Workspace

- `srt-core`: core protocol primitives.
- `srt-error`: shared protocol error types.
- `srt-reliability`: reliability traits and modules.
- `srt-runtime`: protocol runtime boundary for send, receive, response, and progress.

No MCU HAL, async runtime, serial driver, operating-system adapter, simulator, CLI implementation, or separate stream/frame crate is included at this stage.

## Development

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace
```
