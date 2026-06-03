# v0.0.7

VLESS QUIC/SplitHTTP、VMess 完整实现、gRPC 控制面、loadbalance 出站组、死信队列。

## 交付内容

- VLESS inbound/outbound QUIC 传输
- VLESS inbound/outbound SplitHTTP 传输
- VMess inbound（TLS / WebSocket / gRPC 传输）
- VMess outbound（TLS / WebSocket / gRPC 传输）
- gRPC 控制面通道（`grpc-api` feature）
- `loadbalance` 出站组类型（round-robin / random 策略）
- `dead_letter_path`：事件投递失败时的死信队列持久化
- Mieru inbound 注册适配器
- `sni` 路由条件
- 新增 workspace member：`crates/grpc`、`protocols/vmess`、`protocols/mieru`、`crates/dns`

## 不做什么

- 不做 VMess `cipher: auto` 兼容（明确拒绝）
- 不做 Mieru relay-chain 中间跳（仅单跳 TCP）
- 不做 HTTP CONNECT outbound
