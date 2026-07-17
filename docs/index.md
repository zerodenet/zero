---
layout: home
title: Zero
titleTemplate: 模块化网络代理内核

hero:
  name: Zero
  text: 模块化网络代理内核
  tagline: 从首次运行、配置与协议，到控制面集成的工程文档
  actions:
    - theme: brand
      text: 快速开始
      link: /guides/quickstart
    - theme: alt
      text: 配置参考
      link: /project/config

features:
  - title: 部署与运行
    details: 从构建、最小配置和启动命令开始，再进入路由、出站组与热重载。
    link: /guides/quickstart
  - title: 协议能力
    details: 按协议查看入站、出站、TCP、UDP 与已知限制，不从文件数量推断完成度。
    link: /protocols/
  - title: 控制与集成
    details: 查阅 HTTP JSON API、本地 IPC、CLI、事件和 GUI 接入契约。
    link: /control-plane-api/
  - title: 架构与规范
    details: 了解 crate 责任边界、请求生命周期、协议能力模型和 ZRS 格式。
    link: /project/architecture
---

## 阅读路径

| 目标 | 起点 | 后续文档 |
| --- | --- | --- |
| 本地运行 Zero | [快速开始](/guides/quickstart) | [配置参考](/project/config) |
| 配置代理协议 | [协议概览](/protocols/) | [协议配置速查](/protocols/configuration) |
| 开发 GUI 或面板 | [控制与集成](/control-plane-api/) | [GUI 接入](/guides/gui-integration) |
| 修改内核 | [总体架构](/project/architecture) | [工程规则](/project/tooling) |
