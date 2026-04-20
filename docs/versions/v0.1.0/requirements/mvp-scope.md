# MVP 范围

`v0.1.0` 只做一件事：把最小 TCP 代理链路做完整。

## 本版要有

- `SOCKS5` 入站
- `HTTP CONNECT` 入站
- `mixed` 入站
- `direct`
- `block`
- `SOCKS5` 链式出站
- 域名和 CIDR 规则
- JSON 配置
- 基础日志
- 基础状态导出
- 优雅关闭

## 本版不做

- Shadowsocks
- Trojan
- VLESS
- VMess
- UDP
- TUN
- QUIC / WebSocket / DoH
- 热重载
- 完整管理 API
- GUI
