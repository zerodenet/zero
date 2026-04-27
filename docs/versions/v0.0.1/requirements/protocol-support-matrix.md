# 协议支持

`v0.0.1` 是 `v0.0.x` 预发布线的一版，边界很窄。能用的只有下面这些。

| 名称 | 类型 | 方向 | 说明 |
|------|------|------|------|
| `socks5` | 外部协议 | 入站 | 支持 `no-auth`、username/password、`CONNECT` 和 `UDP ASSOCIATE` |
| `socks5` | 外部协议 | 出站 | 用于链式上游，支持 `no-auth` 和 username/password |
| `http-connect` | 外部协议 | 入站 | 只支持 `CONNECT` |
| `mixed` | 入站能力 | 入站 | 同端口多协议入站，当前识别 `socks5` 和 `http-connect` |
| `direct` | 内建动作 | 出站 | 直连 |
| `block` | 内建动作 | 出站 | 拒绝 |

不在这版里的：

- Shadowsocks
- Trojan
- VLESS
- VMess
- TUN

UDP 当前边界：

- 只支持通过 `socks5` 入站进入
- 支持 `direct` / `block`
- 支持经上游 `socks5` 的 UDP 转发
- 不支持 `http-connect` UDP
