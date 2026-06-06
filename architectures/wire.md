# Wire

`wire` 负责 MSRT 在连续字节流上的边界、完整性校验和恢复。它不是 protocol frame 层。

## 职责

wire 负责：

- envelope magic
- envelope header
- packet length
- header CRC-8
- integrity tag
- streaming decode
- half packet buffering
- sticky packet splitting
- noise skip
- resync

wire 不负责：

- DATA payload 语义
- ACK 策略
- packet retransmit
- message reassembly
- channel policy
- adapter read/write

## Packet、Payload、Envelope

这三个概念必须分开：

```text
Wire Envelope
└── Packet
    ├── PacketHeader
    └── PacketPayload
        ├── DATA: message fragment bytes
        └── ACK/PING/PONG: empty
```

Packet 是 core 的协议对象。Envelope 是 wire 的字节流边界。

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

decoder 也不能假设上层每次只传入一个串口中断字节。上层可能一次传入半包、完整包、多个粘在一起的包，或者噪声后面已经跟着合法包。因此 decoder 的 buffer 是连续 byte stream 的窗口，而不是“当前 packet 专用缓存”。

## Resync 原则

wire 层面对的是不可信 byte stream。magic、length、header CRC-8 和 envelope integrity tag 都可能因为噪声或丢字节而失效。

当 header 校验失败时，decoder 不应该清空整个 buffer。因为 header 失败只说明当前位置的 magic 不能作为合法 envelope 起点，不能说明后面的 bytes 全部无效。

```text
A5 bad_len bad_crc A5 good_len good_crc packet integrity_tag
^ fake magic
                 ^ next candidate magic
```

正确的重同步策略是：

```text
丢掉当前位置 fake magic
继续在剩余 buffer 中扫描下一个 magic
```

这样可以保留已经到达的后续合法 envelope。只有当一个完整候选 envelope 的 integrity tag 失败时，decoder 才可以丢掉这个候选 envelope 对应的 bytes。

因此 wire decoder 的丢弃粒度应该按“已经确定无效的范围”决定：

- magic 前面的 bytes：确定是 noise，可以丢弃。
- header 校验失败：只确定当前位置 magic 无效，丢 1 byte 后 resync。
- length 超过 decoder capacity：当前位置 magic 不可信，丢 1 byte 后 resync。
- integrity tag 失败：完整候选 envelope 无效，丢弃这个候选 envelope。
- packet 不完整：不丢弃，等待后续 bytes。

## Integrity

integrity tag 只验证 wire bytes 是否损坏或不合法。它不表达可靠传输语义，也不替代 ACK。

默认 `EngineConfig` 使用 CRC-16/XMODEM。CRC-32、CRC-64 和轻量 keyed validation 通过 `EngineConfig::integrity` 选择。不同 integrity backend 的 tag 长度不同，因此 envelope total length 必须根据当前 engine 配置计算，而不是使用全局固定长度。

length 字段仍然使用固定 CRC-8 做第一阶段保护，用于在完整 envelope 尚未到达前验证 packet length。

integrity 的边界是 wire 合法性，不是业务数据正确性。协议层交付的是 bytes。如果业务 payload 内部有命令、字段、版本或业务 checksum，应该由业务层自己判断。

## Length Framing 的代价

MSRT 使用：

```text
magic + length + length_crc8 + packet + integrity_tag
```

这种 length-based framing 正常效率高，不需要 tail magic，也不需要对 payload 中出现 tail magic 做转义。

代价是：如果 packet body 中间发生连续丢字节，decoder 会按照原 length 等待后续 bytes，坏 envelope 可能吞掉后面真实 envelope 的一部分。这就是压力测试里 `burst-drop` 比整包丢失更危险的原因。

这个现象不是 ACK bug，也不是 engine bug，而是 length framing 面对裸字节流连续丢失时的自然成本。
