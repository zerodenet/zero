# VMess

> 参照 Xray-core v25.3.1 | Crate: `vmess`

VMess 是 V2Ray/Xray 项目的原始代理协议，使用 UUID + 时间戳进行身份验证，内置加密传输。

## 版本追踪

| 项目 | 版本 |
|------|------|
| 参照实现 | [Xray-core](https://github.com/XTLS/Xray-core) v25.3.1 |
| 协议版本 | VMess AEAD（已完成对旧版 VMess MD5 的替代） |
| 本实现 | `vmess` crate v25.3.1（stub，功能正在开发中） |

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
