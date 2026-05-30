# SRT Roadmap

This roadmap describes the current scaffold milestone and the next implementation phases.

## v0.1 Protocol Scaffold

Status: complete.

The current workspace freezes the no-std protocol crate boundaries:

- `srt`
- `srt-core`
- `srt-error`
- `srt-reliability`
- `srt-engine`
- `srt-wire`

This milestone is not a usable transport implementation yet. It defines the protocol ownership model, public boundaries, basic tests, smoke simulation, CI, git hooks, and architecture documents.

## Next Phases

1. Freeze the first wire format draft.
2. Implement real wire encoding and decoding.
3. Implement packet and protocol frame serialization.
4. Implement engine state-machine prototypes.
5. Implement reliability policies.
6. Add heapless/no-alloc buffer strategies.
7. Add host and MCU adapters after the standard core stabilizes.

## v0.2 Wire Format Draft

Status: design in progress.

The next milestone focuses on the first concrete wire format draft:

- Wire Envelope layout
- Packet Header layout
- Protocol Frame layout
- CRC16 coverage and parameters
- decoder state-machine strategy
- table-driven encode/decode tests

## Current Non-goals

- no UART driver
- no DMA driver
- no embedded-hal adapter
- no tokio adapter
- no CLI
- no full protocol state machine
- no finalized wire compatibility guarantee
