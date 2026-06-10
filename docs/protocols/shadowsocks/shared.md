# Shadowsocks Shared

对应 `protocols/shadowsocks/src/shared.rs` — `CipherKind`、密钥派生、地址编码、AEAD 操作、TCP chunk 辅助函数。

## 支持的 Cipher

常规 AEAD：

- `aes-128-gcm`
- `aes-256-gcm`
- `chacha20-ietf-poly1305`

AEAD 2022：

- `2022-blake3-aes-128-gcm`
- `2022-blake3-aes-256-gcm`
- `2022-blake3-chacha20-poly1305`

## CipherKind

```rust
pub enum CipherKind {
    Aes128Gcm,              // "aes-128-gcm"
    Aes256Gcm,              // "aes-256-gcm"
    Chacha20Poly1305,       // "chacha20-ietf-poly1305"
    Blake3Aes128Gcm,        // "2022-blake3-aes-128-gcm"
    Blake3Aes256Gcm,        // "2022-blake3-aes-256-gcm"
    Blake3Chacha20Poly1305, // "2022-blake3-chacha20-poly1305"
}
```

密钥长度：

| Cipher | Key 长度 | Salt 长度 |
|--------|---------|----------|
| `aes-128-gcm` | 16 字节 | 16 字节 |
| `aes-256-gcm` | 32 字节 | 32 字节 |
| `chacha20-ietf-poly1305` | 32 字节 | 32 字节 |
| `2022-blake3-aes-128-gcm` | 16 字节 | 16 字节 |
| `2022-blake3-aes-256-gcm` | 32 字节 | 16 字节 |
| `2022-blake3-chacha20-poly1305` | 32 字节 | 16 字节 |

## Password 规则

### 常规 AEAD

使用普通 Shadowsocks password：

```json
{
  "type": "shadowsocks",
  "password": "your-secret-password",
  "cipher": "chacha20-ietf-poly1305"
}
```

`password` 通过 HKDF-SHA1 派生 session key：
```
session_key = HKDF_SHA1(password, salt, b"ss-subkey", key_len)
```

### AEAD 2022

使用标准 base64 key material：

```json
{
  "type": "shadowsocks",
  "password": "MDEyMzQ1Njc4OWFiY2RlZg==",
  "cipher": "2022-blake3-aes-128-gcm"
}
```

AES 2022 可接受冒号分隔的 identity key 链。Zero 使用最后一段作为用户 PSK，不发送 EIH identity headers。

Base64 解码长度校验：

| Cipher | 密码解码长度 |
|--------|-------------|
| `2022-blake3-aes-128-gcm` | 16 字节 |
| `2022-blake3-aes-256-gcm` | 32 字节 |
| `2022-blake3-chacha20-poly1305` | 32 字节 |

Session key 通过 Blake3 KDF 派生：
```
session_key = Blake3_KDF(master_key, salt, b"shadowsocks-2022-session-subkey", key_len)
```

## HKDF-SHA1 密钥派生

常规 AEAD 使用标准 HKDF-SHA1：
```
session_key = HKDF-SHA1(password_bytes, salt_bytes, b"ss-subkey", key_len)
```

`ring::hkdf` 实现，key 长度由 cipher 决定。

## 地址编码

Shadowsocks address 格式：
- `[ATYP][ADDR][PORT]`
- ATYP: 0x01 (IPv4), 0x03 (domain), 0x04 (IPv6)
- Domain 地址前缀 1 字节长度

## Feature 门控

- `feature = "crypto"` — 常规 AEAD cipher (HKDF-SHA1 + AEAD via `ring`)
- `feature = "blake3"` — AEAD 2022 cipher (额外启用 `crypto` 用于 AEAD 原语)

编译时校验：未启用对应 feature 的 cipher 在配置解析阶段即被拒绝。
