# Shadowsocks

Shadowsocks 是当前最接近生产使用的高级代理协议。这里按常规 AEAD 和 AEAD 2022 分开描述，避免把”能用”和”完整兼容”混在一起。

## 能力分级

### 基础（基本实现）

协议能跑通的最小功能集。满足日常代理使用的基本需求。

| 能力 | 状态 | 说明 |
|------|------|------|
| AEAD TCP inbound + outbound | ✅ 完成 | 3 个常规 cipher：`aes-128-gcm`、`aes-256-gcm`、`chacha20-ietf-poly1305` |
| AEAD UDP inbound + outbound | ✅ 完成 | Per-packet salt + AEAD，已覆盖 Zero 内部 UDP relay |
| AEAD stream wrapper | ✅ 完成 | `ShadowsocksAeadStream` 拥有 chunk 加解密、response salt 生成、download key 派生 |
| HKDF-SHA1 key derivation | ✅ 完成 | 常规 AEAD 的标准密钥派生 |
| 配置校验 + feature gate | ✅ 完成 | 密码校验、cipher 名称校验、未编译 feature 早期失败 |
| 统一 runtime 接入 | ✅ 完成 | InboundProtocol trait 接入 `serve_inbound()`，UDP 接入 `UdpPipe` |
| 运行时可观测 | ✅ 完成 | session 生命周期、统计、事件 |

### 完整（对标大众）

对标 `shadowsocks-rust` / SIP022 规范的常规特性。覆盖市面主流 SS 节点的使用场景。

| 能力 | 状态 | 说明 |
|------|------|------|
| AEAD 2022 cipher 支持 | ✅ 完成 | 3 个 2022 cipher：`2022-blake3-aes-128-gcm`、`2022-blake3-aes-256-gcm`、`2022-blake3-chacha20-poly1305` |
| Blake3 session key derivation | ✅ 完成 | AEAD 2022 的标准密钥派生 |
| SIP022 UDP packet format（outbound） | ✅ 完成 | AES 2022 header 加密/解密，XChaCha20-Poly1305 2022 变体 |
| Base64 password 校验 | ✅ 完成 | 2022 cipher 密码长度自动校验 |
| 冒号分隔 identity key 解析 | ✅ 完成 | 使用最后一段作为用户 PSK，不发送 EIH identity headers |
| Per-user rate limits | ✅ 完成 | 基于 `SessionAuth` 的 per-user `up_bps`/`down_bps` |
| 错误密码 TCP 拒绝 | ✅ 完成 | 密钥不匹配时连接关闭 |
| Large payload chunk 边界 | ✅ 完成 | 跨 AEAD chunk boundary 的大 payload 正确切分 |
| 外部 UDP 互通 | ✅ 完成 | 所有 6 个 cipher 通过 `shadowsocks-rust ssserver -U` 本地互通验证 |
| ⚠ AEAD 2022 TCP | ❌ 未完成 | 未实现 SIP022 TCP request/response header protocol |
| ⚠ AEAD 2022 UDP server response | ❌ 未完成 | 响应需要 session control state，当前无状态 codec 不能完整表达 |

### 特级（Zero 特色）

Zero 在对标基础上提供的独有能力，主流实现不具备。

| 能力 | 说明 |
|------|------|
| SS→SS 同协议 UDP relay chain | Shadowsocks UDP 通过 `DatagramCodec` 编码，由 `SsChainManager` 管理上游 socket 缓存和 response waiter 匹配 |
| SOCKS5→SS 跨协议 UDP relay chain | SOCKS5 UDP ASSOCIATE 作为 carrier，SS 作为 final-hop，通过通用 packet-path 模型组合 |
| 通用 `DatagramCodec` 抽象 | SS UDP 编解码实现为零成本 trait object，新增 carrier/inner protocol 组合只需实现两个 trait |
| `ShadowsocksAccept` 零拷贝入口 | Inbound accept 返回 `remaining_payload`，首包数据直接进入 relay 而非丢掉 |

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
  “type”: “shadowsocks”,
  “password”: “your-secret-password”,
  “cipher”: “chacha20-ietf-poly1305”
}
```

AEAD 2022 使用标准 base64 key material：

```json
{
  “type”: “shadowsocks”,
  “password”: “MDEyMzQ1Njc4OWFiY2RlZg==”,
  “cipher”: “2022-blake3-aes-128-gcm”
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
  “tag”: “ss-out”,
  “protocol”: {
    “type”: “shadowsocks”,
    “server”: “example.com”,
    “port”: 8388,
    “password”: “your-secret-password”,
    “cipher”: “chacha20-ietf-poly1305”
  }
}
```

## Inbound 示例

```json
{
  “tag”: “ss-in”,
  “listen”: { “address”: “0.0.0.0”, “port”: 8388 },
  “protocol”: {
    “type”: “shadowsocks”,
    “password”: “your-secret-password”,
    “cipher”: “chacha20-ietf-poly1305”
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
