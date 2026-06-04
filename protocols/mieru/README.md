# Mieru

> 参照 mieru v3.33.0 | Crate: `mieru`

Mieru 是一种基于 TCP 的加密代理协议，使用 XChaCha20-Poly1305 进行加密，支持 UDP over TCP。

## 版本追踪

| 项目 | 版本 |
|------|------|
| 参照实现 | [mieru](https://github.com/enfein/mieru) v3.33.0 |
| 本实现 | `mieru` crate v3.33.0 |

## 功能对齐状态

| 特性 | 状态 |
|------|------|
| TCP: XChaCha20-Poly1305 加密隧道 | ✅ |
| UDP: UDP associate → 加密 UDP over TCP | ✅ |
| 入站: accept + handshake → route → relay | ✅ |
| 出站: TCP connect + handshake → relay | ✅ |
| UDP chain 出站 | ✅ |

## 架构

```
src/lib.rs       — crate root, re-exports
src/inbound.rs   — MieruInbound (handshake, auth, route)
src/outbound.rs  — MieruOutbound (connect, handshake, relay, encrypt/decrypt)
src/udp.rs       — UDP associate wrap/unwrap
src/protocol.rs  — shared: key derivation, segment frame
```

## 参考

- [mieru](https://github.com/enfein/mieru)
