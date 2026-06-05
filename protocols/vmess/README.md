# VMess

> 参照 Xray-core | Crate: `vmess`

VMess 是 V2Ray/Xray 项目的原始代理协议，使用 UUID + 时间戳进行身份验证，内置加密传输。

## 协议来源

| 项目 | 来源 |
|------|------|
| 参照实现 | [Xray-core](https://github.com/XTLS/Xray-core) |
| 协议形态 | VMess AEAD |
| 本实现 | `vmess` crate（stub，功能正在开发中） |

## 功能对齐状态

| 特性 | 状态 |
|------|------|
| 协议帧: AEAD 头部加密 | ⚠️ stub |
| TCP 入站 | ❌ |
| TCP 出站 | ❌ |
| UDP | ❌ |
| MUX | ❌ |
| AEAD 密码 | ❌ |

> VMess 当前为预留目录。项目优先完成 VLESS / Shadowsocks / Hysteria2 / Trojan 协议支持。

## 参考

- [Xray-core VMess AEAD](https://xtls.github.io/en/config/outbounds/vmess.html)
