# Protocol Overview

This section tracks the protocol implementation surface in the current codebase. It is factual documentation for GUI authors, panel adapters, and kernel integrators. Runtime consumers should still treat `capabilities.protocols` as the machine-readable source of truth.

## 状态术语

| 状态 | 含义 |
|------|------|
| `supported` | 正常内核能力，无已知协议级缺口 |
| `partial` | 基线路径存在，但互操作、MUX、特殊传输或服务端/客户端方向缺口仍然存在 |
| `experimental` | 已实现，但不适合生产兼容假设 |
| `unsupported` | 未实现 |
| `not_applicable` | 协议不定义此方向 |

## Matrix

| Protocol | Config type | Status | TCP inbound | TCP outbound | UDP inbound | UDP outbound | Tracking |
|------|------|------|-------------|--------------|-------------|--------------|----------|
| SOCKS5 | `socks5` | `supported` | `supported` | `supported` | `supported` | `supported` | [SOCKS5](./socks5/index.md) |
| HTTP CONNECT | `http_connect` | `supported` | `supported` | `unsupported` | `not_applicable` | `not_applicable` | [HTTP CONNECT](./http-connect/index.md) |
| Mixed | `mixed` | `supported` | `supported` | `unsupported` | `supported` | `unsupported` | [Mixed](./mixed/index.md) |
| VLESS | `vless` | `partial` | `supported` | `supported` | `partial` | `partial` | [VLESS](./vless/index.md) |
| Hysteria2 | `hysteria2` | `partial` | `supported` | `supported` | `partial` | `partial` | [Hysteria2](./hysteria2/index.md) |
| Shadowsocks | `shadowsocks` | `partial` | `supported` | `supported` | `supported` | `supported` | [Shadowsocks](./shadowsocks/index.md) |
| Trojan | `trojan` | `partial` | `supported` | `supported` | `partial` | `partial` | [Trojan](./trojan/index.md) |
| Mieru | `mieru` | `partial` | `partial` | `partial` | `partial` | `partial` | [Mieru](./mieru/index.md) |
| VMess | `vmess` | `partial` | `partial` | `partial` | `partial` | `partial` | [VMess](./vmess/index.md) |

## 内核动作

| 名称 | 状态 | 说明 |
|------|------|------|
| `direct` | `supported` | 内核直连动作；TCP 和 UDP 出站可用 |
| `block` | `supported` | 内核拒绝动作；TCP 和 UDP 路由决策可使用 |
| `tun` | `supported` | 三层虚拟接口，通过 CLI/控制面控制，非静态 JSON `inbounds` |

## Documentation Layout

Each protocol has its own subdirectory under `docs/protocols/`. The subdirectory mirrors the protocol crate module structure:

```
docs/protocols/{protocol}/
  index.md     ← 对应 lib.rs（概览、能力矩阵、子页面链接）
  inbound.md   ← 对应 inbound.rs
  outbound.md  ← 对应 outbound.rs
  shared.md    ← 对应 shared.rs
  metadata.md  ← 对应 metadata.rs
  stream.md    ← 对应 stream.rs（如果存在）
  crypto.md    ← 对应 crypto.rs（如果存在）
  mux.md       ← 对应 mux.rs（如果存在）
  udp.md       ← 对应 udp.rs（如果存在）
```

Shared pages keep only cross-protocol summaries:

- [Configuration](./configuration.md): common config examples.
- [Incomplete](./incomplete.md): cross-protocol gap index.
- [Protocol Capabilities](../project/protocol-capabilities.md): descriptor and runtime capability model.
