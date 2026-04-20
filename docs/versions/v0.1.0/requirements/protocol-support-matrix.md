# 协议支持

`v0.1.0` 的边界很窄。能用的只有下面这些。

| 名称 | 类型 | 方向 | 说明 |
|------|------|------|------|
| `socks5` | 外部协议 | 入站 | 支持 `no-auth` 和 `CONNECT` |
| `socks5` | 外部协议 | 出站 | 只用于链式上游 |
| `http-connect` | 外部协议 | 入站 | 只支持 `CONNECT` |
| `mixed` | 入站能力 | 入站 | 同端口识别 `socks5` 和 `http-connect` |
| `direct` | 内建动作 | 出站 | 直连 |
| `block` | 内建动作 | 出站 | 拒绝 |

不在这版里的：

- Shadowsocks
- Trojan
- VLESS
- VMess
- UDP
- TUN
