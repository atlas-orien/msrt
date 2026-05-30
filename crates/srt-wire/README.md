# srt-wire

`srt-wire` 是 SRT 的串口字节流边界 crate。

它负责定义 `SRT Packet <-> Wire Envelope bytes` 的编码、解码、校验和重同步边界。

当前阶段只保留 trait、状态和基础类型，不实现完整 wire format。

## 职责

- wire envelope header
- magic
- wire flags
- encoded packet length
- checksum boundary
- encoder boundary
- decoder boundary
- resync state

## 非职责

- 不定义 protocol frame 语义
- 不处理 ACK
- 不处理重传
- 不处理去重
- 不处理 message reassembly
- 不绑定 UART / DMA / tokio / std
- 不使用 `Vec`

## 设计文档

见 [srt-wire 设计](../../architectures/srt-wire-design.md)。
