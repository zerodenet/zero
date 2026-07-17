# Mieru 出站

Mieru 出站先打开载体连接，再调用 `protocols/mieru::tunnel` 完成隧道内控制协商。返回的 TCP stream 或 UDP session 是协议所有类型，代理层只做中立的转发与生命周期管理。

## TCP

1. 适配器准备 Mieru TCP 出站操作。
2. 运行时打开到上游的载体连接。
3. Mieru 协议模块完成隧道握手和 CONNECT 协商。
4. 运行时中继客户端与协议 stream。

## UDP

UDP ASSOCIATE 请求、响应、数据阶段加解密和数据包编解码均属于 `protocols/mieru::udp`。通用 UDP 运行时不持有其 cipher 或 session 状态。
