# srt-wire

Wire envelope boundaries for SRT.

This crate defines the `SRT Packet <-> Wire Envelope bytes` boundary, including encoding, decoding, checksum, and resynchronization contracts.

Current status: fixed envelope primitives and a no-alloc streaming decoder MVP.

## Responsibilities

- wire envelope header
- magic
- wire flags
- encoded packet length
- checksum boundary
- encoder boundary
- decoder boundary
- streaming decode for half packets, sticky packets, noise, and CRC failures
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

See [srt-wire design](../../architectures/v1/srt-wire-design.md).
