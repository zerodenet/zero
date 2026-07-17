# Mieru 会话流程

## TCP CONNECT

```text
载体连接
  -> Mieru 握手与鉴权
  -> 隧道内 SOCKS5 CONNECT
  -> 返回协议 stream
  -> 通用 TCP 路由与 relay
```

## UDP ASSOCIATE

```text
载体连接
  -> Mieru 握手与鉴权
  -> 隧道内 UDP ASSOCIATE
  -> Mieru UDP session / codec
  -> 通用 UDP 流生命周期
```

`protocols/mieru` 拥有控制协商、数据帧和会话状态。`zero-proxy` 拥有路由决策、上游连接、转发、计量和错误归一化。这个边界防止通用运行时重新实现 Mieru 的 SOCKS5-in-tunnel 语义。
