# VLESS 缺口审计与实施方案

日期: 2026-06-15 | 基线: `xray_core_vless` (Xray-core v25.3.1)

---

## 缺口总览

| # | 限制码 | 严重度 | 类型 | 预估工作量 |
|---|--------|--------|------|-----------|
| 1 | `mux_udp_is_not_implemented` | 高 | 功能缺失 | 3–5 天 |
| 2 | `non_reality_tls_fingerprint_passthrough_is_incomplete` | 中 | 部分实现 | 1–2 天 |
| 3 | `udp_relay_chain_final_transport_limited` | 低 | 架构限制 | 记录，不详述 |

---

## 缺口 1: `mux_udp_is_not_implemented`

### 现状

**入站 (inbound) 已实现。** `crates/proxy/src/inbound/vless.rs:641-656` -- 当 MUX server 收到 `NETWORK_UDP` 新流请求时，已按 `spawn_vless_mux_udp_stream_task()` 创建完整 `UdpDispatch`，对每个 UDP MUX 数据帧解析目标地址并路由。入站 MUX UDP 是**功能完备**的。

**出站 (outbound) 缺失。** `crates/proxy/src/runtime/mux_pool.rs:128` 的 `open_stream()` 硬编码了 `NETWORK_TCP`：

```rust
// mux_pool.rs:128
let req = vless::encode_new_stream(vless::NETWORK_TCP, session.port, &session.target)
```

这导致出站侧无法通过 MUX 连接发送 UDP 数据。当前 VLESS UDP 出站不使用 MUX 池 — 它总是通过 `VlessUdpOutboundManager` (`vless_udp.rs:149`) 打开**独立的 VLESS 连接**：

```
当前 TCP 出站 (Vision flow):
  Proxy → MuxPool.open_stream(NETWORK_TCP) → MUX 子流 → 中继

当前 UDP 出站:
  Proxy → establish_vless_udp_upstream() → 独立 VLESS 连接 → send_udp_request → 中继
```

### Xray-core 参考行为 (Mux.Cool)

Xray-core 的 Mux.Cool 对 UDP 的支持：
1. 客户端发送 `STATUS_NEW` 帧时 `network = NETWORK_UDP`
2. 后续 `STATUS_KEEP` 数据帧的 payload 格式为 `[network:1][port:2][atyp:1][address…][data…]`（与 `encode_udp_data_frame()` 一致）
3. 服务器端对每个入站 UDP 帧按目标地址独立路由
4. 出站侧 MUX UDP 子流是**连接无关**的 — 同一个 MUX UDP 子流可向多个目标发送（每帧自带目标地址）

### 在 Zero 中的实现路径

Zero 已有全部需要的构建块：

| 块 | 位置 | 状态 |
|----|------|------|
| UDP MUX 帧编码 | `protocols/vless/src/mux.rs:204` — `encode_udp_data_frame()` | ✅ 已有 |
| UDP target 解析 | `protocols/vless/src/mux.rs:231` — `parse_udp_target_from_keep()` | ✅ 已有 |
| MUX 入站 UDP 处理 | `crates/proxy/src/inbound/vless.rs:641-656` | ✅ 已有 |
| MUX 池连接管理 | `crates/proxy/src/runtime/mux_pool.rs` | ✅ 已有，仅缺 UDP |
| VLESS UDP 出站流水线 | `crates/proxy/src/runtime/vless_udp.rs` — `VlessUdpOutboundManager` | ✅ 已有 |

**需要新增的部分：**

1. **`MuxConnectionPool::open_udp_stream()`** — 类比现有 `open_stream()`，发送 `NETWORK_UDP` 新流请求
2. **将 VLESS UDP 出站接入 MUX 池** — 当 `flow = "xtls-rprx-vision"` 且 MUX 已建立时，复用 MUX 连接发送 UDP 而不是打开独立 VLESS 连接
3. **UDP MUX 子流的出站帧发送** — 使用 `encode_udp_data_frame()` 编码，每帧携带目标地址

**实现涉及的调用链：**

```
当前调用链 (独立 VLESS UDP):
  UdpDispatch::start_flow()
    → ResolvedLeafOutbound::Vless { ... }
    → self.vless_manager.get_or_create_upstream()
      → establish_vless_udp_upstream()
        → 拨号新 TCP 连接
        → send_udp_request() (VLESS 0x02 命令)
        → forward 数据

新调用链 (MUX UDP — 仅 Vision flow 启用):
  UdpDispatch::start_flow()
    → ResolvedLeafOutbound::Vless { flow: Some("xtls-rprx-vision"), ... }
    → 检查 mux_pool 是否已有连接
    → self.vless_manager.get_or_create_mux_udp_stream()
      → mux_pool.open_udp_stream()
        → encode_new_stream(NETWORK_UDP, ...)
      → 通过 MUX 子流发送 UDP 数据帧
```

### 阻塞点 / 风险

- **MUX 池的 `open_stream()` 当前假设每个子流是一个 TCP 中继 (TcpRelayStream)**。UDP MUX 子流需要不同的抽象 — 它是帧级而非流级
- **UDP 出站的 `VlessUdpOutboundManager` 当前管理独立的 VLESS 连接**，引入 MUX 需要改变其上游创建逻辑
- **与现有 `flow = "xtls-rprx-vision"` 的交互** — Vision flow + MUX 是生产配置中最常用的组合，改动需要保证不破坏现有 TCP MUX 行为

---

## 缺口 2: `non_reality_tls_fingerprint_passthrough_is_incomplete`

### 现状

对比 Trojan 和 VLESS 的 TLS 指纹传递路径：

**Trojan (✅ 完整):**
```
tcp_dispatch.rs:284 → client_fingerprint
  → connect_via_trojan_upstream()
    → ClientTlsConfig { client_fingerprint: ... }   ← upstream.rs:495
    → connect_tls_upstream(tls_config)
      → tls.rs:135 lookup_fingerprint(tls.client_fingerprint)
```

**VLESS (⚠️ 部分):**
```
tcp_dispatch.rs:154 → VlessUpstream { tls: ClientTlsConfig }
  (tls 配置自带 client_fingerprint 字段)
  → connect_via_vless_upstream()
    → VlessTransportConnector::new(tls, ...)    ← upstream.rs:164
    → build_vless_outbound_transport(socket, self.tls, ...)
      → connect_tls_upstream(socket, tls, ...)  ← tls 配置原样传入
        → tls.rs:135 lookup_fingerprint()       ← 已读取
```

**结论：单跳 VLESS TLS 出站的指纹路径在代码层面是完整的** — `ClientTlsConfig` 自带的 `client_fingerprint` 字段随 `tls` 参数原样传入 `connect_tls_upstream()`，后者确实查询并应用了指纹。

**但存在以下差异：**

1. **VLESS `ResolvedLeafOutbound` 不受 `client_fingerprint` 影响路由键** — 相比之下 Trojan 将 `client_fingerprint` 作为 outbound 解析键的一部分。这意味着如果配置中有两个 VLESS 出站仅在 `tls.client_fingerprint` 上不同，引擎可能无法区分它们。

2. **VLESS Reality 路径使用独立的 TLS 栈 (`ztls`)** — `reality_client_connection.rs` 中的 fingerprint 行为由 Reality 栈内部控制，与 `ClientTlsConfig.client_fingerprint` 字段无关。

3. **`VlessTransportConnector` 没有独立的 `client_fingerprint` 字段** — 它与 Trojan 等其他出站不同，依赖 `ClientTlsConfig` 携带而非显式参数，使得调用方无法在运行时覆盖。

**Xray-core 参考：** Xray-core 中 VLESS 的 TLS fingerprint 由 `tlsSettings.fingerprint` 配置，直接控制 uTLS 的 ClientHello 指纹选择，与传输类型（tcp/tls/reality）无关。

### 实现路径

**方案 A (最低改动):** 确认现有路径的指纹生效，不做代码改动，仅更新文档说明前提条件（已由 `ClientTlsConfig.client_fingerprint` 控制）。

**方案 B (推荐):** 给 `VlessUpstream` 增加显式 `client_fingerprint` 字段，从 `tls.client_fingerprint` 中提取，并传递给 `VlessTransportConnector` 的显式参数。这样保持调用链语义清晰 — 不依赖隐式透传。

建议选 **方案 A**（审计结论是代码通路已存在），因为：
- `connect_tls_upstream()` 已经正确消费 `ClientTlsConfig.client_fingerprint`
- VLESS 的非 Reality TLS 出站通过 `build_vless_outbound_transport()` → `connect_tls_upstream()` 传入完整的 `tls` 配置
- 添加显式参数仅增加维护成本，不带来新功能

### 阻塞点 / 风险

- 需要确认 **SplitHTTP + TLS 路径**也正确传递了 fingerprint（`build_vless_outbound_transport:44-46` 调用了 `connect_tls_upstream(socket, tls, ...)`）
- 需要确认 **gRPC/H2/WS + TLS 路径**也正确传递了 fingerprint（`build_vless_outbound_transport:101-143` 同）

---

## 缺口 3: `udp_relay_chain_final_transport_limited`

### 现状

`crates/proxy/src/runtime/udp_dispatch/start.rs:553-911` 的 `start_relay_flow()` 是 UDP 中继链的入口。

支持的 VLESS 最终跳传输：TCP、TLS、Reality、WS、gRPC、H2、HTTP Upgrade。

不支持的：SplitHTTP（需要第二个 TCP 连接）、QUIC（需要非 TCP 载体）。

**SplitHTTP 特殊情况：** `start_relay_flow()` 的 `SplitHTTP fast path` (行 594-677) 只对 **VLESS 作为最终跳且 `split_http: Some(_)`** 生效，且需要两次 `dispatch_tcp_relay_prefix()` — 一次 POST、一次 GET。

当前的 SplitHTTP 路径 (行 598-677) 看起来**已经实现** — 它分别建立 POST 和 GET 中继流，然后调用 `build_vless_split_http_over_relay()` 合并。所以 SplitHTTP 作为最终跳**可能已经支持**。

```rust
// start.rs:598-677 — SplitHTTP fast path
if matches!(chain.last(), Some(ResolvedLeafOutbound::Vless { split_http: Some(_), .. })) {
    // ... 创建 post_carrier + get_carrier
    // ... 调用 build_vless_split_http_over_relay(post_carrier, get_carrier, cfg)
    // ... 建立 VLESS UDP upstream
}
```

**QUIC 情况：** 行 705-713 显式拒绝了 QUIC：
```rust
if quic.is_some() {
    return Err(FlowFailure { stage: "udp_relay_final_transport", ... });
}
```
这是因为 QUIC 使用 UDP socket，而中继链模型依赖 TCP 中继前缀建立流。这个限制是架构性的，无法在中继链模型中绕过。

### 实现路径

**SplitHTTP:** 代码看起来已实现。如果确实有效则关闭此限制（从 UDP 不支持列表中移除 SplitHTTP），否则调试修复。

**QUIC:** 保持为已知架构限制。QUIC-over-TCP-chain 没有通用解法。

### 建议

此缺口保持记录状态。SplitHTTP 验证通过后可更新 limitation 文本，QUIC 保持 `unsupported`。

---

## 实施顺序

```
Phase 1: Gap 1 — MUX UDP outbound (最高价值)
  1.1 MuxConnectionPool::open_udp_stream()
  1.2 VlessUdpOutboundManager 集成 MUX UDP 子流
  1.3 连接 UdpDispatch → mux_pool 的 UDP 出站路径

Phase 2: Gap 2 — TLS fingerprint (审计 + 文档)
  2.1 验证所有 VLESS 非 Reality TLS 路径的 fingerprint 传递
  2.2 如有遗漏则补充；否则仅更新文档

Phase 3: Gap 3 — Relay chain (验证 + 文档)
  3.1 验证 SplitHTTP 最终跳在 UDP 中继链中确实有效
  3.2 更新 limitations 列表 + docs
```
