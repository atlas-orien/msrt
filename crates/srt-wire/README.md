# srt-wire

Wire envelope boundaries for SRT.

This crate defines the `SRT Packet <-> Wire Envelope bytes` boundary, including encoding, decoding, checksum, and resynchronization contracts.

Current status: traits, states, and basic types only. No complete wire format is implemented yet.

## Responsibilities

- wire envelope header
- magic
- wire flags
- encoded packet length
- checksum boundary
- encoder boundary
- decoder boundary
- resync state

## Non-goals

- does not define protocol frame semantics
- does not handle ACK
- does not handle retransmission
- does not handle deduplication
- does not handle message reassembly
- does not bind to UART / DMA / tokio / std
- does not use `Vec`

## Design

See [srt-wire design](../../architectures/srt-wire-design.md).
