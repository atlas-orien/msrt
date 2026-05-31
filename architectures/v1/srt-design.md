# SRT 总设计

SRT 是 Serial Realtime Transport 的缩写。

SRT 是一套面向原始串口类字节流的 `no_std` 实时消息传输协议标准。它的第一目标是 MCU、机器人、无人机以及 MCU 与上位机之间的通信，但协议本身不能依赖任何 MCU HAL、操作系统、异步执行器或驱动框架。

SRT 会借鉴 QUIC 中适合嵌入式消息系统的思想，例如 channel、packet 级传输语义、ack、重传、部分可靠性等。但 SRT 不是 HTTP/3，不是 TCP clone，也不是通用互联网传输协议。它是一套面向 message-driven embedded systems 的串口消息传输协议。

## 设计原则

- 协议优先，运行环境后置。
- 默认 `no_std`。
- 协议标准层不依赖堆分配，除非未来某个 crate 明确选择支持。
- 协议语义以 packet 为中心。
- 使用 channel 做路由和调度。
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

Protocol Frame 是 packet payload 内的语义单元，例如 MESSAGE、ACK。

Serial Envelope 是串口字节流上的外层边界。它解决串口传输中的边界问题，例如粘包、半包、重新同步、校验等。

这三个概念不能混在一起。Protocol Frame 属于 `srt-core`。Serial Envelope 后续如果需要独立实现，应该使用不会和 Protocol Frame 混淆的命名。

## 可靠性

SRT 的可靠性应该同时理解 packet 和 channel。

协议未来应该支持 ack、重传、超时处理、重复包检测、滑动窗口。不是所有消息都需要同一种可靠性。某些实时 channel 可能更关心新鲜度，而不是保证每一个旧消息都送达。

当前 v1 已经实现 ACK range、in-flight packet tracking、tick-driven retransmit、retry failure、多 message / 多 channel reassembly 和 BestEffort 最小策略。reliable transport 当前范围已经通过 smoke 和 deterministic long-run integration simulation，当前状态是 v1 freeze candidate。

## v1 用户 API

v1 的目标是可靠 no_std message transport。当前已经冻结外部用户应该看到的最小 API，复杂可靠性细节由 engine 内部处理。

外部用户不应该自己拆 packet，也不应该自己判断一条 message 需要发几次。用户提交的是完整 message：

```text
engine.send(message)
```

`send` 必须是非阻塞的。它只把完整 message 交给 SRT engine，内部自动完成：

```text
message
  -> 分配 message_id
  -> 按 fragment size 贪心拆成多个 fragment
  -> 每个 fragment 封装成 packet
  -> 每个 packet 编码成 wire bytes
  -> 通过 Write 事件交给外部链路写出
```

v1 MVP 默认 `fragment_bytes` 是 32。这个值不是 QUIC 的 1200 bytes 默认思路；SRT 面向串口和 MCU，小包更利于低延迟、低内存占用和调试。后续正式 wire format 可以根据 UART buffer、DMA buffer 和目标 MCU 重新评估默认值。

SRT 使用 greedy fragmentation：

```text
每个 packet 尽量携带 max_fragment_bytes。
最后一个 packet 可以更短。
不会为了平均长度而重新分配 fragment。
```

例如 `max_fragment_bytes = 10`，message 长度是 11：

```text
packet 0: 10 bytes
packet 1: 1 byte
```

不会分成：

```text
packet 0: 6 bytes
packet 1: 5 bytes
```

外部链路收到 bytes 后，用户只需要把当前已经收到的 bytes 喂回 engine：

```text
engine.receive(bytes)
```

`receive` 也必须是非阻塞的。它处理当前 bytes，可能只收到半个 packet，也可能一次收到多个 packet。完整 message 不应该靠 `receive` 阻塞等待，而是通过事件交付：

```text
poll_event()
  -> Write(bytes)
  -> Message(bytes)
  -> MessageAcked
  -> MessageFailed
```

因此 v1 MVP 的用户心智模型是：

```text
send(message)
receive(bytes)
tick(now)
poll_event()
```

其中：

- `send(message)`：提交完整 message，内部自动拆 packet，立即返回。
- `receive(bytes)`：提交当前已经到达的链路 bytes，推进接收状态，立即返回。
- `tick(now)`：推进 ACK 超时和重传状态，立即返回。
- `poll_event()`：取出协议产生的输出，例如需要写出的 bytes 或完整 message。

ACK 与重传的方向在 v1 必须保留边界：

```text
send(message)
  -> 产生多个 Write(packet)
  -> 等待对端 ACK
  -> 未 ACK 的 packet 未来由 tick 触发重传
```

但是 v1 不需要马上实现完整 ACK range、拥塞控制或复杂 loss recovery。

## v1 MVP 已验证内容

当前 v1 MVP smoke simulation 已经验证：

- Mac 与 MCU 两端都使用同一个 no_std `Engine`。
- `send(message)` 一次提交完整 message。
- engine 内部自动拆成多个 packet。
- `poll_event()` 输出待写 wire bytes。
- `receive(bytes)` 接收 packet 并推进状态。
- 干扰数据会被识别为 noise。
- CRC 错误会被检测。
- 丢包后，未 ACK 的 packet 会由 `tick(now)` 触发重发。
- 收到 DATA packet 后会生成最小 ACK。
- 收到 ACK 后会清理 in-flight packet。
- 收齐所有 fragment 后交付完整 message。
- Mac -> MCU 和 MCU -> Mac 双向 message 都可以完成。

这个 smoke 不是最终硬件测试。当前 hardening 已经在软件模拟中覆盖 half packet、sticky packet 和一次 receive 多 packet。

## 当前非目标

- 不实现 UART driver。
- 不实现 DMA driver。
- 不实现 embedded-hal adapter。
- 不实现 tokio executor。
- 不实现 CLI。
- 不实现 simulator。
- 不实现完整重传算法。
- 不实现 UART / OS runtime adapter。

当前 v1 foundation 目标已经完成：冻结架构、crate 边界、第一版 wire format draft，并验证最小协议行为。

## 后续推进顺序

当前 v1 foundation 已经完成：

1. streaming wire decode，支持半包、粘包、一次 receive 多包。
2. 第一版 wire format draft。
3. Packet / Frame serialization 对齐。
4. duplicate packet detection、ACK 和 tick retransmit 的最小闭环。
5. smoke 覆盖噪声、CRC 错误、丢包、重发、ACK 和双向 message。

v1 后续推进顺序：

1. 完善 retry/failure event。
2. 支持多 message、多 channel 的 reassembly。
3. 实现 partial reliability / latest-only 策略。
4. 设计 heapless/no-alloc buffer 策略。
5. 增加可靠传输验收 smoke 和单元测试。
