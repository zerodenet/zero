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
| TCP (`0x01`) | ✅ 完整 |
| UDP (`0x02`) | ✅ 完整（含链式全传输 + per-target upstream + 异步轮询） |
| MUX (`0x03`) | ✅ Xray 兼容帧 + 连接池 + per-stream AES-128-GCM |

### AEAD Flow (Vision)

| Flow | 状态 |
|------|------|
| `xtls-rprx-vision` | ✅ |
| `xtls-rprx-vision-udp443` | ✅ |
| AES-128-GCM + HKDF-SHA256 | ✅ |

### UDP

| 特性 | 状态 |
|------|------|
| UDP v1 包格式（无地址省略） | ✅ |
| UDP v2 包格式（地址省略优化） | ✅ |
| 链式全传输支撑 | ✅ |
| Per-target upstream 管理 | ✅ |
| 会话空闲超时 | ✅ |
| 统一 chain_tasks JoinSet 异步轮询 | ✅ |

### MUX

| 特性 | 状态 |
|------|------|
| Xray 兼容 MUX 帧格式 | ✅ |
| 连接池 (MuxConnectionPool) | ✅ |
| Per-stream AES-128-GCM 加密 | ✅ |
| MUX UDP 子连接 | ❌ |

### 传输层

| 传输 | 入站 | 出站 | 链式 |
|------|------|------|------|
| TCP (RAW) | ✅ | ✅ | ✅ |
| TLS | ✅ | ✅ | ✅ |
| Reality (自研 TLS 1.3) | ✅ | ✅ | ✅ |
| WebSocket | ✅ | ✅ | ✅ |
| gRPC (MultiMode) | ✅ | ✅ | ✅ |
| HTTP/2 | ✅ | ✅ | ✅ |
| QUIC | ✅ | ✅ | ✅ |
| HTTPUpgrade | ✅ | ✅ | ✅ |
| SplitHTTP (XHTTP) | ✅ | ✅ | ✅ |
| DomainSocket | ❌ 不计划 | — | — |

### Fallback

| 特性 | 状态 |
|------|------|
| SNI 探测回落 | ✅ |
| ALPN 匹配回落 | ✅ |
| 非 TLS 流量回落 | ✅ |
| fallback 配置对象 | ✅ |

### 安全性

| 特性 | 状态 |
|------|------|
| UUID 认证 (canonical + 32-hex) | ✅ |
| Reality TLS 1.3: 自研实现 | ✅ |
| Reality: ClientHello 指纹控制 | ✅ |
| Reality: X25519 ECDH | ✅ |
| Reality: Session ID 加密 | ✅ |
| Reality: Ed25519 证书签名 | ✅ |
| uTLS 客户端指纹 (非 Reality) | ❌ |

## 待实现

### MUX UDP

当前 `handle_vless_mux_session` 将所有 MUX `new_stream` 子流创建为 `Network::Tcp`。Xray 的 MUX 支持 `network=0x02`（UDP）子连接。需要：
1. `parse_new_stream_payload` 解析首字节 network 字段
2. `handle_vless_mux_session` 根据 network 创建对应类型的子流
3. UDP 子流接入 `handle_vless_udp_session` 的 dispatch 链路

### QUIC 0-RTT

Xray 支持 QUIC 0-RTT 握手加速。当前实现使用标准握手。

### uTLS 客户端指纹

Reality 模式通过自研 TLS 1.3 实现原生指纹。非 Reality 的普通 TLS 模式缺少 uTLS 库的客户端指纹模拟。

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

## 集成测试

```powershell
# Reality 对 Xray 互操作测试
cargo test -p zero-proxy --test vless relays_tcp_through_vless_reality_xray -- --ignored
```

设置 `ZERO_XRAY_IMAGE` 覆盖默认镜像 `ghcr.io/xtls/xray-core:latest`。

## 参考

- [Xray-core VLESS 出站配置](https://xtls.github.io/en/config/outbounds/vless.html)
- [VLESS 协议规范](https://github.com/XTLS/Xray-core/discussions/1967)
