# Wire

`wire` 负责 MSRT 在连续字节流上的边界、校验和恢复。它不是 protocol frame 层。

## 职责

wire 负责：

- envelope magic
- envelope header
- packet length
- checksum
- streaming decode
- half packet buffering
- sticky packet splitting
- noise skip
- resync

wire 不负责：

- MESSAGE frame 语义
- ACK 策略
- packet retransmit
- message reassembly
- channel policy
- adapter read/write

## Packet、Frame、Envelope

这三个概念必须分开：

```text
Wire Envelope
└── Packet
    ├── PacketHeader
    └── PacketPayload
        ├── MESSAGE Frame
        └── ACK Frame
```

Packet 和 Frame 是 core 的协议对象。Envelope 是 wire 的字节流边界。

## 为什么需要 Envelope

UART、USB CDC、SPI byte stream、TCP mock 都可能表现为连续 bytes。连续 bytes 没有天然 packet 边界，所以 wire 必须回答：

- packet 从哪里开始？
- packet 有多长？
- bytes 是否损坏？
- 收到半包怎么办？
- 多个 packet 粘在一起怎么办？
- 噪声或丢字节后怎么恢复？

这些问题不应该放进 engine。

## Streaming Decoder

wire decoder 必须允许增量输入：

```text
receive([前 3 bytes])
receive([剩余 header])
receive([packet body 一半])
receive([packet body 剩余 + 下一个 packet 开头])
```

decoder 每次只处理当前已经到达的 bytes。它可以返回 incomplete、noise、corrupted 或完整 packet bytes，但不能阻塞等待未来输入。

## Checksum

checksum 只验证 wire bytes 是否损坏。它不表达可靠传输语义，也不替代 ACK。

当前 checksum 使用 CRC-16/XMODEM。未来如果 wire envelope 需要升级，checksum 仍然应该属于 wire 层，而不是 core 或 engine。
