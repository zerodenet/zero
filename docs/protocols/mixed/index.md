# Mixed

`mixed` 是内核入站多路复用器，不是外部代理协议。

## 当前能力

| 能力 | 状态 | 说明 |
|------|------|------|
| TCP 入站 | `supported` | 自动识别 SOCKS5 CONNECT 和 HTTP CONNECT |
| UDP 入站 | `supported` | 使用 SOCKS5 UDP ASSOCIATE 路径 |
| 出站 | `unsupported` | `mixed` 不是出站协议 |

## 边界说明

GUI 客户端可将 `mixed` 作为默认本地入口暴露。检测完成后流量进入正常的 TCP 或 UDP pipe。Mixed 内部使用 SOCKS5 TCP CONNECT 和 UDP ASSOCIATE 运行时路径，HTTP CONNECT 使用 HTTP TCP 运行时路径。
