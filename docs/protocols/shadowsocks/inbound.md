# Shadowsocks Inbound

对应 `protocols/shadowsocks/src/inbound.rs` — `ShadowsocksInbound`、`ShadowsocksAccept`。

## ShadowsocksInbound

实现 `InboundProtocol` trait，接入 `serve_inbound()` 统一管线。

TCP 入站流程：

1. 读取 salt（salt 长度由 cipher 决定）
2. 派生 session key (HKDF-SHA1 for AEAD, Blake3 for AEAD 2022)
3. 用 session key 解密首 chunk 中的 address
4. 验证 cipher/password 匹配（错误密码 → 连接关闭）
5. 返回 `ShadowsocksAccept { session, remaining_payload }`

UDP 入站通过 `UdpPipe` 接入 dispatch 管线。

## ShadowsocksAccept

```rust
pub struct ShadowsocksAccept {
    pub session: Session,
    /// 首 chunk 解密后的剩余数据，直接进入 relay 而非丢弃
    pub remaining_payload: Vec<u8>,
}
```

零拷贝入口：`remaining_payload` 保存了解密首 chunk 后未被 address 解析消费的数据，这些数据直接进入 relay 阶段，避免额外拷贝。

## Inbound 配置示例

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

- `password`: 常规 AEAD 使用明文密码，AEAD 2022 使用 base64 编码的密钥材料
- `cipher`: 可选，见 [shared.md](shared.md) 支持的 cipher 列表
- `idle_timeout_secs`: 可选，空闲超时秒数（内核默认 300s）

## Per-user Rate Limits

通过 `Session::apply_auth()` 注入 `SessionAuth`，携带 per-user `up_bps`/`down_bps`。在 `accept()` 阶段应用到 session。
