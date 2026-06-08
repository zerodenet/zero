# 协议总览

本文跟踪 Zero 当前实现的协议能力、配置入口和未完成部分。这里描述的是当前代码事实；机器可读事实以控制面 `capabilities.protocols` 为准。

## 状态含义

| 状态 | 含义 |
|------|------|
| `supported` | 作为常规内核能力使用，当前没有已知协议级缺口 |
| `partial` | 基线路径存在，但仍有互通、特殊传输、MUX 或服务端/客户端方向缺口 |
| `experimental` | 代码存在，但不能作为生产兼容能力假设 |
| `unsupported` | 当前方向未实现 |
| `not_applicable` | 协议本身不定义该方向 |

## 实现矩阵

| 协议 | 配置名 | 状态 | TCP inbound | TCP outbound | UDP inbound | UDP outbound | 说明 |
|------|--------|------|-------------|--------------|-------------|--------------|------|
| SOCKS5 | `socks5` | `supported` | 支持 | 支持 | 支持 | 支持 | TCP CONNECT、UDP ASSOCIATE、用户名密码认证 |
| HTTP CONNECT | `http_connect` | `supported` | 支持 | 不支持 | 不适用 | 不适用 | 仅作为入站 HTTP CONNECT |
| Mixed | `mixed` | `supported` | 支持 | 不支持 | 支持 | 不支持 | 入站复用器，不是外部协议；同端口识别 SOCKS5 和 HTTP CONNECT |
| VLESS | `vless` | `partial` | 支持 | 支持 | 部分支持 | 部分支持 | 基线 TCP 和 UDP-over-stream 可用，MUX UDP、部分 chain transport、TLS fingerprint 仍有限制 |
| Hysteria2 | `hysteria2` | `partial` | 支持 | 支持 | 部分支持 | 部分支持 | QUIC TCP stream 和 UDP datagram 基线路径存在，外部互通覆盖不足 |
| Shadowsocks | `shadowsocks` | `partial` | 支持 | 支持 | 支持 | 支持 | 常规 AEAD 已接近生产可用；AEAD 2022 仍有明确缺口 |
| Trojan | `trojan` | `partial` | 支持 | 支持 | 部分支持 | 部分支持 | TCP 和 UDP-over-stream 基线路径存在，外部互通与 relay-stream TLS fingerprint 不完整 |
| Mieru | `mieru` | `partial` | 实验 | 部分支持 | 部分支持 | 部分支持 | 单跳 TCP/UDP 基线路径存在，外部互通覆盖不足 |
| VMess | `vmess` | `experimental` | 实验 | 实验 | 不支持 | 不支持 | 不作为当前生产兼容目标；`cipher: auto` 不支持 |

## 内核动作

| 名称 | 状态 | 说明 |
|------|------|------|
| `direct` | `supported` | 内核直连动作；outbound TCP/UDP 均可用 |
| `block` | `supported` | 内核拒绝动作；TCP/UDP 均可作为路由结果 |
| `tun` | `supported` | 三层虚拟网卡入口，通过控制面或 CLI 启停，不写入静态 JSON `inbounds` |

## 配置入口

每个入站或出站协议都通过 `protocol.type` 选择。完整配置字段见 [配置速查](./configuration.md) 和 [配置规范](../project/config.md)。

```json
{
  "tag": "socks-in",
  "listen": { "address": "127.0.0.1", "port": 1080 },
  "protocol": { "type": "mixed" }
}
```

```json
{
  "tag": "proxy",
  "protocol": {
    "type": "shadowsocks",
    "server": "example.com",
    "port": 8388,
    "password": "your-secret-password",
    "cipher": "chacha20-ietf-poly1305"
  }
}
```

## 能力发现

GUI 和控制面消费者不应硬编码某个协议一定可用。启动后先查询 `capabilities.protocols`，再根据 `compiled`、方向字段和 `limitations` 决定展示哪些配置项。

文档页的职责：

- 本页说明当前实现了哪些协议。
- [配置速查](./configuration.md) 给出常见配置形状。
- [Shadowsocks](./shadowsocks.md) 跟踪 SS 的生产边界。
- [未完成项](./incomplete.md) 跟踪协议缺口。
