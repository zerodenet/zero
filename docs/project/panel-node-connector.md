# 面板与节点连接器

本文档描述面板和 Zero 节点的对接方式。这里的“面板”是外部系统概念，不进入 `zero-engine`；节点只暴露通用 API 能力、事件和可选 connector。

## 边界

推荐分成两条独立链路：

- 节点到面板
  - 节点通过 `EventSource` 产生归一化事件。
  - 可选 `EventDispatcher` 把事件投递到配置的 `EventSink`。
  - 面板提供 HTTP/HTTPS webhook 接收事件。
- 面板到节点
  - 节点显式开启控制 API。
  - 面板通过 API key 调用只读查询或受控 command。
  - 默认不开启公网控制面。

内核不理解用户、套餐、余额、面板节点、设备限制等业务概念。面板需要的归因由 `principal_key`、`source_id` 和 `labels` 表达，最终账本、配额和策略由面板维护。

## 编译能力

面板对接能力必须是可选编译能力：

```text
event_dispatcher
  启用事件分发循环和 sink registry。

sink_jsonl
  启用本地 JSON Lines 事件落盘。

panel_connector
  启用 webhook 事件投递和 API key 控制入口，用于面板通讯。
```

不需要面板通讯时，不应编译 `panel_connector`。只需要本地观测时，使用 `event_dispatcher + sink_jsonl` 即可。

## 节点到面板

面板提供事件接收端：

```http
POST /api/zero/events
Authorization: Bearer <api-key>
Content-Type: application/json
```

请求体是 `zero-api` 归一化事件 envelope：

```json
{
  "schema_id": "zero.event.v1",
  "event_id": "flow.completed:42:1760000005000",
  "event_type": "flow.completed",
  "occurred_at_unix_ms": 1760000005000,
  "source_id": "edge-shanghai-01",
  "sequence": 42,
  "principal_key": "user:10001",
  "labels": {
    "tenant": "main"
  },
  "payload": {
    "flow_id": "42",
    "network": "tcp",
    "inbound": {
      "tag": "socks-in",
      "protocol": "socks5"
    },
    "target": {
      "host": "example.com",
      "port": 443
    },
    "traffic": {
      "bytes_up": 1024,
      "bytes_down": 4096
    },
    "outcome": "direct_relayed",
    "record": {
      "flow_id": "42",
      "revision": 8,
      "state": "completed",
      "network": "tcp",
      "inbound": { "tag": "socks-in", "protocol": "socks5" },
      "source": { "ip": "192.168.1.10", "port": 52864 },
      "target": { "host": "example.com", "port": 443, "resolved_ip": "203.0.113.10" },
      "route": {
        "mode": "rule",
        "action": "route",
        "target": "proxy",
        "matched_rule": { "index": 3, "condition": "domain: example.com" },
        "selection_chain": ["proxy", "edge-us"]
      },
      "path": {
        "outbound": { "tag": "edge-us", "protocol": "vmess" },
        "remote": { "host": "198.51.100.8", "port": 443 },
        "relay_chain": []
      },
      "traffic": { "bytes_up": 1024, "bytes_down": 4096 },
      "throughput": { "upload_bps": 0, "download_bps": 0, "sampled_at_unix_ms": 1760000005000 },
      "timing": {
        "started_at_unix_ms": 1760000000000,
        "last_activity_at_unix_ms": 1760000004900,
        "ended_at_unix_ms": 1760000005000,
        "duration_ms": 5000
      },
      "result": { "outcome": "chained_relayed", "close_reason": null, "failure": null }
    }
  }
}
```

面板处理规则：

- 使用 `event_id` 去重。
- 使用 `source_id + sequence` 检测乱序和断点。
- 返回 `2xx` 表示已接收。
- 返回 `429` 或 `5xx` 表示 connector 可以重试。
- 不应在事件里要求内核携带密码、私钥或完整敏感配置。

节点配置示例：

```json
{
  "api": {
    "event_sinks": [
      {
        "tag": "panel",
        "type": "webhook",
        "url": "https://panel.example.com/api/zero/events",
        "events": ["flow.completed", "engine.warning"],
        "source_id": "edge-shanghai-01",
        "api_key_env": "ZERO_PANEL_API_KEY"
      }
    ]
  }
}
```

`http://` 只应在本地或受控内网测试中使用，并需要显式 `allow_insecure = true`。

## 面板到节点

面板发起的节点通讯是 command/query 控制面。简化安全设计如下：

- 必须显式开启监听地址。
- 默认只建议监听 `127.0.0.1`、内网地址、Unix socket 或受防火墙保护的端口。
- 所有请求都携带 `Authorization: Bearer <api-key>`。
- API key 推荐从环境变量读取，不推荐写入明文配置。
- 写操作必须单独映射到 `CommandService`，不能直接操作 engine 内部结构。

首批可开放的能力：

```text
GET  /api/v1/capabilities
GET  /api/v1/health
GET  /api/v1/config
GET  /api/v1/runtime
GET  /api/v1/stats
GET  /api/v1/flows
GET  /api/v1/policies
GET  /api/v1/events
POST /api/v1/commands
```

`POST /api/v1/commands` 接收 `zero-api` 的 `CommandRequest` JSON。当前已接入：

```json
{
  "method": "policies.select",
  "params": {
    "policy_tag": "proxy",
    "target_tag": "direct"
  }
}
```

新增 command 时，需要先定义 `CommandRequest`，再由 transport adapter 暴露。面板只依赖通用 command/query 能力，不直接绑定 engine 内部结构。

## Connector 规则

重试队列、本地 spool 和 dead-letter 都属于 connector，不属于 `zero-engine`：

- `zero-engine` 只负责产出事件和快照。
- `EventDispatcher` 负责从事件源读取、过滤和分发。
- `EventSink` 负责投递到文件、webhook、gRPC 或自定义目标。
- retry、spool、dead-letter 是每个 connector/sink 的投递策略。

当前 connector 可以使用内存重试；需要持久化投递状态时由 sink 或外部 connector 自行维护。
