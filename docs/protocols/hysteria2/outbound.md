# Hysteria2 出站

Hysteria2 出站以显式 TCP 和 UDP 能力注册。适配器准备连接或 UDP 流操作，运行时决定单跳、中继最终跳或 packet-path 的执行顺序。

## 数据路径

- TCP 请求在 QUIC stream 上完成 Hysteria2 会话后交给通用 relay。
- UDP 请求使用协议所有的 datagram codec 和复用状态。
- 通用 packet-path 运行时只保存中立载体描述、缓存标识和计量状态，不解析 Hysteria2 私有字段。

连接失败、协议失败和 UDP 流失败在运行时边界归一化，不由适配器静默回退。
