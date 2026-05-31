# SRT v1 Hardening

## 状态定义

本文档记录的是 v1 hardening 阶段的设计和验收。当前项目状态已经越过 hardening，形成 v1 frozen baseline。

当前状态应该分成四层理解：

```text
v1 MVP
  已完成。

v1 hardening
  当前范围已完成。

v1 reliable transport
  当前范围已完成。

v1 protocol spec
  frozen baseline。
```

因此，本文档不再代表最新收关状态；最新结论以 [v1 protocol spec](srt-v1-spec.md) 和 [v1 reliable transport plan](srt-reliable-transport-plan.md) 为准。

`hardening` 的意思是加固。它不是重新设计方向，也不是引入新的运行环境适配层，而是在 v1 MVP 已经跑通的基础上，把协议推进到更接近真实串口使用的状态。

## 为什么需要 Hardening

v1 MVP 已经验证：

- `send(message)` 一次提交完整 message。
- engine 自动进行 greedy fragmentation。
- `poll_event()` 输出待写 wire bytes。
- `receive(bytes)` 推进接收状态。
- `tick(now)` 可以触发未 ACK packet 重发。
- 最小 ACK、in-flight tracking、CRC 检测、noise 检测、message reassembly 已经跑通。
- smoke simulation 可以覆盖噪声、CRC 错误、丢包、重发、ACK 和双向通信。

v1 MVP 曾经假设 `receive(bytes)` 看到的是一个完整 wire packet。

真实串口不是这样，因此 hardening 已经补齐 streaming wire decode。

真实串口输入可能是：

```text
第 1 次 read: 只读到 magic 的一半
第 2 次 read: 读到剩余 header
第 3 次 read: 读到 packet body 一半
第 4 次 read: 读到 packet body 另一半和下一个 packet 的一部分
```

这就是 hardening 首先要解决的问题。

## Hardening 的 Crate 归属

hardening 是阶段，不是一个新的 crate。

不要新增 `srt-hardening` crate。

v1 hardening 应该按问题归属写进现有 crate：

```text
srt-wire
  负责 byte stream 边界。

srt-reliability
  负责可靠性策略和判断。

srt-engine
  负责组织 wire、reliability、message reassembly 和 event 输出。

srt
  负责对外 facade API，不放协议细节。
```

### srt-wire 负责什么

`srt-wire` 负责把串口 byte stream 转换成完整 packet bytes。

这些工作属于 `srt-wire`：

- magic scan。
- length decode。
- CRC verify。
- half packet。
- sticky packet。
- multiple packets per receive。
- noise before magic。
- CRC error 后 resync。
- channeling decoder state。

原因是这些问题都发生在 `raw bytes -> complete wire envelope` 这一层。

它不应该关心：

- ACK 是否需要发送。
- packet 是否重复。
- message 是否已经完整。
- 是否需要重传。
- channel_id 怎么路由。

### srt-reliability 负责什么

`srt-reliability` 负责可靠性策略和判断。

这些工作属于 `srt-reliability`：

- ACK policy。
- ACK range。
- duplicate packet detection。
- sliding window。
- timeout policy。
- retry limit。
- partial reliability。
- latest-only policy。
- send failed decision。

原因是这些问题都属于“一个 packet / message 在可靠性规则下应该如何处理”。

它不应该关心：

- magic。
- length。
- CRC。
- half packet。
- sticky packet。
- UART / DMA / OS IO。

### srt-engine 负责什么

`srt-engine` 是协议状态机。

这些工作属于 `srt-engine`：

- 调用 `srt-wire` decoder 处理 `receive(bytes)`。
- 将完整 packet 交给 packet/frame/message 处理逻辑。
- 根据 `srt-reliability` 判断 ACK、drop、retransmit、failed。
- 管理 in-flight packets。
- 管理 message reassembly buffers。
- 产生 `Event::Write` / `Event::Message` 等输出。
- 在 `tick(now)` 中推进 timeout / retransmit。

原因是 engine 是组合层。它不应该重新实现 wire scan，也不应该把 reliability policy 写死在 byte parser 里面。

### srt 负责什么

`srt` 是 facade crate。

这些工作属于 `srt`：

- 暴露 `srt::Engine`。
- 暴露 `srt::Config`。
- 暴露 `srt::Event`。
- 暴露 `srt::Message`、`srt::Write`、`srt::Receive`。
- 让外部用户不用直接理解 workspace 内部 crate。

它不应该实现：

- wire decoder。
- ACK policy。
- retransmit policy。
- message reassembly。
- runtime adapter。

### 不属于本仓库的内容

以下内容不属于当前基础协议库：

- `srt-embassy`
- `srt-rtic`
- `srt-tokio`
- `srt-std`
- CLI
- 多语言 SDK

这些应该是独立 wrapper / adapter 项目。

## Hardening 第一优先级

第一优先级是 streaming wire decode。

目标是让：

```text
engine.receive(bytes)
```

可以接收任意长度、任意边界的串口 bytes，而不是要求调用方先切出完整 packet。

需要覆盖：

- half packet：半包。
- sticky packet：粘包。
- multiple packets per receive：一次 receive 多个 packet。
- noise before magic：magic 前有干扰字节。
- corrupted packet：CRC 错误后继续寻找下一个 magic。
- resync：错误后重新同步。

## 目标 API 不变

hardening 不应该改变 v1 MVP 的用户心智模型：

```text
engine.send(message)
engine.receive(bytes)
engine.tick(now)
engine.poll_event()
```

调用方仍然不需要知道：

- packet 多长。
- 一条 message 分成几个 packet。
- 当前 bytes 是否刚好是完整 packet。
- ACK 什么时候发。
- 哪个 packet 需要重发。

这些仍然由 engine 内部处理。

## 不属于 Hardening 的内容

v1 hardening 不应该引入：

- Embassy adapter。
- RTIC adapter。
- Tokio adapter。
- std serialport adapter。
- CLI。
- 多语言 SDK。
- OS 线程或 async task。

这些属于运行环境包装层，不属于本基础协议库。

## Hardening 工作项

当前完成状态：

### 第一批：srt-wire

1. 在 `srt-wire` 中实现 streaming decoder 状态机。
2. 让 decoder 支持 half packet。
3. 让 decoder 支持 sticky packet。
4. 让 decoder 支持 multiple packets per receive。
5. 让 decoder 在 noise / CRC error 后能 resync。
6. 为 decoder 增加 focused unit tests。

第一批完成后，`srt-wire` 已经可以把任意输入 bytes 转换成：

```text
Decoded complete packet bytes
Need more bytes
Noise skipped
CRC error
```

### 第二批：srt-engine

1. 将 `srt-engine::receive(bytes)` 改成使用 `srt-wire` channeling decoder。
2. engine 内部循环处理 decoder 产出的多个 packet。
3. 更新 smoke simulation，模拟半包、粘包、一次多包、噪声和 CRC 错误。
4. 清理 engine 内部旧 wire parsing 代码。
5. 明确 engine 只处理 complete packet，不再自己扫描 magic。

第二批完成后，外部用户仍然只调用：

```text
engine.receive(bytes)
```

但 `bytes` 可以是任意串口读取片段。

### 第三批：srt-reliability

v1 hardening 先完成最小可靠性工具，而不是完整可靠性算法。

已进入 v1 hardening 范围的内容：

1. 增加 fixed-capacity duplicate packet detection。
2. 增加 fixed-capacity ACK tracker。
3. 增加 retry-limit retransmit policy。

仍然留到后续协议冻结前继续设计的内容：

1. ACK range。
2. retransmit timeout policy 的完整时间模型。
3. send failed event。
4. partial reliability / latest-only policy 的实际决策。

第三批完成后，当前 MVP 的简单 ACK / retransmit 逻辑已经具备明确边界，但不会在 v1 hardening 阶段提前实现完整算法。

### 第四批：协议冻结

1. 冻结第一版 wire format spec。
2. 清理 MVP 早期 packet layout。
3. 把 Packet / Frame serialization 边界写清楚。
4. 更新架构文档和 README。

已完成。

多 message / 多 channel reassembly 属于后续可靠性和 channel 行为加深阶段，不再作为 v1 hardening 收关阻塞项。

## Hardening 验收标准

v1 hardening 至少应该满足：

- `cargo check --workspace` 通过。
- `cargo test --workspace` 通过。
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` 通过。
- smoke simulation 覆盖：
  - 正常双向通信。
  - 半包。
  - 粘包。
  - 一次 receive 多包。
  - noise。
  - CRC 错误。
  - 丢包。
  - ACK。
  - tick-driven retransmit。
- 文档明确 wire format spec。
- 用户 API 仍然保持 no_std 状态机模型。

## 结论

v1 MVP 证明了 SRT 的方向是可行的。

v1 hardening 已经证明 SRT 可以面对真实串口 byte stream。

当前只能说 v1 foundation 和 hardening 当前范围已经完成。后续仍然属于 v1：需要进入可靠传输补齐阶段，而不是直接进入 v2。
