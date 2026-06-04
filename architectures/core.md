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
- `ChannelId`
- `MessageId`

`core` 不负责：

- 字节流同步
- checksum
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

Packet 是协议传输单元。DATA packet 的 payload 是 message fragment bytes；ACK、PING、PONG packet 的 payload 为空。

Wire envelope 不属于 core。它只是在字节流上传输 packet 时的外层边界。

## Message

上层提交完整 message bytes，MSRT 负责把 message 拆成 DATA packet，并在接收端重组成完整 message。

`PacketHeader` 携带：

- `channel_id`
- `message_id`
- `packet_index`
- `message_len`
- `fragment_offset`
- `fragment_flags`

`channel_id + message_id` 标识一条 message。

`packet_index` 是 message 内部的 packet 序号，从 `0` 开始。它不能脱离 `message_id` 单独存在。

`message_len + fragment_offset + payload.len()` 用于重组完整 message。

## ACK

ACK packet 没有 payload。ACK 要确认的 packet 已经写在 ACK 的 `PacketHeader` 里：

```text
channel_id + message_id + packet_index
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
