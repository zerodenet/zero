---
layout: home
title: Zero
titleTemplate: 模块化网络代理内核

hero:
  name: Zero
  text: 模块化网络代理内核
  tagline: Rust 编写 · 模块化协议 · JSON 配置 · 多通道控制面
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
    details: 支持 SOCKS5、HTTP CONNECT、VLESS、Hysteria2、Shadowsocks、Trojan、VMess 和 Mieru；具体完成度以协议能力矩阵为准。
  - icon: 🧭
    title: 智能路由
    details: 域名、关键词、正则、IP CIDR、GEOIP、SNI、规则集、AND/OR 组合规则，支持远程规则集热加载。
  - icon: ⚖️
    title: 出站组
    details: Selector 手动切换、Fallback 自动降级、UrlTest 延迟选优、Relay 链式代理、LoadBalance 负载均衡，组间嵌套。
  - icon: 🔐
    title: 传输安全
    details: 提供 TLS、REALITY、WebSocket、HTTP/2、gRPC、XHTTP、QUIC 和多种协议加密能力。
  - icon: 🖥
    title: 多通道控制面
    details: 提供 HTTP JSON API、本地 IPC、SSE 事件流、Webhook、CLI，以及可选的 gRPC 适配器。
  - icon: 🪶
    title: 轻量高效
    details: 单二进制交付，无 GC 停顿；通过 Cargo feature 按需裁剪协议和控制面能力。
  - icon: 🌐
    title: DNS 子系统
    details: 内置 DNS 解析：System / UDP / DoH / DoT，域名路由，TTL 缓存，Fake IP 透明代理。
  - icon: 📡
    title: TUN 虚拟网卡
    details: 提供用户态网络栈和跨平台 TUN 抽象；平台能力与验证范围见 TUN 文档。
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
│ socks5 · http · mixed · vless │
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
