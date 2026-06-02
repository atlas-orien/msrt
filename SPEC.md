# SRT Protocol Standard

SRT, Serial Realtime Transport, is a protocol standard for message-oriented realtime serial links.

This document is intentionally high level. The current goal is to keep protocol ownership and crate boundaries clear while the first no-std draft is hardened.

The current version is a v1 no-std protocol foundation. It is suitable for internal testing and boundary review, but v1 is not complete until reliable transport behavior is implemented and verified.

## Scope

The SRT standard owns:

- core protocol primitive types
- packet and protocol frame boundaries
- wire envelope boundaries
- channel identity and routing semantics
- acknowledgement and partial-reliability concepts
- ordering, deduplication, timeout, and sliding-window contracts
- protocol engine behavior for send, receive, response, and progress
- error surfaces shared by all implementations

The SRT standard does not own:

- UART drivers
- DMA drivers
- embedded-hal adapters
- operating-system serial APIs
- tokio, async-std, or any other executor integration
- CLI tools
- simulator transport backends

## Implementation Model

SRT is designed to be implemented in at least two implementation environments.

MCU implementations are expected to use `no_std` and may avoid allocation entirely.

Host implementations are expected to use normal Rust with operating-system support.

Both implementation environments must implement the same protocol. Environment-specific crates may be added later, but they should depend on the standard protocol crates rather than becoming part of the standard itself.

## Current Crates

- `srt`: no_std facade crate for the protocol standard.
- `srt::core`: shared primitive types and packet/frame boundaries.
- `srt::error`: shared protocol error types and result alias.
- `srt::reliability`: partial-reliability module boundaries.
- `srt::engine`: protocol engine boundary that coordinates send, receive, response, and progress.
- `srt::wire`: byte-stream wire envelope boundaries.

All current crates are protocol-standard crates. They are kept `no_std` so the standard remains portable to MCU environments.

See [SRT v1 Stable Protocol Draft](architectures/v1/srt-stable-protocol-draft.md) for the current v1 draft wire and packet model.
See [SRT v1 Reliable Transport Plan](architectures/v1/srt-reliable-transport-plan.md) for the remaining v1 reliability work.
