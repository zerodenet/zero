# HTTP CONNECT

HTTP CONNECT 是 Zero 中的稳定 TCP 入站协议。模块结构与 `protocols/http-connect/src/` 对应。

## 当前能力

| 能力 | 状态 | 说明 |
|------|------|------|
| TCP 入站 | `supported` | HTTP CONNECT 隧道入口 |
| TCP 出站 | `unsupported` | 未作为上游代理协议实现 |
| UDP | `not_applicable` | HTTP CONNECT 无 UDP ASSOCIATE 模型 |

## 边界说明

HTTP CONNECT 不提供加密、UDP 或多路复用语义，仅为面向客户端的 TCP 隧道入口。
