# srt-engine

Protocol engine boundaries for SRT.

This engine is not an operating-system executor, not a tokio executor, and not an MCU HAL. It describes how the SRT protocol state machine is driven.

Current status: traits and basic types only. No complete protocol state machine is implemented yet.

## Responsibilities

- message send entry points
- non-blocking link receive entry points
- internal feed(bytes) ingress boundary
- engine tick boundary
- engine events
- ACK response boundary
- retransmission driving boundary
- message reassembly boundary
- one protocol driving model for MCU and OS environments

## Non-goals

- does not define Packet / Frame structures
- does not implement serial envelopes
- does not handle magic / length / crc
- does not implement UART / DMA / embedded-hal adapters
- does not implement a tokio adapter
- does not implement a CLI
- does not bind to std

## Design

See [srt-engine design](../../architectures/v1/srt-engine-design.md).

## Current Structure

```text
srt-engine/src/
├── lib.rs
├── event.rs
├── link.rs
├── message.rs
├── receive.rs
├── engine.rs
├── scheduler.rs
├── send.rs
└── time.rs
```
