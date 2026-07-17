# Mixed 入站

Mixed 入站绑定一个 TCP 监听器，并根据连接前缀选择 SOCKS5 或 HTTP CONNECT 处理器。协议接受完成后，请求进入与单独 SOCKS5/HTTP 入站相同的路由和转发管线。

## 分流结果

| 客户端请求 | 执行路径 |
| --- | --- |
| SOCKS5 CONNECT | TCP 入站路由 |
| SOCKS5 UDP ASSOCIATE | UDP 关联生命周期 |
| HTTP CONNECT | TCP 入站路由 |
| 无法识别的前缀 | 协议错误，不进入路由 |

Mixed 只解决入口兼容问题。认证、目标地址解析和 UDP 封包仍由 SOCKS5 或 HTTP 的所有模块负责。
