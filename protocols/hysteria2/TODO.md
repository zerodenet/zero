# Hysteria2 TODO

## 架构改进

### ~~stream.rs 属于传输层~~ ✅ 已修复

`stream.rs` → `transport/quic.rs`，现已对齐 VLESS 的 `transport/quic.rs` 结构。

---

## 功能待完善

### ~~出站 auth 流程~~ ✅ 已完成

`connect_via_hysteria2_upstream` 现已完整实现：
- SHA256(salt) → HMAC-SHA256(password, salt) → build_auth_frame + send
- 读 parse_auth_response 验证
- QUIC 连接使用独立配置（ALPN: hysteria2, insecure skip verify）

### ~~出站 TCP stream~~ ✅ 已完成

- conn.open_bi() 建立双向流
- 发送 build_tcp_connect_header + 读 connect 响应
- 返回 Hysteria2Stream 用于双向中继

### ~~UDP 域名解析~~ ✅ 已完成

- 使用 resolver (DnsResolver trait) 解析 Address::Domain
- 取首个解析结果构造 SocketAddr 转发
