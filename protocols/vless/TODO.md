# VLESS TODO

进度记录，对标 Xray VLESS 的缺失功能。

## 已完成 (2026-05-13)

- [x] gRPC 传输 (入站 + 出站)
- [x] HTTP/2 传输 (入站 + 出站)
- [x] QUIC 传输 (入站 + 出站)
- [x] MUX 流级别 AES-128-GCM 加密
- [x] UDP 链式全传输支持 (TLS / Reality / WS / gRPC / H2 / QUIC)
- [x] VlessUdpOutboundManager 接入生产路径
- [x] VlessUserConfig.flow 字段传播
- [x] QUIC 入站运行时集成

---

## 待实现

### Fallback

认证失败时将连接转发到其他服务，实现伪装能力。

**范围：**
- 配置：`InboundProtocolConfig::Vless` 新增 `fallback` 字段
- TLS 场景下支持 ALPN 映射不同回落目标
- 修改 `handle_vless_client`：认证失败时做双向中继而非直接返回错误

**工时估计：** 4-6h

---

### HTTPUpgrade 传输

WebSocket 的轻量替代，CDN 友好，无帧格式和掩码开销。

**范围：**
- 新建 `crates/proxy/src/transport/http_upgrade.rs`
- 客户端：GET + Upgrade 头 → 101 响应 → 裸双向流
- 服务端：读升级请求 → 发 101 → 裸双向流
- 配置：`http_upgrade` 块 (path, host)

**工时估计：** 2-3h

---

### SplitHTTP (XHTTP) 传输

拆分上传/下载为独立 HTTP 请求，兼容仅支持 GET 或 POST 的 CDN。

**范围：**
- 新建 `crates/proxy/src/transport/split_http.rs`
- POST 连接负责上传，GET 连接负责下载
- 服务端需 session ID 配对机制
- 可能需修改 `ClientStream` trait 支持非对称连接

**工时估计：** 8-12h

---

### Packet Encoding (新版 VLESS UDP 包格式)

优化小包性能的新版 VLESS UDP 包格式。

**范围：**
- 新增 `parse_udp_packet_v2` / `build_udp_packet_v2`
- 协商机制：服务端自动识别新旧格式

**工时估计：** 1-2h

---

### gRPC MultiMode

gRPC 传输支持多服务名，连接时随机选取。

**范围：**
- `GrpcConfig.service_name` 从 `String` 改为 `Vec<String>`

**工时估计：** 0.5h

---

## 不建议实现

### DomainSocket

Unix 域套接字传输层。仅适合本机进程间通信场景（配合 Nginx/Caddy 前置代理），非独立代理的核心需求。
