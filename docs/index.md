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
      link: /guide/quickstart
    - theme: alt
      text: 配置参考
      link: /project/config

features:
  - icon: 🔌
    title: 多协议支持
    details: SOCKS5、HTTP CONNECT、VLESS、Hysteria2、Shadowsocks、Trojan — 全部原生实现，无外部依赖。
  - icon: 🧭
    title: 智能路由
    details: 域名匹配、IP CIDR、GEOIP、关键词、AND/OR 组合规则，支持规则集文件热加载。
  - icon: ⚖️
    title: 出站组
    details: Selector 手动切换、Fallback 自动降级、UrlTest 延迟选优，组间嵌套。
  - icon: 🔐
    title: 传输安全
    details: TLS、REALITY、AEAD cipher、端口跳跃——所有主流混淆与加密方案。
  - icon: 🖥
    title: 多通道控制面
    details: HTTP JSON API、Unix Domain Socket IPC、SSE 事件流、Webhook 投递，CLI 一键操作。
  - icon: 🪶
    title: 轻量高效
    details: Rust 零成本抽象，单二进制，无 GC 停顿，内存占用极低。
---

## 当前版本

**v0.1.0** (pre-release) — TCP 全协议支撑，SOCKS5 UDP，selector/fallback/urltest 出站组，控制面完整。

## 一分钟跑起来

```bash
# 下载
curl -L -o zero https://github.com/zerodenet/zero/releases/latest/download/zero-linux-amd64
chmod +x zero

# 运行
./zero run examples/v0.1.0/client-full.json

# 管理
./zero status
./zero select proxy trojan-node
```

## 架构

```
应用 (浏览器 / curl / 系统代理)
  │
  ▼
┌─────────────────────────┐
│  Inbound                 │  socks5 · http-connect · mixed · vless · hysteria2 · ss · trojan
├─────────────────────────┤
│  Router                  │  domain · ip · geoip · keyword · rule-set · and · or
├─────────────────────────┤
│  Outbound Group          │  selector · fallback · urltest
├─────────────────────────┤
│  Outbound                │  direct · block · socks5 · vless · hysteria2 · ss · trojan
└─────────────────────────┘
  │                       │
  ▼                       ▼
┌──────────────┐  ┌────────────────┐
│  Control API  │  │  Event Sinks   │
│  HTTP · IPC   │  │  jsonl · webhook│
└──────────────┘  └────────────────┘
```
