# Zero 控制面 API

本目录包含 Zero 控制面 API 的完整规范。API 通过四种传输通道提供对内核状态、配置、诊断和事件的标准化访问：HTTP、gRPC、IPC（Unix Domain Socket / Windows Named Pipe）和 CLI。

## 入口

从 **[index.md](./index.md)** 开始，了解架构概览、四种通道对比、核心设计原则和最小可用配置。

## 文档

| 文档 | 说明 |
|----------|-------------|
| [index.md](./index.md) | 架构概览、四种通道对比、设计原则和快速入门 |
| [configuration.md](./configuration.md) | `api.*` 配置模型参考 |
| [http-api.md](./http-api.md) | HTTP JSON 端点规范 |
| [ipc-protocol.md](./ipc-protocol.md) | 本地 IPC 的 UDS / Named Pipe 帧协议 |
| [events.md](./events.md) | 事件目录和负载规范 |
| [hooks.md](./hooks.md) | FlowHook 扩展点规范 |
| [push-connector.md](./push-connector.md) | 节点主动上报和远程命令下发 |
| [cli.md](./cli.md) | CLI 控制命令参考 |
| [contract.md](./contract.md) | API 契约、命名规范和外部集成规则 |

## 给 GUI 开发者

推荐的 IPC 优先集成工作流、启动查询序列、状态模型、连接生命周期和平台相关代码示例（Python、Node.js/Electron），请参阅 [GUI 接入指南](../guides/gui-integration.md)。
