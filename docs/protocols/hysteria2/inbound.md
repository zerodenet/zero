# Hysteria2 入站

Hysteria2 入站准备带鉴权的 QUIC profile。通用运行时负责绑定、QUIC 连接生命周期、关闭和任务回收；Hysteria2 模块负责鉴权、stream/datagram 协议语义和响应封装。

## 请求路径

| 请求 | 接受后路径 |
| --- | --- |
| TCP | 认证后的 QUIC stream 进入通用 stream route |
| UDP | QUIC datagram 解码后进入通用 UDP 流程 |

运行时通过中立 `AuthenticatedQuicInboundProfile` / `AuthenticatedQuicInboundConnection` 契约执行 QUIC 生命周期，不在通用模块中命名 Hysteria2 类型。
