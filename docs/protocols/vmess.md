# VMess

VMess 的主流传输基线（TLS / WS / gRPC）上的 TCP + UDP + MUX 已完整实现，外部互通覆盖 Xray、sing-box、Mihomo 三大实现族。本文档按能力分级描述。

## 能力分级

### 基础（基本实现）

协议能跑通的最小功能集。满足 TLS 上的 VMess TCP 代理需求。

| 能力 | 状态 | 说明 |
|------|------|------|
| TCP inbound over TLS | ✅ 完成 | 单用户 + 多用户认证，`VmessInboundHandler` 接入 `InboundProtocol` |
| TCP outbound over TLS | ✅ 完成 | `TcpSessionProtocol` 集成，返回 `VmessOutboundSession` |
| Xray AEAD header | ✅ 完成 | KDF (HMAC-SHA256 分层)、auth ID 创建、header seal/open |
| Body AEAD chunk relay | ✅ 完成 | `VmessAeadStream` (AsyncRead + AsyncWrite) 双向加解密 |
| 4 个 cipher | ✅ 完成 | `aes-128-gcm`、`chacha20-poly1305`、`none`、`zero` |
| `auto` 归一化 | ✅ 完成 | 配置导入时 `auto` → `aes-128-gcm` |
| 配置校验 + feature gate | ✅ 完成 | UUID 解析、cipher 校验、TLS required for inbound |
| 统一 runtime 接入 | ✅ 完成 | InboundProtocol trait 接入 `serve_inbound()` |
| 运行时可观测 | ✅ 完成 | session 生命周期、统计、事件 |

### 完整（对标大众）

对标 Xray VMess 实现的常规特性。覆盖市面主流 VMess 节点的使用场景。

| 能力 | 状态 | 说明 |
|------|------|------|
| WebSocket over TLS 传输 | ✅ 完成 | Inbound + Outbound 双向，Xray WS 互通已验证 |
| gRPC over TLS 传输 | ✅ 完成 | Inbound + Outbound 双向，Xray gRPC 互通已验证 |
| UDP over stream (CMD_UDP) | ✅ 完成 | Inbound 接受 VMess packet + raw datagram 两种 payload 模式；Outbound 单跳 |
| Mux.Cool TCP sub-connection | ✅ 完成 | `VmessMuxConnectionPool` 连接池，按 (server, port, uuid, cipher, transport) 分桶 |
| Mux.Cool UDP sub-connection | ✅ 完成 | 支持 VMess packet + raw datagram 两种 payload 模式 |
| 多用户认证 | ✅ 完成 | `accept_tcp_multi` 缓冲读取 + 多密钥尝试，支持 per-user credential_id / principal_key / rate limits |
| Body AEAD 高级特性 | ✅ 完成 | Authenticated length、SHAKE128 chunk masking、global padding、2^14 chunk 周期 rekey |
| `cipher: none` | ✅ 完成 | Xray TCP 互通已验证 |
| VMess → VMess UDP relay chain | ✅ 完成 | 同协议链路，`VmessUdpOutboundManager` 管理 |
| 外部互通 (Xray) | ✅ 完成 | 双向 TCP (`aes-128-gcm`/`none`)、双向 WS+TLS TCP、双向 gRPC+TLS TCP、双向 UDP |
| 外部互通 (sing-box) | ✅ 完成 | Zero outbound → sing-box inbound TCP + UDP |
| 外部互通 (Mihomo) | ✅ 完成 | Mihomo outbound → Zero inbound TCP (`auto`) + UDP (`CMD_UDP` raw datagram) |
| ⚠ `cipher: zero` | ⚠ 受限 | Xray inbound 拒绝 `zero` security，仅 Zero-to-Zero 路径可用 |

### 特级（Zero 特色）

Zero 在对标基础上提供的独有能力，主流实现不具备。

| 能力 | 说明 |
|------|------|
| MUX 连接池 | Outbound MUX 按 (server, port, uuid, cipher, transport) 分桶复用，不同目标共享底层连接 |
| Broadcast UDP response bridge | `VmessUdpOutboundManager` 使用 broadcast channel，支持多个 UDP session 共享同一条 VMess UDP stream 的响应 |
| UDP payload-mode 自动检测 | Inbound 自动检测 payload 是 VMess packet 格式还是主流 raw datagram 格式，兼容两种客户端行为 |
| 多用户缓冲读取 + early reject | `VmessReadBuffer` 一次读取 wire data，多密钥依次尝试，认证失败发送 rejection 而非静默断开 |
| 统一 InboundProtocol 入口 | Raw TLS / WS / gRPC 三种传输路径共用同一 `serve_inbound()` 管线，无协议特例分支 |

## Validation Coverage

In-tree validation covers:

- VMess TCP raw TLS, WSS, and gRPC outbound paths.
- Bidirectional AEAD body relay for every explicit supported cipher.
- Shutdown termination chunk delivery for every cipher.
- VMess UDP packet framing (domain, IPv4, IPv6 targets).
- SOCKS5 UDP ASSOCIATE -> VMess TLS outbound -> VMess inbound -> direct UDP echo.
- SOCKS5 entry -> VMess -> VMess -> direct UDP echo as a same-protocol VMess relay-chain path.
- SOCKS5 TCP entry -> VMess TCP MUX outbound -> VMess inbound -> direct TCP echo.
- SOCKS5 UDP ASSOCIATE -> VMess MUX UDP sub-connection -> VMess inbound -> direct UDP echo.

External validation covers:

- Zero VMess outbound -> Xray VMess inbound, TCP, `aes-128-gcm`.
- Zero VMess outbound -> Xray VMess inbound, TCP, `none`.
- Xray VMess outbound -> Zero VMess inbound, TCP, `aes-128-gcm`.
- Xray VMess outbound -> Zero VMess inbound, TCP, `none`.
- Zero VMess outbound -> Xray VMess inbound, WebSocket over TLS, TCP.
- Xray VMess outbound -> Zero VMess inbound, WebSocket over TLS, TCP.
- Zero VMess outbound -> Xray VMess inbound, gRPC over TLS, TCP.
- Xray VMess outbound -> Zero VMess inbound, gRPC over TLS, TCP.
- Zero VMess outbound -> sing-box VMess inbound, TCP, `aes-128-gcm`.
- Zero VMess outbound -> sing-box VMess inbound, UDP.
- Mihomo VMess outbound -> Zero VMess inbound, TCP, `auto`.
- Mihomo VMess outbound -> Zero VMess inbound, UDP over `CMD_UDP` raw datagram payload.
- Zero VMess outbound -> Xray VMess inbound, UDP.
- Xray VMess outbound -> Zero VMess inbound, UDP over Mux.Cool raw datagram payload.
- Xray rejects VMess `zero` security on inbound; Zero therefore does not present `cipher: zero` as mainstream Xray-compatible behavior.

## Relay-chain Boundary

Current VMess chain tracking is same-protocol only: `vmess -> vmess`. Local SOCKS5 or Mixed entry is just the client ingress and does not make the path a general cross-protocol relay-chain. Arbitrary multi-protocol combinations are not part of the VMess completion target.

## Remaining Gaps

| Gap | Impact |
|------|------|
| `cipher: zero` external compatibility | Zero-to-Zero is covered; Xray inbound rejects `zero` security, so GUI integrations should not expose it as a mainstream default |

Zero has validated the listed Xray, sing-box, and Mihomo/Clash-family paths. Do not generalize that evidence to untested transport combinations.

## Inbound Config

```json
{
  "tag": "vmess-in",
  "listen": { "address": "0.0.0.0", "port": 443 },
  "protocol": {
    "type": "vmess",
    "users": [
      {
        "id": "11111111-2222-3333-4444-555555555555",
        "cipher": "aes-128-gcm"
      }
    ],
    "tls": {
      "cert_path": "certs/fullchain.pem",
      "key_path": "certs/privkey.pem"
    }
  }
}
```

`tls` is required for VMess inbound. `users[].cipher` is optional and defaults to `aes-128-gcm`. `ws` and `grpc` are mutually exclusive.

## Outbound Config

```json
{
  "tag": "vmess-out",
  "protocol": {
    "type": "vmess",
    "server": "example.com",
    "port": 443,
    "id": "11111111-2222-3333-4444-555555555555",
    "cipher": "aes-128-gcm",
    "mux_concurrency": 8
  }
}
```

Optional transport fields are `tls`, `ws`, and `grpc`. `ws` and `grpc` are mutually exclusive. `mux_concurrency` enables VMess TCP MUX for outbound TCP sessions; `mux_idle_timeout_secs` is accepted as a pool policy field.
