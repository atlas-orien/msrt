# SRT 总设计

SRT 是 Serial Realtime Transport 的缩写。

SRT 是一套面向原始串口类字节流的 `no_std` 实时消息传输协议标准。它的第一目标是 MCU、机器人、无人机以及 MCU 与上位机之间的通信，但协议本身不能依赖任何 MCU HAL、操作系统、异步运行时或驱动框架。

SRT 会借鉴 QUIC 中适合嵌入式消息系统的思想，例如 stream、packet 级传输语义、ack、重传、部分可靠性等。但 SRT 不是 HTTP/3，不是 TCP clone，也不是通用互联网传输协议。它是一套面向 message-driven embedded systems 的串口消息运行时传输协议。

## 设计原则

- 协议优先，运行环境后置。
- 默认 `no_std`。
- 协议标准层不依赖堆分配，除非未来某个 crate 明确选择支持。
- 协议语义以 packet 为中心。
- 使用 stream 做路由和调度。
- 支持部分可靠性，而不是假设所有消息都需要同一种交付语义。
- 实时性优先于传统字节流兼容性。
- 对 runtime 友好，但不绑定任何 runtime。
- 对 MCU 友好，但不绑定任何 MCU HAL。

## 运行环境家族

SRT 未来应该可以服务两类实现环境。

MCU 实现通常是 `no_std`、资源受限、可能没有堆分配，并且由 UART、DMA、USB CDC 或类似字节链路驱动。

上位机实现通常使用 `std`、操作系统，并且可能接入 tokio 等异步运行时。

这两类实现必须使用同一套协议标准。运行环境适配层应该依赖协议 crate，协议 crate 不应该依赖运行环境适配层。

## 分层

当前 workspace 只定义协议标准 crate。

```text
srt-error
  共享的 no_std 协议错误面。

srt-core
  核心协议结构：packet、标识符、序列号、flags、协议类型。

srt-frame
  原始串口字节流与协议 packet/frame 之间的转换边界。

srt-stream
  stream 标识、QoS、优先级、stream 状态语义。

srt-reliability
  ack、重传、超时、去重、滑动窗口的边界。

srt-runtime
  协议 runtime 边界：send、receive、tick、响应生成、协议推进。
```

## Packet 与 Frame

SRT 需要区分 packet 语义和 frame 编码。

Packet 是协议层传输单元。它承载 packet kind、stream id、sequence number、flags、payload metadata 等协议含义。

Frame 是字节流层传输单元。它解决串口传输中的边界问题，例如粘包、半包、重新同步、校验等。

这种区分可以让核心协议不绑定具体 wire encoding。未来 frame codec 可以演进，而不影响核心 packet 模型。

## 可靠性

SRT 的可靠性应该同时理解 packet 和 stream。

协议未来应该支持 ack、重传、超时处理、重复包检测、滑动窗口。不是所有消息都需要同一种可靠性。某些实时 stream 可能更关心新鲜度，而不是保证每一个旧消息都送达。

当前代码只定义边界，不实现完整算法。

## 当前非目标

- 不实现 UART driver。
- 不实现 DMA driver。
- 不实现 embedded-hal adapter。
- 不实现 tokio runtime。
- 不实现 CLI。
- 不实现 simulator。
- 不实现完整重传算法。
- 不冻结最终 wire format。

当前目标是先冻结架构和 crate 边界，再实现协议行为。
