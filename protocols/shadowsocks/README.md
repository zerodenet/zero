# Shadowsocks

> 参照 shadowsocks-rust v1.21.2 | Crate: `shadowsocks`

Shadowsocks 是一种加密代理协议，使用预共享密钥 + AEAD 密码进行加密传输。

## 版本追踪

| 项目 | 版本 |
|------|------|
| 参照实现 | [shadowsocks-rust](https://github.com/shadowsocks/shadowsocks-rust) v1.21.2 |
| 密码套件 | AES-128-GCM、AES-256-GCM、ChaCha20-IETF-Poly1305、2022-Blake3 |
| 本实现 | `shadowsocks` crate v1.21.2 |

## 功能对齐状态

| 特性 | 状态 |
|------|------|
| TCP: AEAD 分块加密 | ✅ |
| UDP: AEAD 加密 datagram | ✅ |
| AEAD 密码: aes-128-gcm / aes-256-gcm / chacha20-ietf-poly1305 | ✅ |
| 2022-Blake3 (密码派生 + UDP 包头) | ✅ |
| 入站: accept + 解密 → route → relay | ✅ |
| 出站: TCP connect + AEAD 隧道 | ✅ |
| 出站: UDP encrypt + send + recv relay | ✅ |
| UDP 链式出站 (SS → SS) | ✅ |

## 架构

```
src/lib.rs       — crate root, re-exports
src/inbound.rs   — ShadowsocksInbound (accept, AEAD decrypt, UDP relay)
src/outbound.rs  — ShadowsocksOutbound (TCP tunnel, AEAD relay)
src/protocol.rs  — shared: cipher enum, key derivation, target data encode/decode
```

## 参考

- [shadowsocks-rust](https://github.com/shadowsocks/shadowsocks-rust)
- [SIP022 协议规范](https://shadowsocks.org/doc/sip022.html)
