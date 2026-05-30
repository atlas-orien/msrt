# srt-reliability

`srt-reliability` 是 SRT 的可靠性策略边界 crate。

它负责预留 ack、重传、超时、去重和滑动窗口等协议边界。

当前阶段只保留模块与 trait，不实现完整可靠性算法。

## 职责

- packet ack tracking
- packet retransmit policy
- packet timeout policy
- duplicate packet detection
- send / receive sliding window
- message fragment descriptor
- 为 runtime 提供可靠性判断接口

## 非职责

- 不定义 Packet / Frame 数据结构
- 不实现 Packet 编解码
- 不处理 serial envelope
- 不处理 magic / length / crc
- 不绑定 tokio、std、RTOS 或 MCU timer
- 不实现完整 message reassembly buffer

## 设计文档

见 [srt-reliability 设计](../../architectures/srt-reliability-design.md)。

## 当前结构

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
