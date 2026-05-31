# SRT v1 Stable Protocol Draft

## 状态

本文档是 SRT v1 stable protocol 的第一版草案。

它的目的不是继续探索方向，而是把当前已经验证过的 MVP / hardening 行为整理成可以审核的协议标准边界。

当前状态：

```text
v1 MVP
  已完成。

v1 hardening
  当前范围已完成。

v1 stable protocol
  wire / packet / frame layout 已对齐，可靠传输语义尚未完成。
```

本文档中的字段和行为是 v1 可靠传输继续推进的基础。代码如果和本文档不一致，应该优先判断是文档需要修正，还是代码出现了协议漂移。

## 协议定位

SRT v1 是一套面向串口类 byte stream 的 message-oriented transport。

核心目标：

```text
Application Message
  -> MESSAGE Frame fragments
  -> Packet
  -> Wire Envelope
  -> Serial Byte Stream
  -> ACK / dedup / retransmit
  -> reassemble complete Message
  -> deliver to application
```

SRT 借鉴 QUIC 的 packet number、ACK、去重、重传、channel/priority 思想，但不继承 QUIC 的对象模型。

SRT v1 不是：

- HTTP/3。
- QUIC over serial。
- TCP clone。
- 通用 byte-stream transport。
- 操作系统 runtime。
- MCU HAL。

SRT v1 的第一公民是：

```text
Message
```

不是：

```text
Stream
```

## 分层模型

SRT v1 分成三层：

```text
Wire Envelope
  串口 byte stream 外层边界。
  负责 magic、version、length、CRC、resync、半包、粘包。

Packet
  协议传输单元。
  负责 packet type、packet number、packet flags。

Protocol Frame
  Packet payload 内的语义单元。
  v1 stable draft 只定义 MESSAGE 和 ACK。
```

结构关系：

```text
Wire Envelope
└── Packet
    ├── Packet Header
    └── Packet Payload
        ├── MESSAGE Frame
        └── ACK Frame
```

## 字节序

SRT v1 draft 统一使用 little-endian。

原因：

- MCU 实现简单。
- 当前 Rust MVP 已经使用 little-endian。
- SRT 主要面向 MCU / robot / drone / local host link，不追求互联网协议传统网络字节序兼容。

所有多字节整数在 wire 上都使用 little-endian。

## 基础整数宽度

v1 draft 暂定：

```text
PacketNumber  u32
MessageId     u32
ChannelId     u16
MessageLen    u16
FragmentOffset u16
Flags         u8
Checksum      u16
```

说明：

- `PacketNumber(u32)` 足够 v1 长时间运行和测试。
- `MessageId(u32)` 按 channel 作用域递增。
- `ChannelId(u16)` 用于逻辑通道、QoS、可靠性策略和上层路由。
- `MessageLen(u16)` 使单条 message 最大长度在 v1 中天然受限，适合 MCU。
- `FragmentOffset(u16)` 与 `MessageLen(u16)` 对齐。
- `Flags(u8)` 当前足够，后续不够再扩展。

v1 不使用可变长度整数。

## Wire Envelope

Wire Envelope 是 SRT 在串口 byte stream 上的外层边界。

它不属于 Protocol Frame。

### Wire Envelope Header

v1 draft header：

```text
offset  size  field
0       2     magic
2       1     version
3       1     header_len
4       2     packet_len
6       1     wire_flags
7       1     reserved
```

固定 header 长度：

```text
WIRE_HEADER_LEN = 8
```

### Magic

v1 draft magic：

```text
"SR"
```

十六进制：

```text
0x53 0x52
```

`magic` 的作用是从连续 byte stream 中重新找到可能的 envelope 起点。

`magic` 不属于 Packet Header。

### Version

v1 draft：

```text
version = 1
```

不支持的 version 必须触发 resync 或 rejected envelope。

### Header Length

v1 draft：

```text
header_len = 8
```

保留 `header_len` 是为了未来扩展 wire envelope header。

v1 实现可以只接受 `header_len == 8`。

### Packet Length

`packet_len` 表示 encoded Packet bytes 的长度，不包含：

- wire header
- checksum

完整 envelope 长度：

```text
total_len = header_len + packet_len + checksum_len
```

v1 draft：

```text
checksum_len = 2
```

### Wire Flags

v1 draft：

```text
bit 0: checksum_present
bit 1..7: reserved
```

v1 必须设置：

```text
checksum_present = 1
```

未识别 reserved bits 的处理策略后续冻结。保守策略是 rejected + resync。

### Checksum

v1 draft 使用 `u16 checksum` 字段。

v1 draft 冻结为：

```text
CRC-16/XMODEM
```

参数：

```text
width   = 16
poly    = 0x1021
init    = 0x0000
xorout  = 0x0000
refin   = false
refout  = false
check   = 0x31c3  // "123456789"
```

选择原因：

- MCU 实现简单。
- no_std 实现不依赖查表也可接受。
- 当前 `srt-wire::Crc16` 已按该参数实现。

## Packet

Packet 是 SRT 的传输单元。

Packet 由 Packet Header 和 Packet Payload 组成。

```text
Packet
├── Packet Header
└── Packet Payload
```

### Packet Header

v1 draft packet header：

```text
offset  size  field
0       1     packet_type
1       1     packet_flags
2       4     packet_number
```

说明：

- `srt-engine` 当前已开始按这个 header layout 编码和解码。
- `packet_number` 必须属于 Packet Header，不属于 MESSAGE Frame。

### Packet Type

v1 draft：

```text
0x00 DATA
0x01 ACK
```

说明：

- DATA packet 可以携带 MESSAGE Frame。
- ACK packet 可以携带 ACK Frame。
- 后续是否允许一个 packet 携带多个 frame，留到 serialization freeze 时定稿。

### Packet Flags

v1 draft：

```text
bit 0: ack_eliciting
bit 1..7: reserved
```

DATA packet 默认应该是 ack-eliciting。

ACK-only packet 可以不是 ack-eliciting，避免 ACK 风暴。

### Packet Number

`PacketNumber(u32)` 是 packet 级可靠性的核心。

用途：

- ACK。
- duplicate packet detection。
- retransmit tracking。
- receive window。

Packet number 在 endpoint 内单调递增，允许 `u32` wrap，但 v1 stable 是否定义 wrap 后窗口行为需要后续单独冻结。

## Protocol Frames

v1 stable draft 只定义两个 frame：

```text
MESSAGE
ACK
```

不定义：

```text
PING
PONG
RESET_STREAM
CONNECTION_CLOSE
MAX_DATA
```

这些名字会把 SRT 拉回 QUIC/TCP 对象模型。SRT 后续如需要心跳、取消 message、关闭 channel，应按 SRT 自己的 message runtime 语义命名，例如：

```text
HEARTBEAT
CANCEL_MESSAGE
CLOSE_CHANNEL
```

## MESSAGE Frame

MESSAGE Frame 承载一条 application message 的一个 fragment。

v1 draft MESSAGE frame：

```text
offset  size  field
0       1     frame_type
1       2     channel_id
3       4     message_id
7       2     message_len
9       2     fragment_offset
11      1     message_flags
12      N     fragment_bytes
```

### Frame Type

v1 draft：

```text
0x00 MESSAGE
0x01 ACK
```

### ChannelId

`ChannelId(u16)` 是 message 的逻辑通道。

用途：

- 上层路由。
- QoS。
- reliability policy。
- priority。
- message reassembly namespace。

`ChannelId` 不是 QUIC stream。

它更接近：

```text
topic
lane
mailbox route
message class
```

v1 draft 保留：

```text
channel_id = 0
  control channel
```

其它静态和动态分配规则后续冻结。

### MessageId

`MessageId(u32)` 在 channel 内标识一条 message。

v1 draft：

```text
message key = channel_id + message_id
```

接收端使用 message key 定位 reassembly buffer。

### MessageLen

`MessageLen(u16)` 是完整 application message 的长度。

接收端必须等收到覆盖：

```text
[0, message_len)
```

的所有 fragment 后，才能交付完整 message。

### FragmentOffset

`FragmentOffset(u16)` 是当前 fragment 在完整 message bytes 中的起始位置。

合法性要求：

```text
fragment_offset <= message_len
fragment_offset + fragment_len <= message_len
```

不满足要求的 MESSAGE Frame 必须 rejected。

### Message Flags

v1 draft：

```text
bit 0: first
bit 1: last
bit 2..7: reserved
```

说明：

- `first` 表示该 fragment 从 message 起点开始。
- `last` 表示该 fragment 到达 message 末尾。
- 空 message 可以同时设置 `first` 和 `last`。

接收端不能只依赖 `last` 判断完整 message。必须检查所有 byte range 是否完整覆盖。

## ACK Frame

v1 draft 当前定义 fixed-capacity ACK range。

```text
offset  size  field
0       1     frame_type
1       4     largest_acknowledged
5       1     range_count
6       8*N   ack_ranges
```

当前 `N = MAX_ACK_RANGES`，每个 range 固定编码：

```text
offset  size  field
0       4     start_packet_number
4       4     end_packet_number
```

`range_count` 表示前多少个 range 有效。未使用的 range slot 必须编码为零值。

语义：

```text
start <= packet_number <= end
  -> acknowledged
```

当前代码中的 ACK Frame 长度是固定长度，目的是保持 no_std 解码简单。ACK range 生成使用固定容量滑动窗口：

```text
observed packet set full
  -> newer packet number replaces oldest packet number

range count > MAX_ACK_RANGES
  -> encode newest ranges first
```

这样长期运行时 ACK 记忆会向新的 packet number 推进，不会卡在早期 packet 上。

后续可以继续优化更紧凑的 ACK range wire encoding，或增加按时间 / packet distance 的过期策略，但不能破坏 v1 已冻结的基本 ACK range 语义。

## Fragmentation

SRT v1 使用 greedy fragmentation。

策略：

```text
fragment_len = min(max_fragment_bytes, remaining_message_bytes)
```

如果：

```text
max_fragment_bytes = 10
message_len = 11
```

则：

```text
fragment 0: 10 bytes
fragment 1: 1 byte
```

不会平均拆成：

```text
fragment 0: 6 bytes
fragment 1: 5 bytes
```

v1 当前默认：

```text
DEFAULT_FRAGMENT_BYTES = 32
```

这个值是 MCU / 串口友好的默认值，不来自 QUIC MTU。

正式默认值可在 wire format freeze 前再审核一次。

## Receive State Machine

对外 API：

```text
engine.receive(bytes)
```

必须非阻塞。

它可以接收任意串口读取片段：

- 空 bytes。
- 半个 envelope。
- 一个完整 envelope。
- 多个 envelope 粘在一起。
- noise + envelope。
- corrupted envelope + valid envelope。

`receive(bytes)` 的职责是推进当前输入，不等待未来 bytes。

完整 message 不应该通过阻塞 `receive` 返回，而应该通过：

```text
engine.poll_event()
```

产生 Message event。

## Duplicate Packet

当接收端观察到 duplicate DATA packet：

```text
duplicate packet
  -> should ACK again
  -> must not reapply MESSAGE fragment as new data
  -> must not deliver duplicate Message event
```

原因是对端可能没有收到之前的 ACK。

## Retransmit

v1 draft 保留 tick-driven retransmit：

```text
engine.tick(now)
```

当前 v1 行为：

```text
tick(now)
  -> 只重发达到 retransmit_timeout_ms 的 in-flight packet
  -> 已被 ACK range 覆盖的 packet 不再重发
  -> 达到 max_retransmit_attempts 后产生 SendFailed
  -> message 失败后移除本端同 message 的全部 in-flight packet
  -> 同一个 tick 内不会再重发已经失败的 message
```

v1 当前已经有这些配置：

```text
retransmit_timeout_ms
max_retransmit_attempts
```

这些不是 wire format 字段，但属于 v1 stable behavior。

## Partial Reliability

SRT 的长期目标不是所有 channel 都强可靠。

v1 draft 保留这些 reliability mode：

```text
Reliable
BestEffort
LatestOnly
Deadline
```

当前 stable draft 冻结的最小行为：

- Reliable：设置 `ACK_ELICITING`，进入 in-flight，需要 ACK，允许超时重传。
- BestEffort：不设置 `ACK_ELICITING`，不进入 in-flight，不重传，不等待 ACK；如果接收端收到完整 message，仍然正常交付。

当前 stable draft 只保留概念边界，尚未冻结完整算法：

- LatestOnly：旧 message 可以被新 message 替代。
- Deadline：超过时间窗口后停止重传。

正式算法应在 reliability policy 文档中单独冻结。

## Error / Reject 行为

v1 draft 暂定：

```text
bad magic
  -> noise / resync

unsupported version
  -> reject envelope / resync

unsupported header_len
  -> reject envelope / resync

checksum failed
  -> corrupted / resync

packet_len too large
  -> reject envelope / resync

malformed packet
  -> reject packet

malformed MESSAGE frame
  -> reject frame

duplicate DATA packet
  -> ACK but do not reapply
```

具体 `ReceiveReport` / `EngineOutput` 类型需要在代码对齐阶段和这个行为表一致。

## v1 不支持

v1 stable draft 不支持：

- QUIC stream。
- HTTP/3。
- TCP-compatible byte-stream API。
- PING / PONG Frame。
- RESET_STREAM。
- connection migration。
- congestion control。
- TLS。
- crypto handshake。
- dynamic MTU discovery。
- OS runtime adapter。
- MCU HAL adapter。
- multi-language SDK。

这些不是当前协议核心目标。

## 代码对齐清单

当前代码已按本文档检查：

1. `srt-core` 只暴露 MESSAGE / ACK frame。
2. `srt-core` 使用 `MessageFrame` / `ChannelId` 命名。
3. `srt-wire` 实现 envelope header、magic、version、length、CRC-16/XMODEM、resync。
4. `srt-engine` 使用正式 Packet Header + MESSAGE / ACK Frame layout。
5. Packet Header 明确编码 `packet_type`、`packet_flags`、`packet_number`。
6. MESSAGE Frame 明确编码 `frame_type`、`channel_id`、`message_id`、`message_len`、`fragment_offset`、`message_flags`。
7. ACK Frame 明确编码 `frame_type`、`largest_acknowledged`、`range_count`、fixed-capacity ACK ranges。
8. smoke 覆盖 half packet、sticky packet、CRC error、drop、ACK、retransmit、duplicate packet、simultaneous duplex、multi-channel、bidirectional message。

v1 仍必须完成的可靠传输部分：

1. ACK range 的更紧凑 wire encoding 和时间 / distance 过期策略是否需要冻结。
2. message 失败后的对端取消语义。
3. 更复杂持续收发场景下的窗口耗尽和恢复测试。
4. LatestOnly / Deadline 的实际决策。
5. heapless/no-alloc buffer 策略最终冻结。

详见 [SRT v1 可靠传输补齐计划](srt-reliable-transport-plan.md)。

## 结论

SRT v1 stable protocol 的核心不是“把 QUIC 搬到串口上”。

SRT v1 stable protocol 的核心目标是：

```text
Message-Oriented Transport over serial byte streams
```

用 packet number、ACK、dedup、retransmit、channel reliability 等思想，让完整 message 可以在 MCU / robot / drone 场景中可靠、实时、低成本地传输。
