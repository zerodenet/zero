# Zero 控制面 API

Zero 内核提供标准化的控制面，支持本地管理、远程上报和外部业务系统集成。所有能力通过四种通道暴露：**HTTP**、**gRPC**、**Unix Domain Socket / Windows Named Pipe**、**CLI**。

## 快速导航

| 文档 | 说明 |
|------|------|
| [configuration.md](./configuration.md) | `api.*` 配置模型完整参考 |
| [http-api.md](./http-api.md) | HTTP JSON 端点规范 |
| [ipc-protocol.md](./ipc-protocol.md) | UDS / Named Pipe 帧协议 |
| [events.md](./events.md) | 事件目录和 payload 规范 |
| [hooks.md](./hooks.md) | FlowHook 扩展点 |
| [push-connector.md](./push-connector.md) | 节点主动上报与远程命令 |
| [cli.md](./cli.md) | CLI 控制命令 |
| [contract.md](./contract.md) | API 契约和外部命名规则 |
| [GUI 接入指南](../guides/gui-integration.md) | 本地 GUI 的 IPC/HTTP 接入流程、状态模型和短期补齐建议 |

## 架构概览

```
┌─────────────────────────────────────────────────────┐
│                    GUI / CLI / 面板                    │
├──────────┬──────────┬──────────┬──────────┬─────────┐
│  HTTP    │  gRPC    │   UDS    │   CLI    │  Panel  │
│  :9090   │  :9091   │ .sock    │  zero    │Connector│
├──────────┴──────────┴──────────┴──────────┴─────────┤
│                   EngineHandle                       │
│  ┌──────────┐ ┌──────────┐ ┌──────────────────────┐ │
│  │  Query   │ │ Command  │ │     EventSource      │ │
│  │ Service  │ │ Service  │ │  (SSE / IPC / Sink)  │ │
│  └──────────┘ └──────────┘ └──────────────────────┘ │
├─────────────────────────────────────────────────────┤
│                      Engine                          │
│  ┌──────────┐ ┌──────────────┐ ┌─────────────────┐  │
│  │  Router  │ │ Session Reg  │ │   Event Log     │  │
│  └──────────┘ └──────────────┘ └─────────────────┘  │
└─────────────────────────────────────────────────────┘
```

## 四种通道对比

| 维度 | HTTP | gRPC | IPC (UDS/Pipe) | CLI |
|------|------|------|----------------|-----|
| 传输 | TCP | HTTP/2 | Unix Domain Socket / Named Pipe | UDS / Named Pipe |
| 认证 | Bearer token | Bearer token | 文件系统权限 (0600) | 文件系统权限 |
| 查询 | `GET /api/v1/*` | proto RPC | `{"type":"query","id":1,...}` | `zero status/flows/policies` |
| 命令 | `POST /api/v1/commands` | proto RPC | `{"type":"command","id":1,...}` | `zero select <p> <t>` |
| 事件流 | SSE (`text/event-stream`) | server streaming | JSON-line 推送 | `zero events` |
| 适用场景 | 远程调试、Web 面板 | 服务端集成、SDK | 本地 GUI 进程 | 终端管理 |
| 默认端口/路径 | 127.0.0.1:9090 | 127.0.0.1:9091 | `~/.zero/control.sock` / `\\.\pipe\zero-control` | 自动发现 |

## 核心设计原则

1. **内核通用** — API 不绑定任何特定面板或平台，所有消费者平等
2. **能力原语** — 暴露原子能力（查询、切换、关闭），业务逻辑在外部
3. **多通道一致** — HTTP、gRPC、IPC、CLI 四种通道共享相同的语义和数据模型
4. **安全后置** — 本地默认无认证（文件权限隔离），远程使用 Bearer token，mTLS 可选
5. **事件驱动** — 所有状态变更以归一化事件推送，支持 SSE、IPC 流、Sink 投递三种消费方式

## GUI 对接重点

本地 GUI 优先使用 IPC，HTTP 作为浏览器 WebView 或远程调试备选。GUI 首屏应先查询 `health`、`capabilities`、`config`、`runtime`，再建立事件订阅；重连后用 `runtime` / `stats` 重建界面状态。配置编辑应先走 `config.validate`，成功后再调用 `config.apply`。

短期对 GUI 最有价值的内核能力是机器可读契约、配置影响预检、结构化校验诊断、DNS/路由解释、日志流和更完整的 flow/policy 事件。这些能力应作为 Zero 内核通用控制面原语设计，不引入面板业务概念。

## 最小可用配置

```json
{
  "inbounds": [...],
  "outbounds": [...],
  "route": {...},
  "api": {
    "control": {
      "enabled": true,
      "listen": { "address": "127.0.0.1", "port": 9090 }
    }
  }
}
```

启动后即可通过 HTTP 或 IPC 访问控制面：

```bash
# HTTP
curl http://127.0.0.1:9090/api/v1/runtime

# CLI (自动连接 ~/.zero/control.sock)
zero status
zero select proxy direct
zero events
```
