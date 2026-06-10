# VMess Shared

对应 `protocols/vmess/src/shared.rs` — `VmessCipher`、地址编码、UUID 解析、I/O 辅助函数。

## VmessCipher

```rust
pub enum VmessCipher {
    Aes128Gcm,           // "aes-128-gcm"
    Chacha20Poly1305,    // "chacha20-poly1305"
    None,                // "none" — 无加密 body
    Zero,                // "zero" — 零加密（仅 Zero-to-Zero）
    Auto,                // "auto" — 归一化为 aes-128-gcm
}
```

### auto 归一化

配置导入时 `auto` → `aes-128-gcm`：

```rust
impl VmessCipher {
    pub fn normalize(self) -> Self {
        match self {
            VmessCipher::Auto => VmessCipher::Aes128Gcm,
            other => other,
        }
    }
}
```

### zero 兼容性

`cipher: zero` 在 Xray inbound 被拒绝 — Xray 不认同 `zero` security type。Zero-to-Zero 路径可用，GUI 不应作为主流默认选项暴露。

## 地址编码

VMess address 格式遵循 VMess 规范：
- ATYP: 0x01 (IPv4), 0x02 (domain), 0x03 (IPv6)
- Domain 地址前缀 1 字节长度

## UUID 解析

标准 UUID 格式（带或不带破折号均可）。用于：
- KDF 派生 cmd_key（HMAC-SHA256 分层）
- Auth ID 计算
- Header encryption key 派生

## 命令类型

| Command | 值 | 用途 |
|---------|-----|------|
| `CMD_TCP` | 0x01 | TCP 代理 |
| `CMD_UDP` | 0x02 | UDP over stream |

## I/O 辅助

- `read_exact`: 带超时的精确读取
- VMess header 长度计算
- Response header 检查
