# MSRT Roadmap

This roadmap describes the current scaffold milestone and the next implementation phases.

## v1

Status: foundation complete, hardening complete for current scope, v1 reliable transport not complete.

The current crate freezes the no-std protocol module boundaries and contains a minimal working protocol engine:

- `msrt::core`
- `msrt::error`
- `msrt::reliability`
- `msrt::engine`
- `msrt::wire`

This milestone is not the final interoperable MSRT protocol standard. It is the first working no-std engine foundation. It defines the protocol ownership model, public boundaries, basic tests, smoke simulation, CI, git hooks, and architecture documents.

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

The current v1 protocol draft has a foundation layout:

1. Wire Envelope uses magic, version, header length, packet length, wire flags, reserved byte, and CRC-16/XMODEM.
2. Packet Header encodes packet type, packet flags, and packet number.
3. v1 protocol frames are MESSAGE and ACK only.
4. MESSAGE frame serialization encodes channel id, message id, message length, fragment offset, message flags, and fragment bytes.
5. ACK frame serialization encodes a single acknowledged packet number.
6. The engine uses greedy fragmentation and event-based message delivery.

Remaining work before v1 can be called complete:

1. ACK ranges.
2. Retry limits and send-failed events.
3. Multi-message and multi-channel reassembly behavior.
4. Partial reliability policy implementation.
5. Heapless/no-alloc buffer strategy configuration.

See [MSRT v1 Reliable Transport Plan](architectures/v1/srt-reliable-transport-plan.md).

Runtime adapters remain out of this repository.

## Current Non-goals

- no UART driver
- no DMA driver
- no embedded-hal adapter
- no tokio adapter
- no CLI
- no finalized wire compatibility guarantee
