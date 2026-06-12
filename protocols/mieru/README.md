# Mieru

> 参照 mieru | Crate: `mieru`

Mieru 是一种加密代理协议。它先与对端建立一条 XChaCha20-Poly1305 加密隧道（基于用户名/密码/系统时间派生密钥），**然后在隧道内用 socks5 协商代理目标并 relay**。这与 vless / trojan / shadowsocks 等"目标在握手时确定"的协议不同——mieru 的 mieru 会话本身不携带目标。

## 协议模型（socks5-in-tunnel）

```
应用 ── Zero socks5/HTTP/... 入站（解析出 target）
         │
         │ mieru 出站：建立加密隧道（openSession，仅 sessionID + 用户身份）
         │ 然后在隧道里直接发 socks5 请求：[05, CMD, 0, ATYP, addr, port]
         ↓
              mieru 加密会话（XChaCha20-Poly1305）
         ↓
              对端（mita）在隧道终点读 socks5 请求 → 拨目标 → relay
```

关键点：

- **openSessionRequest 不携带目标**，只携带 sessionID；用户身份经密钥派生 + nonce user hint 体现。
- **隧道内的 socks5 不做 greeting / method / user-pass 认证**——mieru 会话本身即认证（对端 `ClientSideAuthentication`）。客户端直接发 socks5 请求，读 socks5 响应。
- 密钥派生：`key = PBKDF2-HMAC-SHA256(SHA-256(password ‖ 0x00 ‖ username), SHA-256(uint64_be(时间取整 2 分钟)), 64 iter, 32 bytes)`。
- segment 成帧：session segment **无前缀 padding0**（nonce 在偏移 0）；padding 是 suffix。

## 协议来源

| 项目 | 来源 |
|------|------|
| 参照实现 | [mieru](https://github.com/enfein/mieru) |
| 本实现 | `mieru` crate |

## 功能对齐状态

| 特性 | 状态 |
|------|------|
| TCP 加密隧道（openSession 握手） | ✅ 已与 mita 互通验证 |
| TCP 出站：socks5-in-tunnel 目标协商 + relay | ✅ 已与 mita 端到端互通验证（httpbin.org） |
| TCP 入站：socks5-in-tunnel（对称于出站） | ✅ loopback 验证（对已验证出站） |
| UDP 出站：socks5 UDP ASSOCIATE | ✅ 已与 mita 互通验证（DNS relay） |
| UDP 入站：socks5 UDP ASSOCIATE | ✅ 已实现（对称设计） |
| 密钥派生（HashPassword）+ nonce user hint | ✅ 已与 mita 字节级对齐 |
| MUX（多会话复用单条 underlay） | ➖ 暂不实现（性能优化项；单会话模式 TCP/UDP 双向已可用） |

## 架构

```
src/lib.rs       — crate root, re-exports
src/inbound.rs   — MieruInbound（openSession 握手，不含目标）
src/outbound.rs  — MieruOutbound（建立加密隧道，不含目标）
src/segment.rs   — segment 成帧（build/parse，无前缀 padding0）
src/crypto.rs    — 密钥派生（HashPassword）+ XChaCha20-Poly1305 + nonce user hint
src/udp.rs       — UDP associate wrap/unwrap
src/protocol.rs  — ProtocolCapabilityDescriptor + TcpSessionProtocol
src/metadata.rs  — segment metadata 编解码
src/session.rs   — 会话状态（seq/window/timestamp）
```

代理层的 socks5-in-tunnel 编排（目标协商）在 `crates/proxy/src/outbound/mieru.rs`（出站 `socks5_connect`）与 `crates/proxy/src/inbound/mieru.rs`（入站 `socks5_serve`）。

## 参考

- [mieru](https://github.com/enfein/mieru)
