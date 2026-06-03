# 版本文档

每个版本目录都要说明四件事：

- 这个版本要交付什么
- 这个版本明确不做什么
- 这个版本怎么推进
- 这个版本怎么验收

当前活跃线：

- `v0.0.x`

当前落点：

- `v0.0.8`

已发布版本：

- `v0.0.1` — 最小代理链路、本地/云端使用方式、UDP 和基础观测
- `v0.0.2` — 出站组能力（selector/fallback/urltest/relay）、事件骨架、VLESS 第一阶段
- `v0.0.3` — DNS 子系统、事件系统、认证强制执行、配置热重载、零默认配置路径
- `v0.0.4` — 入站架构重构、GCRA 速率限制、空闲超时、出站熔断、URL 域名重写、domain-regex、VMess AEAD / Trojan TCP、loadbalance 出站组、gRPC 控制面、direct 入站、TUN 虚拟网卡（三层截取、NetworkStack trait 抽象、UserTcpStack 状态机、跨平台 TunDevice）、SystemStack（OS 级流量重定向）
- `v0.0.5` — 协议正确性修复与真实节点兼容性：TCP 中继方向修正、Trojan TCP request 写入、Shadowsocks AEAD framing/KDF/outbound stream 包装、Mieru 单跳适配器/outbound stream、file logging guard/rotation 修复、VMess 兼容性限制明确标注
- `v0.0.6` — Shadowsocks 2022-blake3 AEAD outbound、DNS 子系统完善（DoH/DoT/Fake IP）、`domain-regex` / `sni` 路由条件、`validate` CLI 命令、`mode` 运行时热切换、TLS client fingerprint（chrome/firefox/safari/edge/ios/randomized）
- `v0.0.7` — VLESS QUIC / SplitHTTP 传输、VMess inbound/outbound（TLS/WS/gRPC）、gRPC 控制面（`grpc-api` feature）、`dead_letter_path` 死信队列、Mieru inbound、`sni` 路由条件、`loadbalance` 出站组（round-robin/random）
- `v0.0.8` — 稳定性修复：跨平台 docs build（移除平台特定 lockfile）、release workflow gate on docs

版本规则：

- `v0.0.x`：未定型推进线，只推进第三位
- `v0.1.0`：第一次正式发布版，达不到正式发布条件前不启用
