# v0.0.6

Shadowsocks 2022-blake3、DNS 完善、路由条件扩展、validate/mode CLI、TLS fingerprint。

## 交付内容

- Shadowsocks 2022-blake3 AEAD outbound 支持
- DNS 子系统：DoH (DNS-over-HTTPS)、DoT (DNS-over-TLS)、Fake IP 透明代理
- `domain-regex` 路由条件（正则域名匹配）
- `sni` 路由条件（TLS ClientHello SNI 匹配）
- `validate` CLI 命令：校验配置文件有效性
- `mode` 运行时热切换：`zero mode rule|direct|global <outbound>`
- TLS client fingerprint：chrome / firefox / safari / edge / ios / randomized
- `mode` 配置从 `route` 内部提升为顶层字段

## 不做什么

- 不做 inbound/outbound 热增删（需重启）
- 不做第三方配置格式兼容
