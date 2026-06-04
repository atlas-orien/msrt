# Endpoint

`endpoint` 是 engine 之上的连接生命周期管理层。它不是协议核心状态机，也不是 UART、UDP、TCP 或网卡 adapter。

可以把分层理解成：

```text
adapter
  负责真实 IO：UART read/write、UDP recv/send、raw ethernet frame 等

endpoint
  负责连接生命周期：创建 Engine、丢弃 Engine、判断连接状态

engine
  负责协议状态：send、receive、poll、ACK、重传、message reassembly
```

## 为什么需要 Endpoint

`Engine` 表示一个协议会话。断线重连以后，旧会话的状态不应该继续复用。

旧状态包括：

- ingress buffer
- event queue
- in-flight packet
- ACK tracker
- dedup table
- message reassembly table
- packet key
- message id

因此 MSRT 不提供 `engine.reset()` 或 `reset_ingress()` 这种局部清理 API。断开以后，endpoint 直接丢掉旧 `Engine`；下一次连接创建新的 `Engine`。

```text
disconnect:
  peer.engine = None

connect / accept:
  peer.engine = Some(Engine::new(config))
```

这个设计避免半清理，也让“一个 Engine = 一个会话状态”这个边界保持清楚。

## 连接状态

endpoint 维护的是连接状态，不是 packet 状态。

当前状态分成三类：

```text
Disconnected
  没有本地 Engine

Connecting
  已经创建本地 Engine，但还没有收到对方合法 packet

Connected
  已经收到对方合法 Data / Ack / Duplicate packet
```

`PeerSlot::has_session()` 表示本地是否有 `Engine`。

`PeerSlot::is_connected()` 表示双方是否已经确认联通。它不能简单等于 `engine.is_some()`。

## 联通确认

MSRT 当前不新增 `Ping` / `Pong` packet type。联通确认使用现有 `Data` + `Ack` 完成。

主动端 connect 时，endpoint 会创建新的 `Engine`，并发送一个很短的 hello message：

```text
ClientEndpoint::connect(now_ms)
  -> PeerSlot::connect(now_ms)
    -> Engine::new(config)
    -> engine.send([0])
    -> state = Connecting
```

之后外部通过 `poll` 拿到 hello 的 wire bytes 并写到底层链路。

只要本端收到对方任意合法 `Data`、`Ack` 或 `Duplicate` packet，就说明：

```text
本端 -> 对端 的发送路径可用
对端 -> 本端 的返回路径可用
```

endpoint 此时把状态切到 `Connected`，并刷新 `last_seen_ms`。

## 主动单 Peer：ClientEndpoint

`ClientEndpoint` 适合主动发起连接的一端。它内部只有一个 `PeerSlot`，最多一个 `Engine`。

典型流程：

```text
startup:
  endpoint = ClientEndpoint::new(config)

connect:
  endpoint.connect(now_ms)

main loop:
  收到底层 bytes:
    endpoint.receive(now_ms, bytes)

  需要发送业务 message:
    endpoint.peer_mut().send(message)

  推进状态:
    endpoint.poll(now_ms, tx_buf)
      -> Transmit(bytes): adapter 写到底层链路
      -> Message(message): 交付应用
      -> SendFailed(failed): 认为断开
      -> Idle

disconnect:
  endpoint.disconnect()
```

`connect()` 不是 socket connect。它只是在 endpoint 层创建一个新协议会话，并排队 hello message。

## 被动单 Peer：PassiveEndpoint

`PassiveEndpoint` 适合 MCU/UART 这类永远只面对一个对端、并且等待对方先发数据的场景。

它启动时不创建 `Engine`：

```text
startup:
  endpoint = PassiveEndpoint::new(config)
  state = Disconnected
  engine = None
```

收到第一批 bytes 时才创建新会话：

```text
receive(now_ms, bytes)
  -> 如果没有 Engine，先 Engine::new(config)
  -> engine.receive(bytes)
  -> 如果 bytes 内有合法 packet，state = Connected
```

断开以后同样丢掉旧 `Engine`，下一次收到 host 数据再重新创建。

这让 MCU 主循环保持简单：

```text
loop:
  UART 收到 bytes:
    endpoint.receive(now_ms, bytes)

  endpoint.poll(now_ms, tx_buf)
    -> Transmit(bytes): UART write
    -> Message(message): 交付应用
    -> SendFailed(failed): endpoint.disconnect()
    -> Idle

  endpoint.disconnect_if_idle(now_ms, timeout_ms)
```

## 多 Peer：ServerEndpoint

`ServerEndpoint<P, N>` 适合 UDP server 或其它多 peer adapter。它是固定容量的 peer table，不使用 heap，也不绑定 `std`。

```text
ServerEndpoint<P, N>
  P = adapter 定义的 peer id
  N = 最大 peer 数量
```

`P` 可以是：

- UDP remote address 的 wrapper
- UART port id
- adapter 分配的连接编号
- 测试环境里的简单整数

server 启动时不预创建 `Engine`：

```text
startup:
  peers = [None, None, None, ...]
```

adapter 发现某个 peer 后，再让 endpoint 为这个 peer 创建或取得 `Engine`：

```text
收到 peer_id 的 bytes:
  endpoint.engine_or_accept(peer_id, now_ms)
  endpoint.receive(peer_id, now_ms, bytes)
```

这类似 TCP 的 listen / accept 思想：

```text
ServerEndpoint
  类似 listen manager

ServerEndpoint::accept(peer_id, now_ms)
  类似 accept 一个具体连接

PeerSlot + Engine
  类似 accept 后得到的连接状态
```

但 endpoint 不打开 socket，也不监听端口。真正的 listen、recv、send 都属于 adapter。

## 断开判断

endpoint 可以通过两类信息判断断开：

```text
SendFailed
  reliable packet 达到重传上限，说明当前会话不可继续信任

idle timeout
  now_ms - last_seen_ms >= timeout_ms
```

如果链路完全空闲，并且不发送任何业务数据或 hello/control message，就不可能马上知道对方已经断开。协议没有信息来源。

因此当前策略是：

```text
有数据时:
  通过 Ack / SendFailed 判断连接是否正常

空闲时:
  通过 endpoint 的 last_seen_ms 超时判断
```

后续如果需要更主动的空闲探测，可以在 endpoint 或更上层用普通 `Data` message 实现 heartbeat，而不需要立刻新增 packet type。

## 不属于 Endpoint 的职责

endpoint 不负责：

- UART 寄存器读写
- DMA buffer 管理
- UDP socket bind / recv / send
- TCP listen / accept 系统调用
- raw ethernet frame 收发
- 应用层消息格式
- 业务登录、鉴权或 session token

这些都属于 adapter 或应用层。

endpoint 只做一件事：把一个或多个 peer 映射到正确生命周期的 `Engine`，并维护连接状态。
