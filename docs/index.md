---
layout: home
title: Zero
titleTemplate: A modular network proxy kernel

hero:
  name: Zero
  text: 模块化网络代理内核
  tagline: Rust 编写 · 协议完备 · JSON 配置 · 多通道控制面
  actions:
    - theme: brand
      text: 快速上手
      link: /guides/quickstart
    - theme: alt
      text: 配置参考
      link: /project/config

features:
  - icon: 🔌
    title: 多协议支持
    details: SOCKS5、HTTP CONNECT、VLESS、Hysteria2、Shadowsocks、Trojan、VMess、Mieru — 全部原生实现，无外部依赖。
  - icon: 🧭
    title: 智能路由
    details: 域名、关键词、正则、IP CIDR、GEOIP、SNI、规则集、AND/OR 组合规则，支持远程规则集热加载。
  - icon: ⚖️
    title: 出站组
    details: Selector 手动切换、Fallback 自动降级、UrlTest 延迟选优、Relay 链式代理、LoadBalance 负载均衡，组间嵌套。
  - icon: 🔐
    title: 传输安全
    details: TLS、REALITY、AEAD cipher、2022-blake3 — 所有主流混淆与加密方案。
  - icon: 🖥
    title: 多通道控制面
    details: HTTP JSON API、gRPC、IPC (Unix Socket / Named Pipe)、SSE 事件流、Webhook 投递、CLI 一键操作。
  - icon: 🪶
    title: 轻量高效
    details: Rust 零成本抽象，单二进制，无 GC 停顿，内存占用极低。Feature gate 按需裁剪。
  - icon: 🌐
    title: DNS 子系统
    details: 内置 DNS 解析：System / UDP / DoH / DoT，域名路由，TTL 缓存，Fake IP 透明代理。
  - icon: 📡
    title: TUN 虚拟网卡
    details: 用户态 TCP 状态机，IPv4/IPv6 双栈，跨平台支持（Linux / macOS / Windows Wintun）。
---

## 一分钟跑起来

```bash
# 构建
cargo build --release

# 运行
./target/release/zero run config.json

# 管理
./target/release/zero status
./target/release/zero select proxy trojan-node
./target/release/zero mode global proxy
./target/release/zero validate config.json
```

## 架构

```
应用层 (浏览器 / curl / 系统代理)
  │
  ▼
┌───────────────────────────────────────┐
│ Inbound                               │
│ socks5 · http_connect · mixed · vless │
│ hysteria2 · shadowsocks · trojan      │
│ vmess · mieru · direct · tun          │
├───────────────────────────────────────┤
│ Router                                │
│ domain · ip · geoip · sni · keyword   │
│ regex · rule_set · and · or           │
├───────────────────────────────────────┤
│ Outbound Group                        │
│ selector · fallback · url_test        │
│ relay · load_balance                  │
├───────────────────────────────────────┤
│ Outbound                              │
│ direct · block · socks5 · vless       │
│ hysteria2 · shadowsocks · trojan      │
│ vmess · mieru                         │
└───────────────────────────────────────┘
  │                                      │
  ▼                                      ▼
┌───────────────┐   ┌──────────────────┐
│ Control API   │   │ Event Sinks      │
│ HTTP · gRPC   │   │ jsonl · webhook  │
│ IPC · CLI     │   │ push connector   │
└───────────────┘   └──────────────────┘
```
