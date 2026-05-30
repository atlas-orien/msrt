# srt-reliability 设计

`srt-reliability` 是 SRT 的可靠性策略边界 crate。

它不定义 Packet，也不定义 Frame。Packet、Packet Header、Packet Number、ACK Frame、STREAM Frame 等协议结构都属于 `srt-core`。

`srt-reliability` 的职责是回答一个问题：

```text
在一个 message-oriented serial transport 里，哪些 packet 需要确认，哪些 packet 需要重传，哪些 packet 应该被丢弃，哪些 message fragment 已经可以交付？
```

当前阶段只冻结模块边界和 trait 方向，不实现完整算法。

## 位置

SRT 当前分层：

```text
srt-core
  定义协议结构。

srt-reliability
  定义可靠性策略边界。

srt-engine
  根据 core 和 reliability 组织发送、接收、响应、调度。

Serial Envelope / Wire Boundary
  后续负责串口字节流边界、magic、length、crc、resync。
```

`srt-reliability` 依赖 `srt-core`，但不依赖 `srt-engine`。

这样 runtime 可以自由选择不同可靠性策略：

```text
控制消息
  强可靠，必须 ack，允许重传。

遥测消息
  部分可靠，旧数据可以被新数据覆盖。

心跳消息
  可以用 PING + ACK，不需要重传旧心跳。

日志消息
  可以使用低优先级窗口。
```

## 与 runtime 的关系

`srt-engine` 是协议如何通信的中心：什么时候发 packet，收到 packet 后如何响应，如何把完整 message 交付给上层。

`srt-reliability` 不是 runtime。它是 runtime 使用的可靠性策略工具箱。

可以这样理解：

```text
srt-engine
  负责驱动协议状态机。

srt-reliability
  负责提供 ack、重传、超时、去重、窗口判断。
```

也就是说：

```text
runtime 决定做什么。
reliability 判断是否应该做。
```

## 核心输入

`srt-reliability` 的核心输入来自 `srt-core`：

- `PacketNumber`
- `AckFrame`
- `StreamFrame`
- `StreamId`
- `MessageId`
- `message_len`
- `fragment_offset`

其中 `PacketNumber` 用于 packet 级可靠性：

```text
发送 packet
  -> 记录 PacketNumber
  -> 等待 ACK
  -> 超时后由策略决定是否重传
```

`StreamFrame` 内的 `StreamId`、`MessageId`、`message_len`、`fragment_offset` 用于 message fragment 级重组和交付判断。

## Packet 级可靠性

Packet 级可靠性关注 packet 是否到达对端。

当前需要保留这些边界：

- ack tracking
- duplicate packet detection
- packet timeout
- retransmit decision
- send window
- receive window

Packet 级可靠性不关心上层 message 的语义。

它只关心：

```text
PacketNumber 是否已经发送？
PacketNumber 是否已经确认？
PacketNumber 是否重复到达？
PacketNumber 是否超时？
PacketNumber 是否仍在窗口内？
```

## Message fragment 级可靠性

SRT 是 message-oriented transport。

STREAM Frame 承载的是完整 message 的 fragment，而不是无限 byte-stream 的任意切片。

因此 reliability 未来还需要服务 message fragment 重组：

```text
stream_id + message_id
  定位一条 message。

message_len
  判断完整 message 的目标长度。

fragment_offset + data.len()
  判断当前 fragment 覆盖的范围。
```

当 fragment 覆盖完整区间：

```text
[0, message_len)
```

runtime 才能把完整 message bytes 交付给上层。

当前阶段不实现 reassembly buffer，因为这会牵涉内存模型、heapless 容量、丢弃策略和 runtime 调度。

## ACK

SRT 借鉴 QUIC 的方向：PING 不需要单独的 PONG Frame，ACK 可以作为响应。

ACK 的语义是：

```text
我已经观察到了某些 PacketNumber。
```

ACK 不等于完整 message 已经交付。一个 packet 可能只携带某个 message 的一部分 fragment。

所以需要区分：

```text
packet acknowledged
  packet 已被对端收到。

message completed
  message 的所有 fragment 已经收齐，可以交付。
```

这两个状态不能混在一起。

## 重传

重传策略不应该直接假设所有 stream 都强可靠。

未来可能存在不同策略：

```text
Reliable
  丢包后重传，直到确认或达到上限。

BestEffort
  不重传，适合高频实时遥测。

LatestOnly
  旧 message 可以被同 stream 的新 message 替代。

Deadline
  超过时间窗口后不再重传。
```

当前阶段只保留 `RetransmitPolicy` 之类的 trait，不实现具体策略。

## 超时

超时是策略输入，不是算法本身。

`srt-reliability` 可以定义 timeout 事件边界，但不应该绑定：

- 系统时钟
- tokio timer
- MCU timer
- RTOS tick
- async runtime

时间来源应该由 runtime 或运行环境适配层提供。

## 去重

串口通信可能出现重传后的重复 packet。

`srt-reliability` 需要保留 duplicate detection 边界：

```text
PacketNumber
  -> 是否已经处理过？
```

重复 packet 仍然可能需要触发 ACK，因为对端可能没有收到之前的 ACK。

## 滑动窗口

窗口用于限制发送端和接收端的在途数据规模。

在 MCU 场景中，窗口不只是吞吐优化，也是内存保护：

```text
send window
  限制未确认 packet 数量。

receive window
  限制可接受 packet number 范围。

message reassembly budget
  限制未完成 message fragment 占用的空间。
```

当前阶段只保留 `SlidingWindow` 边界。

## 不属于本 crate 的内容

`srt-reliability` 不负责：

- Packet / Frame 数据结构定义
- Packet 编解码
- Serial Envelope
- magic / length / crc
- 串口 resync
- UART / DMA / embedded-hal
- tokio / std runtime
- mailbox / scheduler / dispatcher
- 完整 message buffer 分配策略

这些内容应该分别由 `srt-core`、后续 wire 层和 `srt-engine` 处理。

## 目录结构

当前目录结构：

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

这个结构按可靠性关心的层次拆分：packet 级可靠性、message fragment 级边界、stream/message 的可靠性策略描述。

## 第一阶段结论

第一阶段的 `srt-reliability` 应该只做三件事：

1. 明确 packet 级可靠性边界。
2. 明确 message fragment 重组会依赖可靠性策略，但暂不实现。
3. 为 runtime 留出可插拔策略接口。

它不是 runtime，也不是 serial wire codec。

它是 SRT 在 `no_std` 环境下实现可靠通信的策略层。
