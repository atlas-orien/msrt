# SRT v1 可靠传输补齐计划

## 状态

当前 v1 还没有完成。

已经完成的是：

- no_std workspace 和 crate 边界。
- message-oriented API 方向。
- Wire Envelope。
- streaming decode。
- CRC-16/XMODEM。
- Packet Header / MESSAGE Frame / ACK Frame 基础布局。
- 最小 ACK、dedup、in-flight、tick retransmit。
- `SendFailed` / `RetryLimitReached` 事件边界。
- message 级失败聚合的最小边界。
- fixed-slot 多 message reassembly 的最小边界。
- half packet、sticky packet、noise、CRC error、drop、duplicate、同时双向发送 smoke。
- ACK range 的最小 fixed-capacity 编码和批量 in-flight 清理。
- retransmit timeout tick 的最小策略。

但这些只能证明 foundation 是正确的，不能证明 v1 已经是可靠传输。

v1 的目标必须是：

```text
在 MCU / 上位机两端不断同时收发 message 的情况下，
SRT 能以 no_std 状态机形式提供可验证的可靠 message transport。
```

## 当前 smoke 证明了什么

当前 smoke 已经证明：

- 两端可以同时 `send(message)`。
- 两端都可以把 message 拆成多个 packet。
- DATA 和 ACK 可以交错出现。
- 半包、粘包、干扰、CRC 错误、丢包后可以继续推进。
- tick 可以触发未 ACK packet 重发。
- 两端最终都可以收到对方的一条完整 message。

这说明 engine 模型是对的：

```text
send(message)
receive(bytes)
tick(now)
poll_event()
```

但是它还没有证明：

- 多条 message 同时 in-flight。
- 多个 channel 同时 reassembly。
- ACK range。
- retry limit 到达后的失败事件。
- partial reliability。
- buffer budget 和窗口保护。

## v1 完成标准

v1 完成时必须满足：

1. reliable message 可以在丢包、乱序、重复、CRC 错误后最终交付。
2. 多条 message 可以同时处于 reassembly / in-flight 状态。
3. 多个 channel 的 message 不会互相污染。
4. ACK 语义可以覆盖连续和非连续 packet。
5. 重试达到上限后会产生明确失败事件。
6. engine 不会无限增长状态。
7. 所有状态机仍然 no_std、无阻塞。
8. smoke 和单元测试能覆盖真实双向持续收发场景。

## 不属于 v1 的内容

v1 不做运行环境适配：

- UART driver。
- DMA driver。
- embedded-hal adapter。
- RTIC / Embassy adapter。
- Tokio / std adapter。
- CLI。
- 多语言 SDK。

v1 也不做：

- TLS。
- congestion control。
- connection migration。
- QUIC stream。
- HTTP/3。

## 下一步一：ACK range

当前状态：ACK range 已开始落地。

这可以跑通 demo，但效率和语义都不够完整。

v1 需要定义 ACK range：

```text
largest_acknowledged
ack_range_count
first_ack_range
ack_ranges...
```

目标不是照搬 QUIC，而是解决 SRT 的实际问题：

- 一次 ACK 多个 packet。
- 表达中间缺口。
- 避免 ACK packet 过多。
- 支持后续 retransmit decision。

v1 可以先限制 ACK range 数量，保持 no_std 固定容量。

当前已经补齐：

- `AckRange`。
- fixed-capacity `AckFrame`。
- ACK frame 编码 / 解码。
- 接收端累计 observed packet numbers 并生成 ACK range。
- 发送端收到 ACK range 后清理多个 in-flight packet。
- gap ACK range 测试：ACK `0` 和 `2..3` 后只重发缺失的 packet `1`。

后续仍需要补齐：

- ACK range 的正式 wire draft 文档更新。
- ACK range 压缩和过期策略。

## 下一步二：重试失败事件

当前状态：事件边界和最小 retry limit 已开始落地。

当前 tick 会重发所有 in-flight packet。

这只是验证闭环，不是可靠传输完成版本。

v1 需要：

- 每个 in-flight packet 记录 attempts。
- 每个 packet 有 timeout / deadline 判断。
- 达到 retry limit 后停止重发。
- 产生明确事件。

当前事件：

```text
EngineOutput::SendFailed {
  message_id,
  reason,
}
```

第一阶段可以先只支持：

```text
reason = RetryLimitReached
```

当前已经补齐：

- 一个 packet 达到 retry limit 后，按 message 产生 `SendFailed`。
- 同一条 message 的其它 in-flight packet 会被移除。
- 同一条 message 只产生一次 failed event。

当前已经补齐：

- `retransmit_timeout_ms` 配置。
- in-flight packet 记录 `last_sent_ms`。
- `tick(now)` 只有达到 timeout 后才重发。
- 每次重发后更新 `last_sent_ms`。

后续仍需要继续补齐：

- message 失败后的对端取消 / 本端清理语义。

## 下一步三：多 message reassembly

当前状态：fixed-slot 多 message reassembly 已开始落地。

真实场景中，同一端可能连续收到：

```text
message A fragment 0
message B fragment 0
message A fragment 1
message B fragment 1
```

v1 需要按 message key 管理 reassembly：

```text
MessageKey = ChannelId + MessageId
```

固定容量：

```text
MAX_REASSEMBLY_MESSAGES
MAX_MESSAGE_BYTES
```

行为：

- 不同 message 的 fragment 不互相覆盖。
- 完成一条 message 后释放对应 slot。
- 超出 reassembly budget 时产生错误或 drop 策略。

当前已经补齐：

- `MAX_REASSEMBLY_MESSAGES`。
- 按 `ChannelId + MessageId` 查找 reassembly slot。
- A/B 两条 message fragment 交错到达时可以分别完成。

后续仍需要补齐：

- reassembly slot timeout。
- reassembly budget 满后的明确策略。
- 多 channel smoke。

## 下一步四：多 channel

当前状态：`send_on(channel_id, message)` 已开始落地。

v1 需要证明：

- 不同 `ChannelId` 的 message 可以同时收发。
- 相同 `MessageId` 在不同 channel 下不会冲突。
- channel 可以绑定 reliability policy。

v1 不需要复杂动态 channel negotiation。

第一阶段可以只支持调用方显式发送到 channel：

```text
send_on(channel_id, message)
```

默认：

```text
send(message) == send_on(ChannelId::CONTROL, message)
```

当前已经补齐：

- `Engine::send_on(channel_id, message)`。
- facade 导出 `srt::ChannelId`。
- outgoing MESSAGE frame 编码传入的 `channel_id`。
- `MessageEvent` 携带 `channel_id`。
- `SendFailedEvent` 携带 `channel_id`。
- 多 channel smoke，验证不同 channel 的 fragment 交错到达时不会串台。

后续仍需要补齐：

- channel 级 reliability policy。
- 不同 channel 的独立 message id 策略是否需要调整。

## 下一步五：partial reliability

v1 的核心目标是可靠传输，但 SRT 长期目标包含 partial reliability。

v1 至少需要把 policy 行为定义清楚：

```text
Reliable
  需要 ACK，超时重传，失败后报告。

BestEffort
  不进入 in-flight，不重传，不等待 ACK。

LatestOnly
  同 channel 新 message 可以替代旧 message。

Deadline
  超过 deadline 后停止重传。
```

实现顺序建议：

1. Reliable。
2. BestEffort。
3. LatestOnly / Deadline 先文档冻结，再实现。

## 下一步六：窗口和 buffer budget

可靠传输必须保护 MCU 内存。

v1 需要冻结这些容量策略：

```text
MAX_IN_FLIGHT_PACKETS
MAX_REASSEMBLY_MESSAGES
MAX_MESSAGE_BYTES
MAX_ACK_RANGES
MAX_EVENTS
```

当容量不足时必须有明确行为：

- 返回 error。
- drop lower-priority message。
- send failed。
- reject incoming fragment。

不能 silently overwrite。

## 下一步七：验收测试

v1 可靠传输完成前，至少需要这些测试：

1. 单向多 message 同时 in-flight。
2. 双向多 message 同时 in-flight。
3. 多 channel 同时收发。
4. fragment 乱序到达。
5. ACK range 覆盖多个 packet。
6. ACK range 带 gap 后只重发缺失 packet。
7. retry limit reached 后产生 failed event。
8. duplicate retransmit 不重复交付 message。
9. reassembly buffer 满后的行为明确。
10. BestEffort 丢包后不重传。

smoke 需要增加一个持续收发场景：

```text
mac sends A0, A1, A2
mcu sends B0, B1, B2
link randomly drops/corrupts/reorders packets
both sides continue receive/tick/poll
reliable messages eventually arrive
failed messages produce explicit failed events
```

## 推荐实现顺序

不要一次性重写 engine。

建议顺序：

1. 定义 `SendFailed` / `RetryLimitReached` 事件。已开始落地。
2. 给 in-flight packet 增加 attempts metadata。已开始落地。
3. 实现最小 retry limit。已开始落地。
4. 实现 message 级失败聚合。已开始落地。
5. 把 reassembly 从 single buffer 改成 fixed slot table。已开始落地。
6. 增加 `send_on(channel_id, message)`。已开始落地。
7. 实现 ACK range 数据结构和编码。已开始落地。
8. 让 retransmit 只重发缺失 packet。已开始落地。
9. 补 smoke 和单元测试。已开始落地。
10. 最后再更新 stable protocol draft。

## 结论

现在不能冻结 v1。

当前应该说：

```text
v1 foundation: 已完成
v1 hardening: 当前范围已完成
v1 reliable transport: 下一步
v1 stable: 未完成
```

v1 不追求更多运行环境功能，也不追求复杂生态包装。

v1 只追求一件事：

```text
把 SRT 做成可靠的 no_std message transport。
```
