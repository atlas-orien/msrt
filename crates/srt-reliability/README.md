# srt-reliability

Reliability policy boundaries for SRT.

This crate defines packet acknowledgement, retransmission, timeout, deduplication, sliding window, and message fragment reliability boundaries.

Current status: traits and boundary types only. No complete reliability algorithm is implemented yet.

## Responsibilities

- packet ack tracking
- packet retransmit policy
- packet timeout policy
- duplicate packet detection
- send / receive sliding window
- message fragment descriptor
- reliability decisions for engine implementations

## Non-goals

- does not define Packet / Frame structures
- does not encode or decode packets
- does not handle serial envelopes
- does not handle magic / length / crc
- does not bind to tokio, std, RTOS, or MCU timers
- does not implement a complete message reassembly buffer

## Design

See [srt-reliability design](../../architectures/srt-reliability-design.md).

## Current Structure

```text
srt-reliability/src/
├── lib.rs
├── packet.rs
├── packet/
│   ├── ack.rs
│   ├── dedup.rs
│   ├── event.rs
│   ├── retransmit.rs
│   ├── state.rs
│   ├── timeout.rs
│   └── window.rs
├── message.rs
├── message/
│   ├── fragment.rs
│   └── status.rs
└── policy.rs
```
