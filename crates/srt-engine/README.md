# srt-engine

Protocol engine boundaries for SRT.

This engine is not an operating-system executor, not a tokio executor, and not an MCU HAL. It describes how the SRT protocol state machine is driven.

Current status: basic types and a minimal MVP `Engine` implementation. The MVP engine validates the non-blocking user-facing boundary, but it is not a complete reliability implementation yet.

## Responsibilities

- message send entry points
- non-blocking link receive entry points
- internal feed(bytes) ingress boundary
- engine tick boundary
- engine events
- ACK response boundary
- retransmission driving boundary
- message reassembly boundary
- minimal message fragmentation and reassembly prototype
- one protocol driving model for MCU and OS environments

## Non-goals

- does not define Packet / Frame structures
- does not implement the final serial envelope format
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
├── config.rs
├── engine.rs
├── event.rs
├── layout.rs
├── link.rs
├── message.rs
├── scheduler.rs
└── time.rs
```
