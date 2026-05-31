# SRT 总设计

SRT 是 Serial Realtime Transport 的缩写。

SRT 是一套面向原始串口类字节流的 `no_std` 实时消息传输协议标准。它的第一目标是 MCU、机器人、无人机以及 MCU 与上位机之间的通信，但协议本身不能依赖任何 MCU HAL、操作系统、异步执行器或驱动框架。

SRT 会借鉴 QUIC 中适合嵌入式消息系统的思想，例如 stream、packet 级传输语义、ack、重传、部分可靠性等。但 SRT 不是 HTTP/3，不是 TCP clone，也不是通用互联网传输协议。它是一套面向 message-driven embedded systems 的串口消息传输协议。

## 设计原则

- 协议优先，运行环境后置。
- 默认 `no_std`。
- 协议标准层不依赖堆分配，除非未来某个 crate 明确选择支持。
- 协议语义以 packet 为中心。
- 使用 stream 做路由和调度。
- 支持部分可靠性，而不是假设所有消息都需要同一种交付语义。
- 实时性优先于传统字节流兼容性。
- 对 engine 友好，但不绑定任何 engine。
- 对 MCU 友好，但不绑定任何 MCU HAL。

## 运行环境家族

SRT 未来应该可以服务两类实现环境。

MCU 实现通常是 `no_std`、资源受限、可能没有堆分配，并且由 UART、DMA、USB CDC 或类似字节链路驱动。

上位机实现通常使用 `std`、操作系统，并且可能接入 tokio 等异步执行器。

这两类实现必须使用同一套协议标准。运行环境适配层应该依赖协议 crate，协议 crate 不应该依赖运行环境适配层。

## 分层

当前 workspace 只保留协议标准层的核心 crate。

```text
srt-error
  共享的 no_std 协议错误面。

srt-core
  核心协议结构：Packet、Packet Header、Packet Number、Packet Payload、Protocol Frames。

srt-reliability
  ack、重传、超时、去重、滑动窗口的边界。

srt-engine
  协议 engine 边界。

srt-wire
  串口字节流上的 envelope、编码、解码和重同步边界。
```

## Packet 与 Frame

SRT 需要区分 packet、protocol frame 和串口 envelope。

Packet 是协议层传输单元。

Protocol Frame 是 packet payload 内的语义单元，例如 STREAM、ACK、PING。

Serial Envelope 是串口字节流上的外层边界。它解决串口传输中的边界问题，例如粘包、半包、重新同步、校验等。

这三个概念不能混在一起。Protocol Frame 属于 `srt-core`。Serial Envelope 后续如果需要独立实现，应该使用不会和 Protocol Frame 混淆的命名。

## 可靠性

SRT 的可靠性应该同时理解 packet 和 stream。

协议未来应该支持 ack、重传、超时处理、重复包检测、滑动窗口。不是所有消息都需要同一种可靠性。某些实时 stream 可能更关心新鲜度，而不是保证每一个旧消息都送达。

当前代码只定义边界，不实现完整算法。

## 当前非目标

- 不实现 UART driver。
- 不实现 DMA driver。
- 不实现 embedded-hal adapter。
- 不实现 tokio executor。
- 不实现 CLI。
- 不实现 simulator。
- 不实现完整重传算法。
- 不冻结最终 wire format。

当前目标是先冻结架构和 crate 边界，再实现协议行为。

## 当前推进顺序

当前项目应按以下顺序推进：

1. 完成 `srt-core` 的 Packet / Protocol Frame 模型。
2. 重新审视 `srt-reliability`。
3. 推进 `srt-engine`。
4. 设计 `srt-wire` 串口 Serial Envelope 层。
5. 如果需要，再恢复独立 stream 状态管理 crate。
