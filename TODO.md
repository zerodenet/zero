# TODO — 架构改进

## 目标 vs 现状

- **目标：** `traits → core → protocols → config/router/engine → proxy`
- **现状：** 依赖方向正确，抽象边界清晰

| 维度 | 初始 | 现在 | 变化 |
|------|:----:|:----:|:----:|
| engine src/engine/ 嵌套 | 有 | **无** | 扁平化 |
| VLESS protocol crate (no_std) | — | **pure** | 零 engine/config 依赖 |
| Hysteria2 protocol crate (no_std) | — | **pure** | 零 engine/config 依赖 |
| Transport 独立 crate | 0 | **1** (zero-transport) | 新增 |
| Transport 分发去重 | 3 处 | **1 处** | -67% |

---

## 已完成

### 1. Transport 归位 ✅

- VLESS: 5 个传输实现 → `protocols/vless/src/transport/`
- Hysteria2: `stream.rs` → `crates/transport/src/hysteria2_quic.rs`
- 统一的 `TransportConnector` trait 在 `zero-platform-tokio`

### 2. 依赖隔离 ✅

- `zero-engine`: 已从 VLESS/Hysteria2 协议 crate 移除
- `zero-config`: 已从 VLESS/Hysteria2 协议 crate 移除
- 协议 crate 纯 `zero-core` + `zero-traits`，no_std 可用

### 3. 死代码清理 ✅

- `InboundHandler` / `OutboundHandler` trait 删除
- 协议类型归位（ConfiguredVlessUsers, VlessUdpTransport, MuxPoolConn...）
- 传输 relay 去重（spawn_vless_udp_relay）

### 4. 独立 transport crate ✅

- `crates/transport/` — Hysteria2Connector, Hysteria2Stream 的归宿
- 不依赖 proxy，不依赖 engine

### 5. Engine 模块扁平化 ✅

- `crates/engine/src/engine/` → `crates/engine/src/`
- 删除 `mod engine` 中间层，lib.rs 直接声明所有子模块
- 全项目唯一有 `src/<crate-name>/` 嵌套的 crate 已修复

---

## 剩余工作（低优先级，渐进推进）

### 协议 handler 统一 — `ProtocolAcceptor` trait

**问题：** 所有协议的"路由 → 出站 → relay"后半段完全相同时，6 个协议 handler 各自独立实现 relay 逻辑（Trojan/SS/H2 各自写了 `io::copy` / `io::split`）。加一个新字段（如 `up_bps`/`down_bps`）需要在每一条 relay 路径上分别处理。

**目标：**

```rust
trait ProtocolAcceptor {
    async fn accept(&self, stream) -> (Session, ClientStream);
}

async fn serve_connection(stream, acceptor: impl ProtocolAcceptor, engine) {
    let (session, client) = acceptor.accept(stream).await;
    // session 上已有 target, auth, rate_limits
    engine.prepare_session(&mut session);
    let action = engine.route_decision(&session);
    let upstream = engine.establish_outbound(&session, action);
    engine.relay(client, upstream, session.rate_limits);
}
```

**难点：** Shadowsocks 的 AEAD relay 和 Hysteria2 的 QUIC stream relay 不是标准 TCP relay，统一需要抽象 `BidirectionalRelay` trait 或 adapter。

**收益：** 加新协议只需实现 `accept`；加新字段（限速、连接计数等）改一次 `serve_connection` 即全局生效。

**规模：** ~600 行的重构，跨 ~5 个文件。建议在加下一个协议之前做。

### 限速配置架构 — SessionAuth.apply_auth

**现状：** `Session::apply_auth(sa)` 是唯一注入点，所有协议的 `principal_key`、`up_bps`、`down_bps` 在此汇合。`SessionAuth` 是`配置 → Session` 的载体。以后加用户级配置字段，只需改 `SessionAuth` 和 `apply_auth`。

### groups/ 并入 engine（389 行）

`crates/proxy/src/groups/urltest.rs` 逻辑可移入 engine：
- urltest 延迟探测 + 出站选择本质是路由决策，属于 engine 职责
- 但需要 engine 访问 TCP 连接能力（connect_host），会引入循环依赖
- **建议：** 保持现状。规模小（389 行），改动成本高于收益

### proxy 中集成层（1387 行）

| 文件 | 行数 | 说明 |
|------|:----:|------|
| `inbound/vless.rs` | 821 | listener + session dispatch |
| `outbound/vless.rs` | 216 | UDP outbound management |
| `runtime/upstream.rs` | 211 | upstream connection routing |
| `runtime/mux_pool.rs` | 139 | MUX pool connection factory |

这些是**集成层**，依赖 Proxy 的 TCP 连接、DNS、流量统计、会话管理。
继续分离需要 trait 抽象层（连接工厂、会话回调），**收益递减**。

## 架构总览

```
zero (app)
 ├── config (配置)
 ├── engine (决策、状态、事件)
 │    ├── router (规则匹配)
 │    └── core (协议类型)
 ├── proxy (运行时／集成层)
 │    ├── inbound/outbound (协议集成)
 │    ├── groups (出站组)
 │    ├── runtime (连接池、会话)
 │    └── transport (TCP relay、metering)
 ├── transport (传输实现)
 │    └── hysteria2_quic
 └── protocols/
      ├── socks5      (zero-core + zero-traits only)
      ├── http-connect (zero-core + zero-traits only)
      ├── vless        (zero-core + zero-traits only)
      └── hysteria2    (zero-core + zero-traits only)
```
