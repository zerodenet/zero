# TODO — 架构改进

## 当前状态

依赖方向正确（`traits → core → protocols → config/router/engine → proxy`），但抽象层未随功能增长同步演化，导致 proxy crate 膨胀为 7318 行的单体。

---

## 架构问题

### 1. Transport 实现放错了位置

**现状：** gRPC / H2 / QUIC / TLS / WS 传输实现全部在 `crates/proxy/src/transport/` 下（~1416 行），但它们不依赖 proxy 的任何东西，是 VLESS 专有传输。

**目标：**
- 将这些传输移至 `protocols/vless/src/transport/` 下
- 在 `zero-traits` 或 `zero-core` 中定义统一的 `TransportConnector` trait
- proxy crate 只通过 trait 调用传输层，不直接依赖具体实现

**影响范围：** `protocols/vless/`, `crates/proxy/src/transport/`, `crates/proxy/src/runtime/upstream.rs`, `crates/proxy/src/outbound/vless.rs`

**工时估计：** 6-8h

---

### 2. Transport 分发逻辑重复三份

**现状：** 同一个 TLS/Reality/WS/gRPC/H2/QUIC 的 match 分发逻辑出现在三个地方：
- `runtime/upstream.rs` — TCP 出站（~40 行）
- `outbound/vless.rs` — UDP 出站（~60 行）
- `runtime/mux_pool.rs` — MUX 连接池（~30 行）

**目标：** 抽取为单一函数 `build_vless_transport(socket, config) -> impl AsyncSocket`，三处调用点改为使用该函数。

**影响范围：** 上述三个文件

**工时估计：** 2-3h（与问题 1 一起做）

---

### 3. InboundHandler / OutboundHandler trait 形同虚设

**现状：**
- `VlessInbound::handshake()` 永远返回 `Err("requires a user store")`，调用方绕过 trait 直接调用 `handshake_with_auth()`
- `OutboundHandler` trait 全代码库零实现，纯死代码

**目标（二选一）：**

方案 A — 修复 trait 使其可用：
- 将 `VlessUserStore` 作为泛型参数加入 `InboundHandler`
- 为 `VlessOutbound` 实现 `OutboundHandler`

方案 B — 删除 trait，接受 VLESS 协议需要额外参数的现实：
- 从 `zero-core` 中移除 `InboundHandler` 和 `OutboundHandler`
- 协议 crate 各自提供自己的 public API（当前实际做法）

**影响范围：** `zero-core/src/handler.rs`, `protocols/*/`

**工时估计：** 1-2h

---

### 4. Proxy crate 职责过重

**现状：** proxy crate 7318 行，混合了：
- 协议处理逻辑（UDP 会话管理、MUX 流分发）
- 传输层实现（6 种传输）
- 运行时管理（监听、连接生成、关闭）
- 出站组管理（urltest、selector）

**目标：**
- 问题 1 解决后，proxy 自然缩减 ~1400 行（传输层移出）
- 考虑将 `groups/` 出站组逻辑拆分到独立 crate 或并入 engine
- 考虑将 `runtime/upstream.rs` 中的 VLESS 特定连接逻辑移入 protocol crate

**工时估计：** 随问题 1、2 逐步推进

---

## 执行优先级

| 顺序 | 任务 | 工时 | 影响 |
|:----:|------|:----:|------|
| 1 | 修复 InboundHandler/OutboundHandler trait | 1-2h | 消除死代码，明确抽象边界 |
| 2 | 消除 Transport 分发重复 | 2-3h | 减少 ~100 行重复代码 |
| 3 | Transport 实现移至 protocol crate | 6-8h | proxy 缩减 ~1400 行 |
| 4 | Proxy 进一步瘦身 | 渐进 | 持续改进 |

建议 **1 → 2 → 3** 顺序执行，每步独立可验证。
