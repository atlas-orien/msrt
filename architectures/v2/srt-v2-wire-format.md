# SRT v2 Wire Format 草案

本文是 v2 的第一版 wire format 草案。

它不是最终兼容性承诺，而是后续实现 `srt-wire` 和 `srt-core` 编解码的设计基础。

## 总体结构

SRT byte stream 中的基本边界是 Wire Envelope。

```text
Wire Envelope
├── Wire Header
├── Encoded Packet
└── CRC16
```

Encoded Packet 内部：

```text
Encoded Packet
├── Packet Header
└── Packet Payload
    ├── Frame 1
    ├── Frame 2
    └── Frame N
```

## 字节序

v2 建议统一使用 little-endian。

原因：

- MCU 常见架构友好。
- Rust 标准库和 core API 支持直接。
- 当前 smoke simulation 已经使用 little-endian。

## Wire Header 草案

固定头部长度：8 bytes。

```text
offset  size  field
0       2     magic
2       1     wire_version
3       1     header_len
4       2     packet_len
6       1     wire_flags
7       1     reserved
```

字段说明：

```text
magic
  固定为 "SR"，用于 resync。

wire_version
  当前为 1。

header_len
  当前为 8。

packet_len
  Encoded Packet 的字节长度，不包含 Wire Header 和 CRC16。

wire_flags
  Wire 层 flags，不等于 Packet Header flags。

reserved
  保留，当前必须为 0。
```

## CRC16

CRC16 位于 envelope 尾部。

```text
CRC16 = checksum(Wire Header + Encoded Packet)
```

CRC16 本身不参与 checksum。

v2 需要后续冻结：

- polynomial
- init value
- refin / refout
- xorout

当前建议优先评估 CRC-16/CCITT-FALSE 或 CRC-16/IBM-SDLC。

## Packet Header 草案

Packet Header 属于 Encoded Packet。

第一版可以保持紧凑：

```text
offset  size  field
0       1     packet_type
1       1     packet_flags
2       4     packet_number
```

固定长度：6 bytes。

字段说明：

```text
packet_type
  0 = Initial
  1 = Data
  2 = Control

packet_flags
  来自 srt-core::Flags。

packet_number
  用于 ACK、去重、重传。
```

## Packet Payload

Packet Payload 是 encoded protocol frames。

```text
Packet Payload
├── Frame
├── Frame
└── Frame
```

Packet Header 不携带 `stream_id`。

`stream_id` 只出现在 STREAM Frame。

## Frame Header 草案

每个 Protocol Frame 建议使用统一小头：

```text
offset  size  field
0       1     frame_type
1       2     frame_len
```

字段说明：

```text
frame_type
  frame 类型。

frame_len
  frame body 长度，不包含 frame header。
```

这样 decoder 可以跳过未知 frame，为后续扩展留空间。

## Frame Type 草案

```text
0x01 STREAM
0x02 ACK
0x03 PING
0x04 RESET_STREAM
```

保留：

```text
0x00 invalid
0x80..0xff experimental / vendor range
```

## STREAM Frame Body 草案

```text
offset  size  field
0       2     stream_id
2       4     message_id
6       4     message_len
10      4     fragment_offset
14      1     stream_flags
15      N     data
```

body 长度：

```text
15 + data.len()
```

这个设计保留 message boundary。

接收端通过：

```text
stream_id + message_id + message_len + fragment_offset
```

重组完整 message。

## ACK Frame Body 草案

当前 `AckFrame` 只携带 largest acknowledged。

第一版 body：

```text
offset  size  field
0       4     largest_acknowledged
```

后续可以扩展 ACK ranges。

因此 ACK frame 的 `frame_len` 很重要，可以允许未来兼容扩展。

## PING Frame Body 草案

PING body 为空。

```text
frame_type = 0x03
frame_len = 0
```

PING 的响应是 ACK，不定义 PONG。

## RESET_STREAM Frame Body 草案

需要对齐当前 `ResetStreamFrame` 的 core 定义。

如果当前 core 还缺少 error code 或 final size，v2 需要决定是否扩展。

最小 body：

```text
offset  size  field
0       2     stream_id
```

## Decoder 状态机草案

wire decoder 至少需要这些状态：

```text
ScanningMagic
ReadingHeader
ReadingPacket
ReadingChecksum
Complete
Resync
```

处理场景：

- half packet
- sticky packet
- noise before magic
- invalid length
- checksum mismatch
- unsupported version

## 长度上限

v2 必须定义最大 packet length。

建议先使用：

```text
u16 packet_len
max encoded packet length = 65535
```

实际 MCU 实现可以用更小的 capacity：

```text
const MAX_PACKET_SIZE: usize = 256;
const MAX_PACKET_SIZE: usize = 1024;
```

协议字段允许大上限，实现可以选择小上限。

## v2 待决问题

- CRC16 具体参数。
- Packet Header 是否需要 header form bit。
- PacketNumber 是否长期使用 u32。
- StreamId 是否保持 u16。
- Message length 是否使用 u32。
- ACK 是否在 v2 就支持 ranges。
- RESET_STREAM 是否需要 error code。
- 是否需要 CONNECTION_CLOSE。

这些问题应该先在文档里讨论，再进入实现。
