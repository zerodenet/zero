# Shadowsocks Outbound

对应 `protocols/shadowsocks/src/outbound.rs` — `ShadowsocksOutbound`、`ShadowsocksOutboundSession`、`ShadowsocksDatagramCodec`、UDP packet types。

## ShadowsocksOutbound

实现 `TcpSessionProtocol` trait：

1. 生成随机 salt（长度由 cipher 决定）
2. 派生 session key
3. 构造并写入 target address chunk
4. 返回 `ShadowsocksOutboundSession { key, nonce, cipher }`

Proxy runtime 负责 transport setup、routing、metering、session lifecycle、stats 和 events。

## ShadowsocksOutboundSession

携带 session 密钥材料，用于构建 `ShadowsocksAeadStream` 进行双向 chunk relay。

## ShadowsocksDatagramCodec

实现 `DatagramCodec` trait，支持 UDP datagram 的编码/解码：

- `encode(target, payload)` → 加密的 datagram（AEAD: per-packet salt + AEAD encrypt；2022: SIP022 header + AEAD encrypt）
- `decode(bytes)` → `(target, payload)`（AEAD: salt 提取 + AEAD decrypt；2022: header 解密 + payload decrypt）

## ShadowsocksUdpPacket / ShadowsocksUdpPacketTarget

用于 UDP relay chain 的中间表示。`UdpDatagramFraming` 在 protocol crate 内，proxy 只负责 socket/cache/response bridge。

## Outbound 配置示例

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

## 已验证路径

- SOCKS5 inbound → Shadowsocks outbound → Shadowsocks inbound → direct target (TCP)
- SOCKS5 UDP ASSOCIATE → Shadowsocks outbound → Shadowsocks inbound → UDP target
- Shadowsocks UDP relay chains over implemented packet-path carriers
- Large TCP payload crossing AEAD chunk boundaries
- Wrong-password TCP rejection
- All supported cipher names in in-tree TCP and UDP tests (6 ciphers)
- Local external UDP outbound interoperability against `shadowsocks-rust ssserver -U` for all cipher names
- SOCKS5 → Shadowsocks → Shadowsocks UDP relay chain (same-protocol)
- SOCKS5 → SOCKS5 → Shadowsocks UDP relay chain (cross-protocol via SOCKS5 carrier)
- Xray/sing-box/shadowsocks-rust external interop tests at `crates/proxy/tests/shadowsocks_xray_interop.rs` (local only, `#[ignore]`)
