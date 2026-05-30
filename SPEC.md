# SRT Protocol Standard

SRT, Serial Realtime Transport, is a protocol standard for message-oriented realtime serial links.

This document is intentionally high level. The current goal is to freeze protocol ownership and crate boundaries before defining wire-level details.

The current version is a protocol scaffold, not a finalized interoperable transport implementation.

## Scope

The SRT standard owns:

- core protocol primitive types
- packet and protocol frame boundaries
- wire envelope boundaries
- stream identity and routing semantics
- acknowledgement and partial-reliability concepts
- ordering, deduplication, timeout, and sliding-window contracts
- protocol engine behavior for send, receive, response, and progress
- error surfaces shared by all implementations

The SRT standard does not own:

- UART drivers
- DMA drivers
- embedded-hal adapters
- operating-system serial APIs
- tokio, async-std, or any other runtime integration
- CLI tools
- simulator transport backends

## Runtime Model

SRT is designed to be implemented in at least two runtime families.

MCU implementations are expected to use `no_std` and may avoid allocation entirely.

Host implementations are expected to use normal Rust with operating-system support.

Both runtime families must implement the same protocol. Runtime-specific crates may be added later, but they should depend on the standard protocol crates rather than becoming part of the standard itself.

## Current Crates

- `srt`: no_std facade crate for the protocol standard.
- `srt-core`: shared primitive types and packet/frame boundaries.
- `srt-error`: shared protocol error types and result alias.
- `srt-reliability`: partial-reliability module boundaries.
- `srt-engine`: protocol engine boundary that coordinates send, receive, response, and progress.
- `srt-wire`: byte-stream wire envelope boundaries.

All current crates are protocol-standard crates. They are kept `no_std` so the standard remains portable to MCU environments.
