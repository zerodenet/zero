# TODO — 架构改进

## 目标 vs 现状

- **目标：** `traits → core → protocols → config/router/engine → proxy`
- **现状：** 依赖方向正确，抽象边界逐步清晰

| 维度 | 初始 | 现在 | 变化 |
|------|:----:|:----:|:----:|
| proxy 行数 | 7318 | **5398** | -26.2% |
| proxy warning | ~14 | **0** | 清零 |
| VLESS transport（独立模块） | 0 | **1549** | 新增 |
| VLESS 协议层独立性 | no_std 可行 | **no_std 可行** | 保持 |
| VLESS reality 模式 | heavy deps | **全部 optional** | feature-gated |

---

## 已完成

### 1. Transport 实现放错位置 ✅

- 5 个传输文件（tls/ws/grpc/h2/quic）从 `crates/proxy/src/transport/` 移至 `protocols/vless/src/transport/`（1373 行）
- `vless_transport.rs` 统一分发函数移至 VLESS（106 行）
- proxy 通过 `transport/mod.rs` 按需重导出 `InboundTlsStream`、`QuicInbound`、`accept_grpc` 等入站类型
- `ClientStream` + `TcpRelayStream` 移至 `zero-platform-tokio`（正确的平台抽象层）
- `TransportConnector` trait 定义在 `zero-platform-tokio`，VLESS 实现 `VlessTransportConnector`
- proxy 通过 trait 调用：`connector.connect(socket, server, port)` 替代裸函数

### 2. Transport 分发去重 ✅

- 3 处重复 match（~130 行）→ 1 个 `build_vless_outbound_transport()` 函数
- 函数在 `protocols/vless/src/transport/vless_transport.rs`

### 3. InboundHandler / OutboundHandler 删除 ✅

- 方案 B：死代码直接删除
- `crates/core/src/handler.rs` 移除
- 3 个协议 crate 的 `impl InboundHandler` 移除
- 协议 crate 各自提供 public API

### 4. 协议类型归位 ✅

- `ConfiguredVlessUsers` adapter → `protocols/vless/src/inbound.rs`
- `upgrade_reality_server_from_config` → `protocols/vless/src/reality/stream.rs`
- `VlessUdpTransport` + `VlessUdpUpstream` → `protocols/vless/src/udp.rs`
- `MuxStreamRelay` / `MuxPoolConn` / `PoolKey` / `TransportKey` → `protocols/vless/src/mux_pool.rs`
- `encrypt_mux_payload` / `decrypt_mux_payload` → VLESS mux_pool

### 5. 传输 relay 去重 ✅

- `outbound/vless.rs` QUIC/TCP 两个分支的 60 行重复 relay spawn 抽取为 `spawn_vless_udp_relay()`

---

## 依赖合规性

### VLESS — no features（纯协议模式）

```
zero-protocol-vless
├── zero-core        # 协议类型
└── zero-traits      # I/O 抽象（no_std）
```

**独立可用。** 只做 VLESS 握手、MUX 帧编解码、UUID 解析、加密原语。

### VLESS — reality feature（传输模式）

```
zero-protocol-vless
├── zero-core
├── zero-traits
├── zero-platform-tokio   # optional
├── zero-config           # optional — 可进一步解耦
├── zero-engine           # optional — 可进一步解耦
├── h2, quinn, rustls...  # optional — 传输实现依赖
└── tokio                 # optional
```

所有重型依赖 feature-gated，不破坏协议层独立性。

### 已知矛盾

| 依赖 | 问题 | 修复方向 |
|------|------|------|
| `zero-engine` (EngineError) | 传输函数返回 EngineError | 改为 `io::Error`，EngineError: From<io::Error> 自动转换 |
| `zero-config` (ClientTlsConfig 等) | 传输函数参数用 config 类型 | 在 VLESS 中定义传输原生配置类型，proxy 侧做转换 |
| `zero-platform-tokio` (reality feature) | 平台抽象类型 | **合理** — 传输层天然需要平台 I/O 类型 |

---

## 剩余工作（渐进，无强制顺序）

### proxy 中仍含协议逻辑的模块

| 文件 | 行数 | 内容 | 归属问题 |
|------|:----:|------|------|
| `inbound/vless.rs` | 821 | VLESS listener + 会话处理 | listener 骨架属 proxy，MUX/UDP 会话处理是集成层 |
| `outbound/vless.rs` | 216 | VLESS UDP 出站管理 | `VlessUdpOutboundManager` 是纯协议逻辑但依赖 Proxy |
| `runtime/upstream.rs` | 177 | VLESS 出站连接 | `VlessUpstream` 参数结构是 VLESS 概念 |
| `runtime/mux_pool.rs` | 259 | MUX 池连接建立 | 核心已移出，剩余是 Proxy 依赖的连接工厂 |

这些是**集成层**而非协议逻辑——它们需要 Proxy 的 TCP 连接、DNS 解析、流量统计、会话生命周期管理。深入分离需要引入更多 trait 抽象（如连接工厂、会话回调），收益递减。

### 可做的小改进

- `zero-engine` 依赖移除：传输函数返回值从 `EngineError` 改为 `io::Error`
- `zero-config` 依赖移除：传输函数参数用 VLESS 原生类型替代 config 类型
- `groups/`（360 行）：出站组逻辑可并入 engine 或保持现状（规模小）

---

## 架构总览

```
zero (app)
 ├── config (配置)
 ├── engine (决策、状态、事件)
 │    ├── router (规则匹配)
 │    ├── core (协议类型)
 │    └── platform/tokio (I/O 抽象)
 ├── proxy (运行时／集成层)
 │    ├── inbound/outbound (协议集成)
 │    ├── groups (出站组)
 │    ├── runtime (连接池、会话)
 │    └── transport (TCP relay、metering)
 └── protocols/
      ├── socks5
      ├── http-connect
      └── vless
           ├── 纯协议：握手、MUX 帧、加密
           ├── transport/ (reality-gated)：5 种传输实现
           ├── mux_pool (reality-gated)：MUX 池核心类型
           └── udp (reality-gated)：UDP 出站类型
```
