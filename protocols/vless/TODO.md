# VLESS TODO

对标 Xray VLESS 的功能差距追踪。

## 已完成

### 协议核心
- [x] VLESS v0 帧格式 (version, UUID, addon, command, port, address)
- [x] UUID 认证 (canonical + 32-hex)
- [x] TCP / UDP / MUX 命令分发
- [x] AEAD Flow (xtls-rprx-vision, xtls-rprx-vision-udp443) — AES-128-GCM + HKDF-SHA256
- [x] UDP 包 v1 + v2 (auto-detect, address omission)

### 传输层 (7 种)
- [x] TLS (rustls, cert/key, ALPN, SNI, insecure, custom CA)
- [x] WebSocket (tokio-tungstenite, path validation, custom headers)
- [x] gRPC (h2 framing, service_name, TLS+gRPC)
- [x] HTTP/2 (raw DATA frames, host+path)
- [x] QUIC (quinn, rustls-ring, 入站 + 出站)
- [x] HTTPUpgrade (GET + Upgrade 头 → 101, path validation)
- [x] Reality (自研 TLS 1.3: ClientHello 指纹, X25519 ECDH, Session ID 加密, HMAC 证书, Ed25519 签名)

### MUX
- [x] Xray 兼容 MUX 帧格式 (session_id + length + payload, max 16384)
- [x] MUX 连接池 (PoolKey, TransportKey, MuxPoolConn, MuxStreamRelay)
- [x] 流级别 AES-128-GCM 加密 (per-stream key derivation, counter nonce)

### UDP
- [x] VLESS UDP 包 v1 + v2 格式
- [x] UDP 链式全传输支持 (TLS / Reality / WS / gRPC / H2 / QUIC / HTTPUpgrade)
- [x] VlessUdpOutboundManager (per-target upstream 管理)
- [x] UDP 会话空闲超时 + 流追踪

### Fallback
- [x] 入站 TLS SNI/ALPN 探测 → 回落至配置目标
- [x] 非 TLS 流量直接回落
- [x] 配置: `fallback: { address, port }` + ALPN 映射

### 架构
- [x] Transport 实现从 proxy 移至 protocols/vless
- [x] Transport 分发去重 (build_vless_outbound_transport)
- [x] InboundHandler / OutboundHandler 死代码删除
- [x] 协议类型归位 (ConfiguredVlessUsers, VlessUdpTransport, MuxPoolConn...)
- [x] Reality 测试文件独立提取 (11 个 test 文件, 2294 行)
- [x] proxy 瘦身 7318 → 5398 行 (-26%)

---

## 待实现

### ~~SplitHTTP (XHTTP)~~ ✅ 完整实现

- 客户端多连接：开两条 TCP，peer_addr 取 GET 连接，显式 drop 防泄漏
- 服务端多路: `SplitHttpRegistry` 按 X-Session-Id 配对 POST/GET
- `SplitHttpPairedStream<R, W>` 支持非对称读写 (reader=GET, writer=POST)
- Chunked transfer encoding 双向编解码，shutdown 写 `0\r\n\r\n` 干净终止
- 60s 超时 + 清理
- 入站 accept / 出站 connect 全分发链路

### ~~gRPC MultiMode~~ ✅ 已完成

- `serve_grpc(stream, services, handler)`: 循环 accept h2 requests，每请求构建
  GrpcStream + spawn handler task。单 h2 连接承载多路 gRPC stream。
- `poll_write` 使用 `poll_send` 正确反压，不静默丢数据
- `poll_shutdown` 发送 gRPC END_STREAM frame
- `accept_grpc` 保留为 SingleMode legacy wrapper

---

## Hysteria2 协议 ✅ (2026-05-14)

完整实现，对标生产可用：
- `protocols/hysteria2/` crate（shared, inbound, outbound, udp）— 纯协议层，no_std
- `stream.rs` 已提取到 `transport/quic.rs`（Hysteria2Connector）
- 入站：QUIC accept_connection → TLS key export salt → HMAC-SHA256 验证 → auth OK/ERR
- 入站：accept_bi loop → parse connect header → route → bidirectional relay
- 入站：UDP datagram loop with resolver-based domain forwarding
- 出站：QUIC connect → auth handshake → open_bi → connect header → relay
- 配置模型 + engine 类型 + inventory 注册
- 示例配置 `examples/v0.1.0/hysteria2.json`

---

## 不建议实现

### DomainSocket

Unix 域套接字传输。仅适合本机前置代理（Nginx/Caddy）场景，非独立代理核心需求。
