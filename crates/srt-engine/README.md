# srt-engine

Protocol engine boundaries for SRT.

This engine is not an operating-system executor, not a tokio executor, and not an MCU HAL. It describes how the SRT protocol state machine is driven.

Current status: basic types and a hardened MVP `Engine` implementation. The engine validates the non-blocking user-facing boundary and includes streaming wire ingress, fixed-capacity ACK ranges, duplicate detection, in-flight tracking, retransmission, channel reliability policy, and message reassembly. It is not the final reliability implementation yet.

## Responsibilities

- message send entry points
- non-blocking link receive entry points
- internal feed(bytes) ingress boundary
- streaming wire decode integration
- engine tick boundary
- engine events
- ACK response boundary
- fixed-capacity ACK range generation
- retransmission driving boundary
- message reassembly boundary
- minimal message fragmentation and reassembly prototype
- minimal ACK range and in-flight retransmission prototype
- minimal reliable and best-effort channel policy
- duplicate packet acknowledgement without duplicate message delivery
- one protocol driving model for MCU and OS environments

## Fragmentation

The MVP engine uses greedy fragmentation. Each packet carries up to `fragment_bytes`, and only the last packet may be shorter. For example, with `fragment_bytes = 10`, an 11-byte message is split into `10 + 1`, not `6 + 5`.

The default `fragment_bytes` is `32`.

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
├── engine/
│   ├── inflight.rs
│   ├── ingress.rs
│   ├── outgoing.rs
│   ├── packet.rs
│   ├── queue.rs
│   ├── reassembly.rs
│   └── retransmit.rs
├── event.rs
├── layout.rs
├── link.rs
├── message.rs
├── scheduler.rs
└── time.rs
```
