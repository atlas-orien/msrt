# Reliability

`reliability` 定义 MSRT 的可靠性判断工具。它不是 engine，也不负责字节编码。

## 职责

reliability 负责：

- packet ACK tracking
- duplicate packet detection
- retransmit policy
- retry limit
- dynamic recovery policy
- message fragment status
- packet kind reliability mode

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

Packet identity 是：

```text
message_id + packet_index
```

这里没有全局 packet number，也没有 ACK range。MSRT 的 ACK 是单 packet key 确认。这个边界不能再退回 global stream 模型。

## ACK 和 Duplicate

重复 Data packet 的处理原则：

```text
duplicate Data
  -> 重新 ACK
  -> 不重复交付 message fragment
```

ACK 不能因为“这个 key 之前 ACK 过”就被全局去重。ACK 自己也会丢，所以 duplicate Data 是对端明确告诉我们“它可能没收到上一次 ACK”的信号。

Data/retransmit 队列可以按同一个 `PacketKey` 替换冗余 write event，避免队列膨胀。但 ACK pending 队列表达的是“现在需要发出的确认”，不能用同样思路压掉。

## Message 级重组

message 由多个 MESSAGE fragment 组成。reliability 可以提供 fragment 状态判断，但完整 reassembly buffer 由 engine 内部维护。

重组判断依赖：

- `message_id`
- `message_len`
- `fragment_offset`
- fragment bytes length

当 fragment 覆盖完整 `[0, message_len)` 区间时，engine 才能交付完整 message。

## Reliability Mode

MSRT 不应该假设所有 packet kind 都强可靠。

当前核心模式：

- `Data` 使用 `Reliable`：packet ACK eliciting，进入 in-flight，超时重传，超过 retry limit 后产生 send failure。
- `Log` 使用 `BestEffort`：不要求 ACK，不进入 in-flight，适合日志或可丢弃消息。
- `Ack`、`Ping`、`Pong` 是内部控制包，不作为应用 message 进入可靠性模式选择。

未来可以扩展：

- latest-only
- deadline
- packet kind priority
- bounded window

这些策略应该先作为 reliability 概念存在，再由 engine 选择如何使用。

## 固定恢复

默认恢复策略是固定 RTO + 固定 retry limit：

```text
now_ms - last_sent_ms >= retransmit_timeout_ms
  -> 如果 attempts < max_retransmit_attempts:
       retransmit
  -> 否则:
       SendFailed
```

这个策略适合 UART、USB CDC、短距离 MCU 链路。它的优点是代码小、行为可预测、测试容易复现。

## 动态恢复

`dynamic-recovery` feature 提供轻量 RTT/PTO 恢复策略。它借鉴 Quinn/QUIC 的思想，但没有引入 QUIC 的完整复杂度。

动态恢复维护：

```text
latest RTT
smoothed RTT
RTT variance
timer granularity
max ACK delay
exponential backoff
```

PTO 形状是：

```text
PTO = RTT + max(4 * RTT variance, timer granularity) + max_ack_delay
timeout = PTO * 2^attempts
```

收到 ACK 时，在删除 in-flight packet 前用 `now_ms - last_sent_ms` 更新 RTT 估算。

动态恢复适合：

- UDP
- 公网网络
- 延迟突然变大
- 抖动明显
- 同一连接内 RTT 持续变化

不建议把它作为默认 MCU 串口策略。串口网络通常延迟稳定，固定 RTO 更简单。

## SendFailed 边界

`SendFailed` 不是 panic，也不是库崩溃。它表示当前 reliable message 的某个 packet 超过恢复预算。

engine 返回 `SendFailed` 后，endpoint 层应该丢弃当前 engine session。因为旧 session 里可能还有 ingress buffer、in-flight、dedup 和 reassembly 状态，局部清理容易出错。

```text
SendFailed
  -> endpoint.disconnect()
  -> next connect/accept creates fresh Engine
```
