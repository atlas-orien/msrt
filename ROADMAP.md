# SRT Roadmap

This roadmap describes the current scaffold milestone and the next implementation phases.

## v1

Status: MVP complete, hardening complete for current scope, stable protocol draft next.

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

Hardening is still v1 work. The current hardening scope turns the MVP into a protocol that can face real serial byte streams.

Completed in the current hardening scope:

1. Streaming wire decode for half packets, sticky packets, and multiple packets per receive.
2. Noise and CRC error handling.
3. Duplicate packet detection.
4. Minimal ACK / retransmit smoke coverage.
5. Message-oriented terminology cleanup around `MessageFrame` and `ChannelId`.

## v1 Stable Protocol Draft

The next phase freezes the protocol draft before changing more code:

1. Freeze the first wire format draft.
2. Move MVP packet encoding toward the final Packet / Frame serialization model.
3. Freeze MESSAGE / ACK frame serialization.
4. Freeze CRC16 parameters.
5. Define ACK semantics, retry limits, and failure events.
6. Define multi-message and multi-channel reassembly behavior.
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
