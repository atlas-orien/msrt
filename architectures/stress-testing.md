# 压力测试与稳定性修复记录

这份文档记录 MSRT 在高压噪音测试中暴露的问题、排查过程和最终修复点。

它不是 API 说明，也不是实现计划，而是一次重要调试过程的复盘。以后如果再次出现
`send_failed`、ACK 收不到、双边压力下断线等问题，应该先回到这里对照。

## 测试目标

这轮测试的目标不是模拟普通网络，而是验证 MSRT 在裸字节链路上的极限恢复能力。

重点场景：

- 双边同时发送。
- application message 自动拆成多个 packet。
- DATA 和 ACK 都会被干扰。
- 接收端按 UART 思想处理连续 byte stream。
- wire 层需要自己完成 magic 重同步、length 校验和 CRC 校验。
- engine 需要维持 ACK、重传、去重、reassembly、in-flight 状态。

因此这个测试比常规 TCP/UDP 丢包测试更狠。TCP/UDP 底层通常是整帧通过 CRC 后交给协议栈，要么整包到，要么整包丢。MSRT 面向 UART 这类裸字节流，可能遇到字节错、字节丢、字节插入，甚至连续一段字节丢失。

## 初始失败现象

在双边高频发送和噪音测试下，最早的失败表现是：

```text
msrt in_flight send_failed ... attempts=10 retry_limit=10
host disconnect
```

表面看像是：

- in-flight 太小。
- poll 一次只吐一个 packet 太慢。
- ACK 被数据包挤压。
- 双边发送压力太大。
- UDP/bin 测试程序调度不正确。

这些方向都排查过，也做过调整，但只能改善现象，不能解释根因。

关键日志最后显示：

```text
host/mcu 已经收到重传 DATA
对端也发送了 ACK
发送端也收到 ACK
但旧的 failed packet 仍然没有从 in-flight 清掉
```

这说明问题不在“包没有到”，也不在“完全没有 ACK”，而在 ACK 内容没有覆盖那个老的重传 packet。

## 真正根因：ACK 语义错误

原来的 ACK 状态更像一个“最近见过的 packet number 集合”：

```text
AckRanges 保存最近 N 个 packet number
build_ack() 从这个集合生成 ACK ranges
```

这在低压下能工作，但在高压双边发送时有致命问题：

```text
1. 老 packet 重传到达接收端
2. 接收端确实 observe 了它
3. 随后大量新 packet 到达
4. 小容量 ACK ring 被新 packet 覆盖
5. ACK 发出去时 largest 很新，但 range 不包含老 packet
6. 发送端收到 ACK，却无法清掉老 packet
7. 老 packet 到 retry limit，触发 SendFailed
```

也就是说，ACK 最大号很新并不代表它确认了所有旧包。ACK range 如果不包含那个旧 packet，发送端不能清掉它。

这个问题和 `MAX_IN_FLIGHT_PACKETS` 没有直接关系。日志里失败时 in-flight 通常没有满，增大 in-flight 只是掩盖问题。

## 修复方向

ACK 的语义被重新定义为：

```text
ACK 只确认当前这批尚未发出去的 pending packet number
ACK 发出去以后清空 pending 集合
重复 DATA 到达时重新 observe，并再次 ACK
```

也就是：

```text
receive(DATA packet)
  -> 先 observe packet_number 到 AckState
  -> 再做 duplicate 判断
  -> duplicate 也必须重新 ACK

poll()
  -> 如果 ack pending，最高优先级发送 ACK
  -> ACK bytes 发出后 on_ack_sent()
  -> 清空 pending ACK ranges
```

这个修复点很小，但非常关键。

修复以后，重复 DATA 不再因为“已经见过”而失去 ACK 机会；老重传包刚到达时，会进入新的 pending ACK 集合，不会被之前的全局历史污染。

## 调度顺序

这轮测试也确认了调度顺序很重要，但它不是最终根因。

当前 engine 的输出优先级应该保持：

```text
1. ACK
2. control / pong
3. retransmit
4. local event，例如 SendFailed
5. complete Message
6. new DATA
```

ACK 不能作为普通 data queue 里的一个普通 write event 长时间排队。ACK 是协议恢复的核心，必须在 `poll` 时由 `AckState` 直接优先生成。

重传也必须高于新 DATA。新业务消息可以慢一点，ACK 和重传不能被新 DATA 压住。

## 去重结论

发送队列需要对相同 packet number 的 DATA/retransmit 做去重，避免同一个 packet 在队列里堆出多份。

但 ACK 不能按“同一个 ACK 已经发过”这种思路去重。ACK 是不可靠的，对方可能没有收到。重复 DATA 到来时再次 ACK 是必要行为。

最终原则：

```text
DATA/retransmit 队列可以按 packet number 去重
ACK pending 只表达当前需要确认的 packet set
重复 DATA 必须重新生成 ACK
```

## 噪音模型

压力测试最开始只有三种基本噪音：

- `corrupt`：随机改坏一个字节。
- `drop-byte`：随机丢一个字节。
- `insert-byte`：随机插入一个字节。

后续增加了更接近真实硬件问题的噪音：

- `burst-corrupt`：连续一段字节被改坏。
- `burst-drop`：连续一段字节丢失。
- `packet-drop`：整个 packet 丢失。
- 随机 chunk receive：一次 receive 传入 1 到 16 字节，覆盖 DMA buffer、半包、粘包场景。

其中 `burst-drop` 是最危险的噪音。

原因是 MSRT wire 使用 length-based framing：

```text
magic + length + length_crc8 + body + crc16
```

`length_crc8` 能保证 length 字段本身可靠，避免坏 length 导致等待超大包。但如果 body 中间连续丢字节，decoder 仍然会按照原 length 继续收后续字节，于是当前坏包会吞掉后面真实 packet 的一部分。这样一次 `burst-drop` 可能浪费不止一个 packet 的有效数据。

这不是 bug，而是 length framing 的自然代价。相比 tail magic，length framing 正常效率更高，不需要扫描 tail，也不需要处理 payload 中出现 tail magic 的转义问题。

## 单因子噪音结论

为了判断到底哪类噪音能打穿协议，测试改成单因子：

```text
一次只打开一种噪音
其他噪音全部为 0
```

在 `interval=1ms`、`message_len=240`、每类单独 `10%`、模拟 `600s` 的测试里：

```text
corrupt 10%        通过
drop-byte 10%      通过
insert-byte 10%    通过
burst-corrupt 10%  通过
packet-drop 10%    通过
burst-drop 10%     失败
```

这说明当前最容易打穿协议的是连续丢字节，而不是整包丢失。

这个结果符合直觉：

- `packet-drop` 边界干净，只是整个 packet 没了，重传即可。
- `corrupt` 和 `burst-corrupt` 会导致 CRC 失败，但包长度仍然可控。
- `insert-byte` 通常会变成 CRC 失败或噪声，后续还能重同步。
- `drop-byte` 会浪费后续字节。
- `burst-drop` 会连续浪费更多后续字节，是最危险情况。

## 极限测试结论

`interval=1ms`、`message_len=240` 时，一条 message 大约拆成 5 个 DATA packet。双边同时发送时，理论压力约为：

```text
2 sides * 1000 message/s * 5 packet/message
= 10000 DATA packet/s
```

这不是普通 UART 负载，而是极限压力模型。

在 ACK 修复前，双边 `1/1/1` 都可能很快断开。

ACK 修复后：

- `1/1/1` 稳定性显著提升。
- `3/3/3` 在高压下能跑很久。
- 更高组合噪音或加入 burst-drop 后，仍可能触发 retry limit。

当日志出现：

```text
attempts=10
retry_limit=10
```

并且前面有大量 corrupted/drop/insert/burst-drop 统计时，通常不是 ACK 语义 bug，而是当前恢复预算被概率性打穿。

如果要让这类极限组合继续不失败，应调整恢复预算，而不是继续修改 ACK 语义：

- 提高 `max_retransmit_attempts`
- 或使用动态 RTO / backoff
- 或对 ACK 做更强保护
- 或降低应用发送压力

## 当前判断

这轮测试修复后，MSRT 的核心可靠传输闭环已经被验证：

```text
receive DATA
observe ACK
duplicate DATA re-ACK
poll ACK first
receive ACK
clear in-flight
timeout retransmit
retry limit report failure
```

之前的快速失败已经不是当前主要问题。现在能打穿协议的主要是极端连续丢字节，尤其是 `burst-drop`。

因此当前结论是：

```text
协议核心方向正确
ACK 语义已经修正
普通 corrupt/drop/insert/packet-drop 都能被可靠恢复
burst-drop 是当前最强破坏模型
```

后续如果继续提高极限稳定性，优先研究 burst-drop 下的恢复策略，而不是回头改 packet header 或 ACK pending 语义。
