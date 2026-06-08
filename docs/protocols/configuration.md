# 配置速查

本页只列协议相关的常见配置形状。完整字段、校验规则、路由、模式和出站组见 [配置规范](../project/config.md)。

## Inbound

### SOCKS5

```json
{
  "tag": "socks-in",
  "listen": { "address": "127.0.0.1", "port": 1080 },
  "protocol": {
    "type": "socks5",
    "users": [
      { "username": "alice", "password": "secret" }
    ]
  }
}
```

`users` 可省略。省略时为 no-auth。

### HTTP CONNECT

```json
{
  "tag": "http-in",
  "listen": { "address": "127.0.0.1", "port": 8080 },
  "protocol": { "type": "http_connect" }
}
```

### Mixed

```json
{
  "tag": "mixed-in",
  "listen": { "address": "127.0.0.1", "port": 1080 },
  "protocol": { "type": "mixed" }
}
```

`mixed` 是入站复用器：SOCKS5 TCP、SOCKS5 UDP ASSOCIATE 和 HTTP CONNECT 共用同一个监听端口。

### VLESS

```json
{
  "tag": "vless-in",
  "listen": { "address": "0.0.0.0", "port": 443 },
  "protocol": {
    "type": "vless",
    "users": [
      { "id": "11111111-2222-3333-4444-555555555555" }
    ],
    "tls": {
      "cert_path": "certs/fullchain.pem",
      "key_path": "certs/privkey.pem"
    }
  }
}
```

`tls`、`reality`、`ws`、`grpc`、`h2`、`http_upgrade` 和 `quic` 是可选传输配置。`reality` 不能和这些非 raw TCP 传输组合。

### Shadowsocks

```json
{
  "tag": "ss-in",
  "listen": { "address": "0.0.0.0", "port": 8388 },
  "protocol": {
    "type": "shadowsocks",
    "password": "your-secret-password",
    "cipher": "chacha20-ietf-poly1305"
  }
}
```

支持 cipher：

- `aes-128-gcm`
- `aes-256-gcm`
- `chacha20-ietf-poly1305`
- `2022-blake3-aes-128-gcm`
- `2022-blake3-aes-256-gcm`
- `2022-blake3-chacha20-poly1305`

AEAD 2022 的 `password` 必须是标准 base64 key material：AES-128 为 16 字节，AES-256 和 chacha20 为 32 字节。AES 2022 可以使用 `i_psk:u_psk` 形式，Zero 使用最后一段作为用户 PSK。

### Trojan

```json
{
  "tag": "trojan-in",
  "listen": { "address": "0.0.0.0", "port": 443 },
  "protocol": {
    "type": "trojan",
    "password": "your-secret-password",
    "tls": {
      "cert_path": "certs/fullchain.pem",
      "key_path": "certs/privkey.pem"
    }
  }
}
```

### Hysteria2

```json
{
  "tag": "hysteria2-in",
  "listen": { "address": "0.0.0.0", "port": 8443 },
  "protocol": {
    "type": "hysteria2",
    "password": "your-secret-password",
    "cert_path": "certs/fullchain.pem",
    "key_path": "certs/privkey.pem"
  }
}
```

### Mieru

```json
{
  "tag": "mieru-in",
  "listen": { "address": "0.0.0.0", "port": 2999 },
  "protocol": {
    "type": "mieru",
    "users": [
      { "username": "alice", "password": "secret" }
    ]
  }
}
```

Inbound TCP 在能力描述中仍标为实验。

## Outbound

### SOCKS5

```json
{
  "tag": "socks-out",
  "protocol": {
    "type": "socks5",
    "server": "127.0.0.1",
    "port": 1081,
    "username": "upstream",
    "password": "secret"
  }
}
```

`username` 和 `password` 必须同时配置或同时省略。

### VLESS

```json
{
  "tag": "vless-out",
  "protocol": {
    "type": "vless",
    "server": "example.com",
    "port": 443,
    "id": "11111111-2222-3333-4444-555555555555",
    "tls": {
      "server_name": "example.com"
    }
  }
}
```

### Shadowsocks

```json
{
  "tag": "ss-out",
  "protocol": {
    "type": "shadowsocks",
    "server": "example.com",
    "port": 8388,
    "password": "your-secret-password",
    "cipher": "chacha20-ietf-poly1305"
  }
}
```

AEAD 2022 password 规则与 inbound 相同。

### Trojan

```json
{
  "tag": "trojan-out",
  "protocol": {
    "type": "trojan",
    "server": "example.com",
    "port": 443,
    "password": "your-secret-password",
    "sni": "example.com"
  }
}
```

### Hysteria2

```json
{
  "tag": "hysteria2-out",
  "protocol": {
    "type": "hysteria2",
    "server": "example.com",
    "port": 443,
    "password": "your-secret-password"
  }
}
```

### Mieru

```json
{
  "tag": "mieru-out",
  "protocol": {
    "type": "mieru",
    "server": "example.com",
    "port": 2999,
    "password": "secret"
  }
}
```

`username` 可省略。省略时内核使用 `password` 作为 username。

### VMess

```json
{
  "tag": "vmess-out",
  "protocol": {
    "type": "vmess",
    "server": "example.com",
    "port": 443,
    "id": "11111111-2222-3333-4444-555555555555",
    "cipher": "aes-128-gcm"
  }
}
```

VMess 是实验能力。`cipher: auto` 不支持。

### Direct 和 Block

```json
{ "tag": "direct", "protocol": { "type": "direct" } }
```

```json
{ "tag": "block", "protocol": { "type": "block" } }
```

`direct` 和 `block` 是内核动作，不是外部协议。
