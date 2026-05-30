# srt-runtime

`srt-runtime` 是 SRT 的协议运行时边界 crate。

这里的 runtime 不是操作系统 runtime，也不是 tokio runtime，更不是 MCU HAL。它表示 SRT 协议状态机如何被驱动。

当前阶段只保留 trait 和基础类型，不实现完整协议状态机。

## 职责

- 定义发送 message 的入口
- 定义接收 bytes 的入口
- 定义 runtime tick
- 定义 runtime event
- 组织 ACK 响应边界
- 组织重传驱动边界
- 组织 message reassembly 边界
- 为 MCU 和 OS 环境提供同一套协议驱动模型

## 非职责

- 不定义 Packet / Frame 数据结构
- 不实现 serial envelope
- 不处理 magic / length / crc
- 不实现 UART / DMA / embedded-hal
- 不实现 tokio adapter
- 不实现 CLI
- 不绑定 std

## 设计文档

见 [srt-runtime 设计](../../architectures/srt-runtime-design.md)。

## 当前结构

```text
srt-runtime/src/
├── lib.rs
├── event.rs
├── link.rs
├── message.rs
├── receive.rs
├── runtime.rs
├── scheduler.rs
├── send.rs
└── time.rs
```
