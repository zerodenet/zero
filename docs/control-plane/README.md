# 控制面文档

本目录包含 Zero 代理控制面的完整设计文档。

## 文档索引

| 文档 | 说明 |
| --- | --- |
| **[01-control-plane-roadmap.md](./01-control-plane-roadmap.md)** | 整体路线图、阶段划分、架构概览 |
| **[02-api-endpoints.md](./02-api-endpoints.md)** | HTTP API 端点规范、请求/响应格式 |
| **[03-http-adapter-design.md](./03-http-adapter-design.md)** | HTTP Adapter 详细设计、Handler 模式、中间件 |
| **[04-event-system.md](./04-event-system.md)** | 事件总线、事件格式、Sink 框架设计 |
| **[05-auth-and-permissions.md](./05-auth-and-permissions.md)** | 认证方式、权限模型、安全边界 |
| **[06-service-provider-integration.md](./06-service-provider-integration.md)** | 机场集成边界、内核能力 vs 上层业务 |
| **[07-node-heartbeat-and-push.md](./07-node-heartbeat-and-push.md)** | 节点主动上报、心跳机制、指令下发 |
| **[08-performance-and-rate-limiting.md](./08-performance-and-rate-limiting.md)** | 性能基准、限流分层架构、Hook扩展点 |

---

## 快速导航

### 核心设计决策

| 能力 | 实现位置 | 性能开销 | 文档 |
| --- | --- | --- | --- |
| 节点主动上报 | Connector | ~1% CPU | [07](./07-node-heartbeat-and-push.md) |
| 心跳保活 | Connector | 可忽略 | [07](./07-node-heartbeat-and-push.md) |
| 指令下发 | WebSocket长连接 | 可忽略 | [07](./07-node-heartbeat-and-push.md) |
| 总带宽/并发限流 | 内核 | <1% CPU | [08](./08-performance-and-rate-limiting.md) |
| 单IP/单用户限流 | 内核 | <1% CPU | [08](./08-performance-and-rate-limiting.md) |
| 设备数限制 | Connector + 中心 | ~5% CPU | [08](./08-performance-and-rate-limiting.md) |
| 积分/余额控制 | Hook + 计费系统 | ~5% CPU | [08](./08-performance-and-rate-limiting.md) |
| 共享订阅检测 | 中心控制面 | 0(在中心) | [08](./08-performance-and-rate-limiting.md) |

### 阶段路线图

```
Phase 1: 核心模型收敛 ✅
  └─ Query/Command/Event trait 定义

Phase 2: In-process Adapter ✅
  └─ EngineHandle 封装

Phase 3: HTTP JSON Adapter ✅
  └─ Router, Handlers, Auth Middleware

Phase 4: 事件 Sink 框架 ✅
  └─ FileSink, HttpSink, SinkManager

Phase 5: 基础限流 ✅
  └─ 带宽/并发/速率限制

Phase 6: Hook 扩展点 ✅
  └─ FlowHook ABI

Phase 7: Connector ✅
  └─ 主动上报, 心跳, 指令接收
```

### 核心能力

| 能力 | 状态 | 代码位置 |
| --- | --- | --- |
| Query API (13 种) | ✅ 已实现 | `crates/api/src/query.rs`、`crates/engine/src/api.rs` |
| Command API (11 种) | ✅ 已实现 | `crates/api/src/command.rs`、`crates/engine/src/api.rs` |
| 事件订阅 (SSE + IPC) | ✅ 已实现 | `src/http_adapter/sse.rs`、`src/ipc/connection.rs` |
| 事件投递 (File/Webhook/Memory/DeadLetter) | ✅ 已实现 | `crates/api/src/sink.rs`、`crates/connector/src/` |
| Bearer Token 认证 | ✅ 已实现 | `src/http_adapter/mod.rs` |
| 权限控制 (4 级) | ✅ 已实现 | `crates/api/src/auth.rs` |

---

## 阅读顺序

1. 先看 **路线图** 了解整体规划
2. 再看 **API 端点** 了解对外接口
3. 按实现顺序阅读各组件详细设计：
   - HTTP Adapter
   - 事件系统
   - 认证与权限
