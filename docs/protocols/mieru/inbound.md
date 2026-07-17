# Mieru 入站

Mieru 入站完成载体接受、协议鉴权和隧道控制协商。控制请求确定后续是 TCP CONNECT 还是 UDP ASSOCIATE，然后把协议所有的 stream 或 UDP relay 交给通用入站路由。

## 责任划分

| 责任 | 所有者 |
| --- | --- |
| Mieru 握手、加解密和帧编解码 | `protocols/mieru` |
| 隧道内 SOCKS5 CONNECT / UDP ASSOCIATE 协商 | `protocols/mieru::tunnel` |
| 监听与连接任务生命周期 | 通用入站运行时 |
| 路由、转发与计量 | 通用代理运行时 |

代理适配器不持有 Mieru 密码学状态，也不自行构建或解析协议帧。
