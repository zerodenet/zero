# 机场面板接入指南

本文面向需要把 Zero 作为节点内核接入机场面板的开发者，给出节点在线状态、用户流量计费和远程运维的最小闭环。

Zero 只提供代理内核与通用控制面，不管理用户、套餐、余额、订单、订阅链接或设备数量。这些业务对象由面板维护，并通过稳定标识映射到 Zero 的 `node_id`、`source_id` 和 `principal_key`。

## 集成架构

```text
订阅/用户系统 ──生成凭据和节点配置──> Zero Node
                                      │
                ┌─────────────────────┼─────────────────────┐
                │                     │                     │
                ▼                     ▼                     ▼
          PushConnector         EventDispatcher       Control API
          心跳/远程命令          flow.completed        查询/受控命令
                │                 Webhook                   │
                └─────────────────────┼─────────────────────┘
                                      ▼
                                  机场面板
```

三条链路职责不同：

| 需求 | Zero 能力 | 面板用途 |
|------|-----------|----------|
| 节点在线状态 | 顶层 `push` / PushConnector | 心跳、活跃连接数、节点累计流量和远程命令 |
| 用户流量计费 | `api.event_sinks` Webhook | 持久消费 `flow.completed`，按用户聚合最终字节数 |
| 运维查询 | `api.control` | 查询健康、运行状态、策略和 Sink 投递状态 |
| 订阅、套餐和支付 | 面板自身能力 | 生成客户端配置、管理账户和商业规则 |

GUI 实时连接不经过 PushConnector。GUI 使用 IPC/HTTP/gRPC 的 `EventSource`；机场面板的计费事件由 EventDispatcher 投递。

## 1. 构建节点

机场面板闭环需要 `panel_connector`。该 feature 会启用 PushConnector、Webhook Event Sink 和事件分发能力：

```bash
cargo build --release --features full,status_api,panel_connector
```

如果还需要节点本地 JSONL 审计文件，额外启用 `sink_jsonl`：

```bash
cargo build --release --features full,status_api,panel_connector,sink_jsonl
```

未编译相应 feature 却配置 `push` 或 Webhook 时，Zero 会在启动阶段明确报错，不会静默忽略。

## 2. 建立面板标识映射

面板至少维护以下映射：

| 面板对象 | Zero 字段 | 建议值 |
|----------|-----------|--------|
| 节点 | `push.node_id` | `edge-shanghai-01` |
| 事件来源 | Sink `source_id` | 与 `node_id` 相同 |
| 用户/账户 | 入站用户 `principal_key` | `account:10001` |
| 投递幂等键 | 事件 `event_id` | 原样保存并建立唯一索引 |

`principal_key` 应是稳定、非敏感的面板账户 ID。不要使用密码、UUID 凭据原文或会随套餐变化的显示名称作为计费主键。

以下 SOCKS5 入站把认证用户映射到面板账户：

```json
{
  "tag": "panel-users",
  "listen": { "address": "0.0.0.0", "port": 1080 },
  "protocol": {
    "type": "socks5",
    "users": [
      {
        "username": "user-10001",
        "password": "replace-with-generated-secret",
        "principal_key": "account:10001"
      }
    ]
  }
}
```

VLESS 和 VMess 多用户入站同样支持 `credential_id`、`principal_key`、`up_bps` 和 `down_bps`。面板生成新配置后，应先运行 `zero validate`，再通过受控部署流程替换节点配置并 reload。

## 3. 配置面板连接

将以下片段合入节点配置。示例使用同一个面板接收密钥处理心跳与事件，控制 API 使用独立密钥：

```json
{
  "api": {
    "control": {
      "enabled": true,
      "listen": { "address": "127.0.0.1", "port": 9090 },
      "api_key_env": "ZERO_NODE_CONTROL_KEY"
    },
    "event_sinks": [
      {
        "type": "webhook",
        "tag": "panel-billing",
        "url": "https://panel.example.com/api/zero/events",
        "events": ["flow.completed"],
        "source_id": "edge-shanghai-01",
        "api_key_env": "ZERO_PANEL_API_KEY"
      }
    ],
    "dead_letter_path": "zero-panel-dead-letter.jsonl"
  },
  "push": {
    "url": "https://panel.example.com",
    "node_id": "edge-shanghai-01",
    "api_key_env": "ZERO_PANEL_API_KEY",
    "heartbeat_interval_seconds": 30,
    "pull_commands": true,
    "command_poll_interval_seconds": 10
  }
}
```

`api_key` 与 `api_key_env` 二选一，不能同时配置。生产环境应使用 HTTPS 和环境变量：

```bash
export ZERO_PANEL_API_KEY='replace-with-panel-key'
export ZERO_NODE_CONTROL_KEY='replace-with-node-control-key'
./target/release/zero validate config.json
./target/release/zero run config.json
```

PowerShell：

```powershell
$env:ZERO_PANEL_API_KEY = 'replace-with-panel-key'
$env:ZERO_NODE_CONTROL_KEY = 'replace-with-node-control-key'
./target/release/zero.exe validate config.json
./target/release/zero.exe run config.json
```

控制 API 示例只监听 localhost。如果面板必须主动访问节点，应使用内网、VPN 或反向代理，并限制来源地址；不要直接把无额外网络隔离的控制端口暴露到公网。

## 4. 实现面板端点

面板至少实现三个 HTTP 端点：

| Method | Path | 用途 | 成功响应 |
|--------|------|------|----------|
| `POST` | `/api/v1/nodes/{node_id}/heartbeat` | 接收节点心跳，可在响应中嵌入命令 | `{"ok":true}` |
| `GET` | `/api/v1/nodes/{node_id}/commands` | 返回待执行命令；仅 `pull_commands=true` 使用 | `[]` |
| `POST` | `/api/zero/events` | 接收 `ApiEvent` 事件信封 | 任意 `2xx` |

三个端点都应验证：

```http
Authorization: Bearer <ZERO_PANEL_API_KEY>
```

### 心跳

节点发送：

```json
{
  "node_id": "edge-shanghai-01",
  "build_id": "0.0.16-dev",
  "uptime_seconds": 3600,
  "active_flows": 42,
  "bytes_up": 1024000,
  "bytes_down": 5120000
}
```

面板更新节点的 `last_seen`、构建版本和运行指标，然后返回：

```json
{
  "ok": true,
  "commands": [
    {
      "method": "policies.select",
      "params": { "policy_tag": "proxy", "target_tag": "server-b" }
    }
  ]
}
```

心跳中的 `bytes_up` / `bytes_down` 是节点累计运行指标，可能因重启归零，也可能因重试重复上报。它适合监控，不应直接作为用户计费增量。

### 计费事件

Webhook 接收的是完整 `ApiEvent` 信封。计费核心字段如下：

```json
{
  "schema_id": "zero.event.v1",
  "event_id": "flow.completed:42:1760000005000",
  "event_type": "flow.completed",
  "source_id": "edge-shanghai-01",
  "sequence": 42,
  "principal_key": "account:10001",
  "payload": {
    "flow_id": "42",
    "auth": { "principal_key": "account:10001" },
    "traffic": { "bytes_up": 1024, "bytes_down": 4096 },
    "record": {
      "state": "completed",
      "traffic": { "bytes_up": 1024, "bytes_down": 4096 }
    }
  }
}
```

面板处理顺序：

1. 验证 Bearer token、`schema_id` 和 `event_type`。
2. 以 `event_id` 写入唯一索引；重复事件直接返回 `2xx`，不得重复计费。
3. 以 `principal_key` 定位账户；缺失或未知时进入人工核对队列。
4. 优先读取 `payload.record.traffic`，兼容旧内核时回退到 `payload.traffic`。
5. 在同一数据库事务中保存原始事件并累加 `bytes_up + bytes_down`。
6. 事务提交后才返回 `2xx`。

Webhook 返回 `429` 或 `5xx` 时会进入有界重试；其他 `4xx` 视为不可重试。重试耗尽后事件写入 `dead_letter_path`。当前内置队列不是持久 outbox，面板应监控 Sink 状态并建立死信补录流程。

如果 Sink 配置了事件白名单，`sequence` 会跳过未投递的其他事件类型，因此只能用于排序和诊断，不能要求每个相邻数值都连续。

## 5. 验收

1. 启动节点，确认面板在两个心跳周期内更新 `last_seen`。
2. 使用配置的用户凭据建立并关闭一条代理连接。
3. 确认 `/api/zero/events` 收到一次 `flow.completed`，且 `principal_key` 正确。
4. 用同一个 `event_id` 重放请求，确认账户流量没有再次增加。
5. 临时让 Webhook 返回 `500`，确认 Sink 失败计数增长并发生重试。
6. 恢复 `2xx` 后检查状态：

```bash
curl -H "Authorization: Bearer ${ZERO_NODE_CONTROL_KEY}" \
  http://127.0.0.1:9090/api/v1/sinks
```

7. 返回一条 `policies.select` 命令，确认节点日志记录执行结果。

## 当前边界

- Zero 不提供机场用户、套餐、支付、订阅链接或节点库存 API。
- PushConnector 不消费 flow 事件，也不负责用户计费。
- PushConnector 不接受远程 `config.validate` / `config.apply`，完整配置发布应走独立部署流程。
- 远程命令执行结果当前写入节点日志，不回传命令结果端点。
- `flow.completed` 是计费最终事实；`flow.snapshot` 只用于实时客户端同步，不投递到 Webhook。

## 相关文档

- [Push Connector 协议](../control-plane-api/push-connector.md)
- [事件目录与 flow.completed](../control-plane-api/events.md#flow-completed)
- [控制面配置](../control-plane-api/configuration.md)
- [HTTP 控制 API](../control-plane-api/http-api.md)
- [兼容性与破坏性变更](../control-plane-api/breaking-changes.md)
