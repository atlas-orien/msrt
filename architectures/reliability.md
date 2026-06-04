# Reliability

`reliability` 定义 MSRT 的可靠性判断工具。它不是 engine，也不负责字节编码。

## 职责

reliability 负责：

- packet ACK tracking
- duplicate packet detection
- retransmit policy
- retry limit
- message fragment status
- channel reliability mode

reliability 不负责：

- 生成 wire bytes
- 读取 wire bytes
- 分配 message id
- 管理 engine output queue
- 直接向外部交付 message

## Packet 级可靠性

MSRT 的可靠性首先是 packet 级的。

```text
发送 packet
  -> 记录 packet key
  -> 等待 ACK
  -> 超时后重传
  -> 达到 retry limit 后失败
```

ACK 只说明 packet 被对端观察到，不说明完整 message 已经交付。

## Message 级重组

message 由多个 MESSAGE fragment 组成。reliability 可以提供 fragment 状态判断，但完整 reassembly buffer 由 engine 内部维护。

重组判断依赖：

- `channel_id`
- `message_id`
- `message_len`
- `fragment_offset`
- fragment bytes length

当 fragment 覆盖完整 `[0, message_len)` 区间时，engine 才能交付完整 message。

## Reliability Mode

MSRT 不应该假设所有 channel 都强可靠。

当前核心模式：

- `Reliable`：packet ACK eliciting，进入 in-flight，超时重传，超过 retry limit 后产生 send failure。
- `BestEffort`：不要求 ACK，不进入 in-flight，适合日志或可丢弃消息。

未来可以扩展：

- latest-only
- deadline
- channel priority
- bounded window

这些策略应该先作为 reliability 概念存在，再由 engine 选择如何使用。
