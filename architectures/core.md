# Core

`core` 定义 MSRT 的协议语言。它描述协议对象是什么，但不负责驱动通信过程。

## 职责

`core` 负责：

- `Packet`
- `PacketHeader`
- `PacketNumber`
- `PacketPayload`
- `PacketType`
- `Frame`
- `FrameKind`
- `MessageFrame`
- `AckFrame`
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

## Packet 与 Frame

MSRT 借鉴 packet/frame 分层思想，但它不是 QUIC，也不是 TCP clone。

```text
Packet
├── PacketHeader
└── PacketPayload
    ├── MESSAGE Frame
    └── ACK Frame
```

Packet 是协议传输单元。Frame 是 packet payload 内的语义单元。

Wire envelope 不属于 core。它只是在字节流上传输 packet 时的外层边界。

## Message

MSRT 是 message-oriented transport。上层提交完整 message bytes，MSRT 负责把 message 拆成 `MESSAGE Frame` fragment，并在接收端重组成完整 message。

`MESSAGE Frame` 携带：

- `channel_id`
- `message_id`
- `message_len`
- `fragment_offset`
- `flags`
- `data`

`channel_id + message_id` 标识一条 message，`message_len + fragment_offset + data.len()` 用于重组。

## ACK

`ACK Frame` 表示接收端观察到某些 packet number。ACK 是 packet 级确认，不等于完整 message 已经交付。

这个区分很重要：

```text
packet acknowledged
  对端收到了某个 packet

message delivered
  对端收齐并交付了一条完整 message
```

core 只定义 ACK frame 的结构，不决定什么时候 ACK、什么时候重传。
