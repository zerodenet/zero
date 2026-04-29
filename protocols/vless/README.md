# zero-protocol-vless

VLESS 协议实现。

当前阶段只实现 TCP，TLS 由 `zero-proxy` 传输层包裹：

- 入站：UUID 用户校验、TCP command、IPv4/domain/IPv6 目标地址
- 出站：向上游 VLESS TCP 节点建立隧道，可走 TLS 上游连接
- 观测归因：入站用户可配置 `credential_id` 和 `principal_key`

暂不包含 UDP、MUX、XTLS/Reality、WebSocket、gRPC 等传输形态。
