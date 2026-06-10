# SOCKS5 出站

对应 `protocols/socks5/src/outbound.rs` — 上游 SOCKS5 CONNECT 和 UDP ASSOCIATE 出站。

## TCP CONNECT 出站

实现 `TcpTunnelProtocol` trait：

1. 连接到上游 SOCKS5 服务器
2. 认证协商（无认证或用户名/密码）
3. 发送 CONNECT 请求
4. 验证响应
5. 返回已连接的流

## UDP ASSOCIATE 出站

1. TCP 控制连接到上游 SOCKS5 服务器
2. 认证协商
3. 发送 UDP ASSOCIATE 请求
4. 获取 relay 地址 + 端口
5. UDP 数据通过 relay 转发

## 配置示例

```json
{
  "tag": "socks5-upstream",
  "protocol": {
    "type": "socks5",
    "server": "proxy.example.com",
    "port": 1080,
    "username": "user",
    "password": "pass"
  }
}
```
