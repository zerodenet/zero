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

### SplitHTTP (XHTTP)

拆分上传(POST) / 下载(GET)为独立 HTTP 连接，兼容仅支持 GET 的 CDN。

**范围：**
- 新建 `protocols/vless/src/transport/split_http.rs`
- 客户端: 两个 HTTP 连接，通过 session ID 配对
- 服务端: 配对 POST/GET 请求，session ID 关联
- 可能需修改流抽象支持非对称连接

**工时估计：** 8-12h

### gRPC MultiMode ✅

已实现：
- `GrpcConfig.service_names: Vec<String>` + 自定义反序列化器（`"str"` 或 `["arr"]`）
- `connect_grpc`: 随机选取 `service_names`
- `accept_grpc`: 匹配任意 `expected_services`

---

## 不建议实现

### DomainSocket

Unix 域套接字传输。仅适合本机前置代理（Nginx/Caddy）场景，非独立代理核心需求。
