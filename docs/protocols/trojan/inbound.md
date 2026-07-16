# Trojan Inbound

对应 `protocols/trojan/src/inbound.rs` — `TrojanInbound` 以及协议内部 accept/session route glue。

## TrojanInbound

实现 `InboundProtocol` trait，通过 `serve_inbound()` 统一管线。

### TCP 入站流程

1. TLS 握手完成（必须）
2. 读取 Trojan request：`[PASSWORD_HASH][CRLF][CMD][ATYP][ADDR][PORT][CRLF]`
3. 验证 password hash
4. 返回 `TrojanAccept { session }`

密码验证流程：
```rust
// read_password 读取到第一个 CRLF 为止
let password = shared::read_password(&mut stream).await?;
// 验证 SHA224(password) 是否匹配
if sha224(password) != expected_password_hash {
    return Err(TrojanError::AuthFailed);
}
```

## Inbound 配置

```json
{
  "tag": "trojan-in",
  "listen": { "address": "0.0.0.0", "port": 443 },
  "protocol": {
    "type": "trojan",
    "password": "your-password",
    "tls": {
      "cert_path": "certs/fullchain.pem",
      "key_path": "certs/privkey.pem"
    }
  }
}
```

- `password`: 必需
- `tls`: 必需，Trojan 必须使用 TLS
## 边界说明

Raw Trojan accept state is module-private.
Downstream glue should use `accept_session()`, `accept_client()`, or the route helpers
instead of depending on handshake-local accept models.
