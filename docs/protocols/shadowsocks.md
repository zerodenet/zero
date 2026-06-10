# Shadowsocks

Shadowsocks 是当前最接近生产使用的高级代理协议。这里按常规 AEAD 和 AEAD 2022 分开描述，避免把“能用”和“完整兼容”混在一起。

## 当前结论

| 范围 | 状态 | 说明 |
|------|------|------|
| 常规 AEAD TCP outbound | 生产候选 | 已覆盖本地运行时测试、large payload、错误密码关闭 |
| 常规 AEAD TCP inbound | 生产候选 | 已接入统一 TCP pipe 和 AEAD stream wrapper |
| 常规 AEAD UDP outbound | 生产候选 | 已覆盖 SOCKS5 UDP ASSOCIATE、Zero 内部中转、本地 `shadowsocks-rust ssserver -U` 互通 |
| 常规 AEAD UDP inbound | 生产候选 | 已覆盖 Zero 内部 UDP relay |
| AEAD 2022 UDP outbound | 互通已验证 | 本地验证通过 `shadowsocks-rust ssserver -U`，支持 AES 2022 和 XChaCha20Poly1305 2022 |
| AEAD 2022 TCP | 未完成 | 未实现 SIP022 TCP request/response header protocol |
| AEAD 2022 UDP inbound/server response | 未完成 | 响应包需要携带 client/session control state，当前无状态 datagram codec 不能完整表达 |

因此，`shadowsocks` 在 capability 中仍保持 `partial`。GUI 可以把常规 AEAD 作为可用 SS 节点能力展示，但不能把 AEAD 2022 当作完整 TCP+UDP 双向生产能力。

## 支持的 cipher

常规 AEAD：

- `aes-128-gcm`
- `aes-256-gcm`
- `chacha20-ietf-poly1305`

AEAD 2022：

- `2022-blake3-aes-128-gcm`
- `2022-blake3-aes-256-gcm`
- `2022-blake3-chacha20-poly1305`

## Password 规则

常规 AEAD 使用普通 Shadowsocks password：

```json
{
  "type": "shadowsocks",
  "password": "your-secret-password",
  "cipher": "chacha20-ietf-poly1305"
}
```

AEAD 2022 使用标准 base64 key material：

```json
{
  "type": "shadowsocks",
  "password": "MDEyMzQ1Njc4OWFiY2RlZg==",
  "cipher": "2022-blake3-aes-128-gcm"
}
```

校验规则：

| Cipher | `password` 解码长度 |
|--------|----------------------|
| `2022-blake3-aes-128-gcm` | 16 字节 |
| `2022-blake3-aes-256-gcm` | 32 字节 |
| `2022-blake3-chacha20-poly1305` | 32 字节 |

AES 2022 可接受冒号分隔的 identity key 链。Zero 使用最后一段作为用户 PSK，不发送 EIH identity headers。

## Outbound 示例

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

## Inbound 示例

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

## 已验证路径

- SOCKS5 inbound -> Shadowsocks outbound -> Shadowsocks inbound -> direct target
- SOCKS5 UDP ASSOCIATE -> Shadowsocks outbound -> Shadowsocks inbound -> UDP target
- Shadowsocks UDP relay chains over implemented packet-path carriers (SOCKS5 UDP ASSOCIATE and Shadowsocks UDP)
- Large TCP payload crossing AEAD chunk boundaries
- Wrong-password TCP rejection
- All supported cipher names in in-tree TCP and UDP tests (6 ciphers: 3 AEAD + 3 AEAD 2022)
- Local external UDP outbound interoperability against `shadowsocks-rust ssserver -U` for all supported cipher names
- SOCKS5 -> Shadowsocks -> Shadowsocks UDP relay chain (same-protocol)
- SOCKS5 -> SOCKS5 -> Shadowsocks UDP relay chain (cross-protocol via SOCKS5 carrier)

## 未完成实现

### AEAD 2022 TCP

当前 TCP 2022 仍使用现有 AEAD stream wrapper，没有实现 SIP022 的 TCP request/response header protocol。结果是 Zero 内部测试可以自洽，但不能宣称与外部 AEAD 2022 TCP 实现完整互通。

完成标准：

- 实现 AEAD 2022 TCP request header
- 实现 AEAD 2022 TCP response header
- 区分常规 AEAD TCP chunking 与 AEAD 2022 TCP header/chunking
- 与 `shadowsocks-rust` 做本地 TCP 外部互通测试

### AEAD 2022 UDP inbound/server response

AEAD 2022 UDP server response 不是简单的 `target + payload` 加密。响应包需要 server session id、client session id、packet id、timestamp 等 control state。当前 `UdpDatagramFraming` 是无状态接口，适合普通 AEAD datagram，但不能完整表达 AEAD 2022 server response。

完成标准：

- 为 Shadowsocks UDP inbound 保存 AEAD 2022 client control state
- 将 response context 传入响应编码
- 对 AES 2022 response header 做正确加密
- 与 `shadowsocks-rust` 做 Zero 作为 UDP server 的外部互通测试

## GUI 建议

GUI 可按以下方式展示：

| 选项 | 建议 |
|------|------|
| 常规 AEAD SS | 默认可选 |
| AEAD 2022 UDP outbound | 可选，但标注 TCP 2022 未完整 |
| AEAD 2022 inbound server | 不建议作为完整对外服务能力展示 |
| `capabilities.protocols[].limitations` | 必须展示或用于禁用高级选项 |
