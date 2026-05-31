# SRT Roadmap

This roadmap describes the current scaffold milestone and the next implementation phases.

## v1

Status: MVP complete, hardening next.

The current workspace freezes the no-std protocol crate boundaries and contains a minimal working protocol engine:

- `srt`
- `srt-core`
- `srt-error`
- `srt-reliability`
- `srt-engine`
- `srt-wire`

This milestone is not the final interoperable SRT protocol standard. It is the first working no-std engine MVP. It defines the protocol ownership model, public boundaries, basic tests, smoke simulation, CI, git hooks, and architecture documents.

The v1 MVP engine demonstrates:

- long-lived `Engine` state
- application-driven `send(message)`
- non-blocking `receive(bytes)`
- explicit `tick(now)`
- event-based output through `poll_event()`
- automatic message fragmentation into packets
- complete message reassembly
- CRC error detection
- noise detection
- minimal ACK generation
- minimal in-flight packet tracking
- tick-driven retransmission
- bidirectional Mac-to-MCU style smoke simulation

## v1 Hardening

Hardening is still v1 work. It turns the MVP into a protocol that can face real serial byte streams.

1. Implement streaming wire decode for half packets, sticky packets, and multiple packets per receive.
2. Freeze the first wire format draft.
3. Move MVP packet encoding toward the final Packet / Frame serialization model.
4. Implement duplicate packet detection and better ACK semantics.
5. Implement retransmission timeout policy, retry limits, and failure events.
6. Add multi-message and multi-stream reassembly.
7. Add heapless/no-alloc buffer strategy configuration.

Runtime adapters remain out of this repository.

## Current Non-goals

- no UART driver
- no DMA driver
- no embedded-hal adapter
- no tokio adapter
- no CLI
- no full reliability algorithm
- no finalized wire compatibility guarantee
