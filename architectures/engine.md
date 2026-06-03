# Engine

`engine` 是 MSRT 的协议状态机入口。它不是 executor，不是 adapter，也不是 HAL。

## 职责

engine 负责：

- 接收外部输入 bytes
- 接收外部提交的完整 message
- 分配 packet number 和 message id
- 分片并编码 outgoing packet
- 处理 incoming packet
- 生成 ACK
- 维护 in-flight packet
- 执行超时重传
- 重组 message fragment
- 通过 `poll` 返回外部需要执行的动作

engine 不负责：

- 读取 UART 寄存器
- 管理 DMA ring
- 运行 tokio task
- 运行 RTIC task
- 选择中断号
- 决定链路 write 是否阻塞
- 管理应用消息格式

## 外部 API

engine 的外部 API 应该保持窄：

```rust
Engine::new(config)
Engine::default()
engine.send(message)
engine.send_on(channel_id, message)
engine.receive(bytes)
engine.poll(now_ms, tx_buf)
```

`poll_event`、`tick`、内部队列、ACK range、reassembly slot、in-flight table 都不应该暴露给外部 crate。

## Poll 模型

`poll` 是 engine 的统一推进入口。

```text
poll(now_ms, tx_buf)
  -> 内部更新时间
  -> 检查是否需要重传
  -> 弹出一个待处理输出
  -> 如果是发送动作，把 bytes copy 到 tx_buf
  -> 返回 EnginePoll
```

外部可以这样驱动：

```text
loop:
  收到 bytes 时调用 receive(bytes)
  要发送应用消息时调用 send(message)
  周期性或事件触发调用 poll(now_ms, tx_buf)
  如果 poll 返回 Transmit，就把 bytes 写到底层链路
```

`poll` 每次返回一个动作，而不是一次性清空所有状态。这样 adapter 可以自然地控制写出节奏。

## 时间

engine 需要外部传入 `now_ms`，因为协议核心不应该依赖系统时间、硬件 timer 或 runtime clock。

时间只用于协议判断，例如：

- in-flight packet 是否超时
- 是否需要重传
- reassembly slot 是否过期

外部使用毫秒、tick 或其它单调时间源都可以，只要同一个 engine 实例内保持单调语义。

## Machine

`Engine` 是 facade，`Machine` 保存内部协议状态。

这个分层的目的是让 `engine.rs` 保持入口清晰，而把 ACK、ingress、outgoing、reassembly、retransmit 等状态逻辑放入内部模块。

外部不应该知道 `Machine` 的存在。
