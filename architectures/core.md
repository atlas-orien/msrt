# Core

`core` 定义 MSRT 的协议语言。它描述协议对象是什么，但不负责驱动通信过程。

## 职责

`core` 负责：

- `Packet`
- `PacketHeader`
- `PacketIndex`
- `PacketKey`
- `PacketPayload`
- `PacketType`
- `MessageId`

`core` 不负责：

- 字节流同步
- wire integrity tag
- ACK 策略
- 重传策略
- message reassembly buffer
- engine event queue
- 串口、TCP、USB CDC 或任何 adapter

## Packet

MSRT 是 message-oriented transport。packet 不是全局 stream 里的编号对象，而是某条 message 的 fragment。

```text
Packet
├── PacketHeader
└── PacketPayload
```

Packet 是协议传输单元。`Data` 和 `Log` packet 的 payload 是 message fragment bytes；`Ack`、`Ping`、`Pong` packet 的 payload 为空。

Wire envelope 不属于 core。它只是在字节流上传输 packet 时的外层边界。

## Message

上层提交完整 message bytes，MSRT 负责把 message 拆成 DATA packet，并在接收端重组成完整 message。

不同 packet kind 使用不同 header。`Data`/`Log` 携带：

- `message_id`
- `packet_index`
- `message_len`
- `fragment_offset`

`message_id` 标识一条 message。

`packet_index` 是 message 内部的 packet 序号，从 `0` 开始。它不能脱离 `message_id` 单独存在。

`message_len + fragment_offset + payload.len()` 用于重组完整 message。MSRT 不需要额外的 fragment flags；第一片可以由 `fragment_offset == 0` 推导，最后一片可以由 `fragment_offset + payload.len() == message_len` 推导。

`packet_type` 表示 packet 语义：

- `Data`：可靠应用 message fragment。
- `Log`：best-effort 诊断 message fragment。
- `Ack`：确认单个 Data packet。
- `Ping`：内部保活探测。
- `Pong`：内部保活响应。

应用层路由不属于 core 协议边界。如果业务需要区分命令、日志、状态或其它消息类型，应该放在 payload 自己的应用格式里。

## ACK

ACK packet 没有 payload。ACK 要确认的 packet 已经写在 ACK 的 `PacketHeader` 里：

```text
message_id + packet_index
```

ACK 是 packet 级确认，不等于完整 message 已经交付。

这个区分很重要：

```text
packet acknowledged
  对端收到了某个 packet

message delivered
  对端收齐并交付了一条完整 message
```

core 只定义 ACK packet 的边界，不决定什么时候 ACK、什么时候重传。

## MessageId

`MessageId` 当前是 `u32`。engine 不再使用简单单调递增值，而是用一个轻量、确定性的伪随机序列分配 message id。

这样做的目的不是密码学安全，而是降低极端噪声下错误 message id 仍然碰上有效上下文的概率，同时保持 `no_std` 和 MCU 友好：

```text
无外部 random crate
无系统 RNG 依赖
测试可复现
initial_message_id 可以作为 seed
```

`message_id` 只在一条 engine session 内有意义。endpoint 断开后会丢弃旧 engine，新 session 重新从配置 seed 创建状态。
