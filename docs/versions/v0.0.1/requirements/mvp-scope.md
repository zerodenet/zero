# MVP 范围

`v0.0.1` 不按完整产品的标准定义。这版先收成 `v0.0.x` 预发布基线，只把最小 TCP 代理链路、最小 UDP 链路和基本可用性做扎实。

## 本版先满足两种使用方式

- 本地：用户在本机启动 `mixed` 监听后，浏览器、`curl` 或其他常见客户端能直接通过 `127.0.0.1:7890` 使用代理
- 云端：服务端启动 `socks5` 入站后，能作为远端节点被另一个 `zero` 或其他支持 `SOCKS5` 的客户端接入

## 本版必须具备

- 同端口多协议入站，当前至少覆盖 `SOCKS5 + HTTP CONNECT`

## 本版要有

- `SOCKS5` 入站
- `HTTP CONNECT` 入站
- `mixed` 入站
- `direct`
- `block`
- `SOCKS5` 链式出站
- `SOCKS5 UDP ASSOCIATE`
- `mode = rule | global | direct`
- `selector` 出站组
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
- TUN
- QUIC / WebSocket / DoH
- 热重载
- 完整管理 API
- GUI
