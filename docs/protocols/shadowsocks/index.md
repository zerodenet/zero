# Shadowsocks

Shadowsocks 是当前最接近生产使用的高级代理协议。模块结构与 `protocols/shadowsocks/src/` 一一对应：

| 文档 | 对应源码 | 内容 |
|------|---------|------|
| [inbound.md](inbound.md) | `inbound.rs` | `ShadowsocksInbound`、`ShadowsocksAccept`、零拷贝入口、TCP/UDP 入站 |
| [outbound.md](outbound.md) | `outbound.rs` | `ShadowsocksOutbound`、`ShadowsocksOutboundSession`、UDP packet framing、配置示例 |
| [shared.md](shared.md) | `shared.rs` | `CipherKind`、密码规则、HKDF-SHA1 / Blake3 密钥派生、地址编码 |
| [stream.md](stream.md) | `stream.rs` | `ShadowsocksAeadStream`、chunk 加解密、response salt |
| [metadata.md](metadata.md) | `metadata.rs` | `ShadowsocksProtocol` 能力描述符、feature gate、limitations |

## 能力分级总览

### 基础（基本实现）

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

对标 `shadowsocks-rust` / SIP022 规范的常规特性。

| 能力 | 状态 | 说明 |
|------|------|------|
| AEAD 2022 cipher 支持 | ✅ 完成 | 3 个 2022 cipher |
| Blake3 session key derivation | ✅ 完成 | AEAD 2022 的标准密钥派生 |
| SIP022 UDP packet format（outbound） | ✅ 完成 | AES 2022 header 加密/解密 |
| Base64 password 校验 | ✅ 完成 | 2022 cipher 密码长度自动校验 |
| Per-user rate limits | ✅ 完成 | 基于 `SessionAuth` 的 per-user `up_bps`/`down_bps` |
| 错误密码 TCP 拒绝 | ✅ 完成 | 密钥不匹配时连接关闭 |
| Large payload chunk 边界 | ✅ 完成 | 跨 AEAD chunk boundary 的大 payload 正确切分 |
| 外部 UDP 互通 | ✅ 完成 | 所有 6 个 cipher 通过 `shadowsocks-rust ssserver -U` 本地互通验证 |
| AEAD 2022 TCP | ✅ 完成 | SIP022 TCP request/response header protocol；入站已通过 `sslocal` 端到端互操作验证，出站通过 Zero→Zero 验证 |
| AEAD 2022 UDP server response | ✅ 完成 | 回填客户端 SIP022 session id；已通过手动探针（DNS 往返）验证 |
| SIP022 3.1.3 检测防御 | ✅ 完成 | salt+固定头单次读取 + 失败时 drain |
| SIP022 3.1.5 服务端重放 salt 池 | ✅ 完成 | 60 秒滚动窗口（无 Bloom filter） |
| SIP022 3.2.4 UDP session-id 隔离 | ✅ 完成 | 按客户端 session id 隔离 UDP 中继流，不同客户端到同一 target 不复用出站流 |
| SIP022 3.2.4 UDP sliding window | ✅ 完成 | 每会话 packet id 重放过滤 |

### 特级（Zero 特色）

| 能力 | 说明 |
|------|------|
| SS→SS 同协议 UDP relay chain | `DatagramCodec` 编码，`SsChainManager` 管理上游 socket 缓存和 response waiter 匹配 |
| SOCKS5→SS 跨协议 UDP relay chain | SOCKS5 UDP ASSOCIATE 作为 carrier，SS 作为 final-hop |
| 通用 `DatagramCodec` 抽象 | 新增 carrier/inner protocol 组合只需实现两个 trait |
| `ShadowsocksAccept` 零拷贝入口 | Inbound accept 返回 `remaining_payload`，首包数据直接进入 relay |

## GUI 建议

| 选项 | 建议 |
|------|------|
| 常规 AEAD SS | 默认可选 |
| AEAD 2022 UDP outbound | 可选 |
| AEAD 2022 TCP outbound | 可选，标注 `shadowsocks_2022_hardening_not_externally_validated` |
| AEAD 2022 inbound server | 可选，标注 `shadowsocks_2022_hardening_not_externally_validated` |
| `capabilities.protocols[].limitations` | 必须展示或用于禁用高级选项 |
