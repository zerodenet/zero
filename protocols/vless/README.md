# VLESS

> 参照 Xray-core v25.3.1 | Crate: `vless`

VLESS 是 Xray 项目的核心入站/出站协议，无状态、轻量级，使用 UUID 身份验证，依赖外层 TLS 提供加密，自身不实现加密层。

## 版本追踪

| 项目 | 版本 |
|------|------|
| 参照实现 | [Xray-core](https://github.com/XTLS/Xray-core) v25.3.1 |
| 协议版本 | VLESS v0（协议头 version 字段恒为 0） |
| 本实现 | `vless` crate v25.3.1 |

## 协议帧

```
[version:1][uuid:16][addon:1+M][command:1][port:2][address_type:1][address:?]
```

- **version** — 始终 `0x00`
- **uuid** — 16 字节用户标识
- **addon** — 可选扩展（含 Flow），首字节决定是否存在
- **command** — `0x01` TCP、`0x02` UDP、`0x03` MUX
- **address** — IPv4 / IPv6 / 域名

## 功能对齐状态

### 命令分发

| 命令 | 状态 |
|------|------|
| TCP (`0x01`) | ✅ |
| UDP (`0x02`) | ✅ |
| MUX (`0x03`) | ✅ |

### AEAD Flow (Vision)

| Flow | 状态 |
|------|------|
| `xtls-rprx-vision` | ✅ |
| `xtls-rprx-vision-udp443` | ✅ |

### UDP

| 特性 | 状态 |
|------|------|
| UDP v1/v2 包格式 | ✅ |
| 链式全传输 | ✅ |
| Per-target upstream + 异步轮询 | ✅ |
| 会话空闲超时 | ✅ |

### MUX

| 特性 | 状态 |
|------|------|
| Xray 兼容帧 + 连接池 + per-stream AES-128-GCM | ✅ |
| MUX UDP 子连接 | ❌ |

### 传输层

| 传输 | 入站 | 出站 |
|------|------|------|
| TCP / TLS / Reality / WS / gRPC / H2 / QUIC / HTTPUpgrade / SplitHTTP | ✅ | ✅ |
| DomainSocket | ❌ 不计划 |

### Fallback

| SNI/ALPN 探测 + 非 TLS 流量回落 | ✅ |

### TLS 指纹

Reality 模式通过自研 `ztls` 实现完整 ClientHello 控制。非 Reality TLS 路径同样使用 `ztls`（`connect_tls13_upstream`），但存在透传缺陷：

> `connect_tls13_upstream` 接收 `_fp: &TlsFingerprint` 参数但未使用——始终传 `DEFAULT_CIPHER_SUITES`，fingerprint 定义的套件列表和 `kx_groups` 未透传到 `ztls`。非 bug，但 fingerprint 配置在此路径下不生效。

**结论**：不需要 uTLS 外部库——`ztls` 已覆盖 TLS 指纹需求。待修的是一个参数透传问题。

## 待实现

### MUX UDP
`handle_vless_mux_session` 将所有 MUX 子流创建为 `Network::Tcp`。Xray 支持 `network=0x02`（UDP）子连接。

### QUIC 0-RTT
Xray 支持 QUIC 0-RTT 握手加速，当前使用标准握手。

### TLS 指纹透传
`connect_tls13_upstream` 应将 `TlsFingerprint` 的 `cipher_suites` 和 `kx_groups` 传给 `ztls::handshake::Tls13Config`。

## 架构

```
protocols/vless/src/
├── lib.rs            # crate root, re-exports
├── inbound.rs        # VlessInbound: accept, auth, session dispatch
├── outbound.rs       # VlessOutbound: connect, send request, relay
├── flow.rs           # Vision flow (AES-128-GCM encrypt/decrypt)
├── mux.rs            # MUX frame, client, server
├── protocol.rs       # common: frame parsing, address, UUID
├── udp.rs            # UDP packet v1/v2
└── reality/          # Reality TLS 1.3
    ├── mod.rs
    ├── reality_server_connection.rs
    ├── stream.rs
    └── util.rs
```

## 参考

- [Xray-core VLESS 出站配置](https://xtls.github.io/en/config/outbounds/vless.html)
- [VLESS 协议规范](https://github.com/XTLS/Xray-core/discussions/1967)
