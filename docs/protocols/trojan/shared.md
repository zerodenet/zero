# Trojan Shared

对应 `protocols/trojan/src/shared.rs` — 密码/请求/地址读写、命令和地址类型常量。

## 常量

```rust
pub const PASSWORD_HASH_LEN: usize = 56;  // SHA224 hex
pub const CMD_TCP: u8 = 0x01;
pub const CMD_UDP: u8 = 0x03;
pub const ATYP_IPV4: u8 = 0x01;
pub const ATYP_DOMAIN: u8 = 0x03;
pub const ATYP_IPV6: u8 = 0x04;
pub const CRLF: &[u8] = b"\r\n";
```

## 密码处理

Trojan 使用 `SHA224(password)` 的 hex 编码作为 wire 上的 password hash：

```rust
pub fn password_hash(password: &str) -> String {
    hex::encode(sha2::Sha224::digest(password.as_bytes()))
}
```

## Request 格式

### TCP request

```
[56-byte password hash][CRLF][CMD_TCP][ATYP][ADDR][PORT][CRLF]
```

### UDP request

```
[56-byte password hash][CRLF][CMD_UDP][ATYP][ADDR][PORT][CRLF]
```

## 读写函数

```rust
pub async fn read_password(stream: &mut T) -> Result<String>
pub async fn write_password(stream: &mut T, password: &str) -> Result<()>
pub async fn read_request(stream: &mut T) -> Result<(u8, Address, u16)>
pub async fn write_request(stream: &mut T, cmd: u8, target: &Address, port: u16) -> Result<()>
```

## UDP Packet 格式

```
[ATYP][ADDR][PORT][2-byte length (big-endian)][PAYLOAD]
```

与 SOCKS5 UDP ASSOCIATE packet 格式兼容。
