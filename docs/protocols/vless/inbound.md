# VLESS 入站

VLESS 入站负责传输请求准备、协议接受、用户鉴权和目标解析。接受完成后，TCP、UDP 和 MUX 请求分别进入通用入站路由边界。

## 责任划分

| 责任 | 所有者 |
| --- | --- |
| UUID、flow 与 VLESS 请求解析 | `protocols/vless` |
| TLS、REALITY、WebSocket、gRPC、HTTP/2、XHTTP、QUIC 载体 | `zero-transport` |
| 监听、接受循环、关闭与任务回收 | `zero-proxy` 通用入站运行时 |
| 接受后的 TCP、UDP 与 MUX 路由 | `zero-proxy` 通用路由管线 |

VLESS 适配器只准备协议所需的操作，不自行启动监听循环，也不保留完整 `Proxy` 对象。

## 数据路径

- TCP 请求进入通用 stream route。
- UDP-over-stream 请求通过协议所有的 relay 封装交给通用 UDP 路由。
- MUX TCP/UDP 子流通过中立的 MUX relay 契约交给运行时。
