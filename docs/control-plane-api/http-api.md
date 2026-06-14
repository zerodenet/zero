# HTTP JSON API

## 基础信息

- 稳定前缀：`/api/v1/`
- 认证：`Authorization: Bearer <token>` 或 `X-Zero-Api-Key: <token>`（未配置时无认证模式，默认所有权限）
- CORS：所有端点返回 `Access-Control-Allow-Origin: *`
- 限流：Query 100/s，Command 10/s，SSE 5 并发

## 通用响应格式

HTTP 和 IPC 共享相同的响应信封格式（定义在 `zero_api::ApiResponse`）。`api_id` 字段始终存在，用于协议标识。

成功：
```json
{
  "api_id": "zero.api.v1",
  "ok": true,
  "result": { }
}
```

失败：
```json
{
  "api_id": "zero.api.v1",
  "ok": false,
  "error": {
    "code": "not_found",
    "message": "Policy not found",
    "field_path": "policy_tag"
  }
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `api_id` | string | 协议标识，始终为 `"zero.api.v1"` |
| `id` | string \| number \| null | 请求关联 ID，不透明原样回显（客户端可用任意标量作配对令牌：数字、字符串如 UUID/标签）；IPC 多路复用时使用，HTTP 通常为 null |
| `ok` | bool | 成功标志 |
| `result` | object? | 成功时的响应数据 |
| `error.code` | string | 机器可读错误码（snake_case） |
| `error.message` | string | 人类可读错误信息 |
| `error.field_path` | string? | 参数校验错误时的字段路径 |

> **HTTP 和 IPC 的 `result` 格式不同**：HTTP 的 `result` 直接包含端点数据（如 `{engine_build_id:"build-id",...}`）。IPC 的 `result` 包含一个变体名 key 包裹（如 `{"health":{engine_build_id:"build-id",...}}`）。详见 [ipc-protocol.md](./ipc-protocol.md)。

错误码（snake_case，与 JSON serde 格式一致）：

| code | HTTP | 说明 |
|------|------|------|
| `not_found` | 404 | 资源不存在 |
| `invalid_argument` | 400 | 参数无效 |
| `permission_denied` | 403 | 权限不足 |
| `feature_disabled` | 501 | 功能未编译 |
| `conflict` | 409 | 状态冲突 |
| `unsupported` | 501 | 不支持的操作 |
| `internal` | 500 | 内部错误 |

---

## Query 端点

### GET /api/v1/capabilities

API 能力列表。

```json
{
  "api_id": "zero.api.v1",
  "schema_id": "zero.event.v1",
  "adapters": [{ "kind": "in_process", "enabled": true }],
  "sinks": [{ "kind": "none", "enabled": false }],
  "features": ["query", "config_snapshot", "runtime_snapshot", "flow_snapshot", "policy_snapshot"],
  "protocols": [
    {
      "protocol": "socks5",
      "feature": "socks5",
      "compiled": true,
      "status": "supported",
      "compatibility_baseline": "rfc_1928_rfc_1929",
      "inbound": {
        "tcp": { "supported": true, "level": "supported", "notes": [] },
        "udp": { "supported": true, "level": "supported", "notes": [] }
      },
      "outbound": {
        "tcp": { "supported": true, "level": "supported", "notes": [] },
        "udp": { "supported": true, "level": "supported", "notes": [] }
      },
      "transports": ["tcp"],
      "mux": { "supported": false, "level": "not_applicable", "notes": [] },
      "limitations": []
    }
  ],
  "build_features": ["status_api", "socks5", "http_connect", "mixed", "vless"],
  "permissions": ["read"]
}
```

`protocols` 是供 GUI 和外部控制面消费者使用的机器可读协议矩阵。`zero-api` 定义此线路模型；代理运行时会从当前二进制的编译协议清单中填充该字段。当前 TCP/UDP 能力模型和限制码详见 [protocol-capabilities.md](../project/protocol-capabilities.md)。

### GET /api/v1/health

进程健康状态。

```json
{
  "engine_build_id": "build-id",
  "started_at_unix_ms": 1713500000000,
  "healthy": true
}
```

### GET /api/v1/config

当前配置快照。所有类型定义在 `zero-api::snapshot` 模块，外部消费者可直接依赖 `zero-api` crate。

```json
{
  "mode": { "kind": "rule", "outbound": null },
  "rule_count": 5,
  "listeners": [
    { "tag": "socks-in", "protocol": "socks5", "listen_address": "0.0.0.0", "listen_port": 1080 }
  ],
  "outbounds": [
    { "tag": "direct", "protocol": "direct", "server": null, "port": null },
    { "tag": "proxy", "protocol": "vless", "server": "1.2.3.4", "port": 443 }
  ],
  "outbound_groups": [
    {
      "tag": "auto",
      "kind": "url_test",
      "outbounds": ["server-a", "server-b"],
      "selected": "server-a",
      "latency_ms": 120,
      "last_checked_unix_ms": 1713500000000,
      "effective_chains": [["server-a"]],
      "url_test_members": [
        {
          "member_tag": "server-a",
          "healthy": true,
          "latency_ms": 120,
          "last_checked_unix_ms": 1713500000000,
          "last_error": null,
          "effective_chains": [["server-a"]]
        }
      ]
    }
  ]
}
```

| 字段 | 说明 |
|------|------|
| `mode.kind` | 路由模式：`rule` / `global` / `direct` |
| `mode.outbound` | global 模式的出站 tag（其他模式为 null） |
| `rule_count` | 规则数量 |
| `listeners` | 入站监听列表（tag, protocol, listen_address, listen_port） |
| `outbounds` | 出站列表（tag, protocol, server?, port?） |
| `outbound_groups` | 出站组（selector/fallback/url_test/relay/load_balance） |

### GET /api/v1/runtime

完整运行时状态：统计、日志配置、活动流、最近完成的流。

```json
{
  "stats": {
    "active_sessions": 3,
    "total_started": 100,
    "completed_sessions": 97,
    "failed_sessions": 0,
    "blocked_sessions": 0,
    "direct_sessions": 50,
    "chained_sessions": 47,
    "bytes_up": 1024000,
    "bytes_down": 5120000,
    "udp_upstream": { "active_associations": 0, "..." : "..." }
  },
  "udp_upstream_idle_timeout_seconds": 300,
  "log_level": "info",
  "log_files": ["logs/zero.log"],
  "active_sessions": [],
  "recent_completed_sessions": []
}
```

| 字段 | 说明 |
|------|------|
| `stats` | 统计摘要（同 `GET /api/v1/stats`） |
| `udp_upstream_idle_timeout_seconds` | UDP upstream 空闲超时 |
| `log_level` | 当前日志级别（`trace`/`debug`/`info`/`warn`/`error`） |
| `log_files` | 配置的日志文件路径列表，无文件输出时为空数组 |
| `active_sessions` | 活动流详情列表 |
| `recent_completed_sessions` | 最近完成的流详情列表 |

### GET /api/v1/stats

轻量统计摘要。`active_sessions`, `total_started`, `completed_sessions`, `failed_sessions`, `bytes_up`, `bytes_down` 等。

### GET /api/v1/flows

活动流列表，返回 `active_flows`（强类型 `FlowSnapshot` 数组）。支持过滤。

| 参数 | 默认 | 说明 |
|------|------|------|
| `limit` | 100 | 最大返回数 |
| `inbound_tag` | — | 按入站过滤 |
| `principal_key` | — | 按用户过滤 |

### GET /api/v1/flows/{flow_id}

单流详情。不存在返回 404。

### GET /api/v1/policies

所有 policy 状态（selector / fallback / url_test / load_balance），包含当前选择和健康探测结果。

### GET /api/v1/policies/{policy_tag}

单个 policy 详情。不存在返回 404。

### GET /api/v1/sinks

事件接收器投递状态快照。

### GET /api/v1/tun_status

TUN 虚拟网卡运行状态。

```json
{
  "running": true,
  "name": "zero-tun",
  "addr": "10.0.0.1",
  "tag": "tun-in"
}
```

| 字段 | 说明 |
|------|------|
| `running` | TUN 是否正在运行 |
| `name` | 网卡名称（运行时返回） |
| `addr` | 网卡地址（运行时返回） |
| `tag` | 入站 tag（运行时返回） |

未启动时所有字段为零值 / null：
```json
{ "running": false, "name": null, "addr": null, "tag": null }
```

---

## Command 端点

### POST /api/v1/commands

统一命令入口。

```json
{
  "id": "req-123",
  "method": "policies.select",
  "params": {
    "policy_tag": "proxy",
    "target_tag": "direct"
  }
}
```

支持的方法：

#### policies.select

切换 selector 的当前出站。

Params：`policy_tag` (string), `target_tag` (string)

Response：
```json
{ "policy_tag": "proxy", "selected": "direct" }
```

错误：
- `not_found` — policy 不存在
- `invalid_argument` — target 不在 members 中

#### policies.probe

触发 `url_test` 立即执行一轮探测（**异步**：命令仅触发，不等探测完成）。

Params：`policy_tag` (string)

Response（同步返回的只是"已触发"，**不含延迟**）：
```json
{ "policy_tag": "auto", "probe_triggered": true }
```

延迟结果通过两种途径获取（GUI 二选一）：

1. **事件推送**（推荐）—— 探测完成后发射 `policy.probe.completed` 事件，payload 含每个成员的 `latency_ms`（见 [events.md](events.md#policyprobecompleted)）。
2. **查询拉取** —— `GET /api/v1/policies/{policy_tag}` 返回 `PolicySnapshot`，其中 `latency_ms`（整组，= 选中节点延迟）与 `url_test_members[].latency_ms`（每成员）、`last_checked_unix_ms`、`last_error`。

> 单成员探测 5s 超时、N 个成员并发/串行跑完可能十几秒，故采用"触发 + 异步取结果"模式，不阻塞命令响应。

错误：`not_found` — policy 不存在或不是 `url_test` 类型

#### flows.close

主动关闭活动 flow。

Params：`flow_id` (string)

Response：
```json
{ "flow_id": "flow-123", "closed": true }
```

错误：`not_found` — flow 不存在或已结束

#### config.validate

校验配置有效性，不改变运行时状态。

Params：`config` (object, 完整配置)

Response：
```json
{ "valid": true }
```

错误：`invalid_argument` — 配置无效（cause 字段包含详情）

#### config.apply

热加载配置。成功后路由规则、出站组、模式原子替换，活动连接不受影响。

Params：`config` (object, 完整配置)

Response：
```json
{ "applied": true }
```

错误：`invalid_argument` — 配置无效

权限：`config`

#### mode.set

切换全局代理模式。

Params：`mode` (string, `"rule"` | `"direct"` | `"global"`), `outbound` (string, 仅 global 模式必填)

Response：
```json
{ "accepted": true }
```

错误：
- `invalid_argument` — mode 值无效，或 global 缺少 outbound
- `invalid_argument` — outbound tag 不存在

权限：`admin`

#### tun.start

启动 TUN 虚拟网卡。

Params：`name` (string, 可选), `addr` (string), `mask` (string, 可选, 默认 `"255.255.255.0"`), `mtu` (number, 可选, 默认 1500), `tag` (string)

Response：
```json
{ "accepted": true }
```

权限：`admin`

#### tun.stop

停止 TUN 虚拟网卡。

Params：无

Response：
```json
{ "accepted": true }
```

权限：`admin`

#### diagnostics.probe_target

对指定出站的 `server:port` 做一次 **直连 TCP 可达性探测**（**同步**返回）。

> ⚠️ 这是诊断用途：从内核所在主机**直接** `TcpStream::connect_timeout` 到出站配置里的 server:port，**不经过代理协议、不做 TLS 握手、不发协议请求**。`latency_ms` 仅反映"本机 → 服务器 TCP 端口"的 RTT，不等于经代理的端到端延迟。2 秒超时。
>
> 真正的"经代理测单节点延迟"目前由 `url_test` 组承载（把该节点放进一个 `url_test` 组再 `policies.probe`，结果经 `policy.probe.completed` / `policies` 查询拿）。

Params：`target_tag` (string)

Response：
```json
{
  "target_tag": "server-a",
  "server": "1.2.3.4",
  "port": 443,
  "reachable": true,
  "latency_ms": 12
}
```

无固定 server 的出站（relay 链、无 server 的 direct）返回 `reachable: false` 且 `error: "outbound has no probeable fixed server"`。

错误：`not_found` — target 不存在

权限：`admin`

#### diagnostics.probe_outbound

对指定出站做一次 **经代理的同步单节点延迟探测**（`url_test` 探测的单节点、同步版本）。

与另两种探测的区别：

| 命令 | 同步 | 走代理 | 测的是 |
|---|---|---|---|
| `policies.probe` | ❌ 异步（多成员） | ✅ | 组内每成员经代理首字节延迟 |
| `diagnostics.probe_target` | ✅ | ❌ 直连 TCP | 本机→server:port TCP RTT |
| **`diagnostics.probe_outbound`** | **✅** | **✅** | **单节点经代理首字节延迟** |

实现复用 `url_test` 的探测逻辑：经出站建立连接（含 TLS + 协议握手）→ 发 `HEAD {url}` → 读首字节，返回 `elapsed` 毫秒。单成员 ≤5s 超时，故可同步阻塞返回，适合 GUI"点一个节点测速"。仅支持 `http://` URL（明文，延迟不含 TLS 握手，与 `url_test` 一致）。

Params：`target_tag` (string)、`url` (string, 可选，默认 `http://www.gstatic.com/generate_204`)

Response（成功）：
```json
{
  "target_tag": "node-a",
  "url": "http://www.gstatic.com/generate_204",
  "via": "through_proxy",
  "reachable": true,
  "latency_ms": 128
}
```

Response（探测失败——超时/拒绝，属于**结果**而非命令错误，GUI 据此显示节点不可达）：
```json
{
  "target_tag": "node-a",
  "url": "http://www.gstatic.com/generate_204",
  "via": "through_proxy",
  "reachable": false,
  "latency_ms": null,
  "error": "probe target closed connection without response"
}
```

错误：`not_found` — target 不存在；`invalid_argument` — URL 非法（非 `http://` 等）

权限：`admin`

#### diagnostics.dns_lookup

解析域名。

Params：`hostname` (string)

Response：
```json
{
  "hostname": "example.com",
  "resolved_addresses": ["93.184.216.34", "2606:2800:220:1:248:1893:25c8:1946"],
  "count": 2
}
```

权限：`admin`

#### diagnostics.trace_route

查看路由规则对指定目标的匹配结果。

Params：`target` (string), `port` (number), `protocol` (string, 可选，默认 `"tcp"`)

Response：
```json
{
  "target": "1.1.1.1",
  "port": 443,
  "protocol": "tcp",
  "effective_mode": "rule",
  "route_action": { "route": "proxy" }
}
```

权限：`admin`

---

## 鉴权

内核采用 Bearer Token 鉴权和粗粒度权限门禁：

- **未配置 token**（例如 CLI `--status-listen` 的本地调试模式）：所有端点无鉴权，默认授予 `read`、`control`、`config`、`admin`
- **已配置 token**：所有请求必须携带 `Authorization: Bearer <token>` 或 `X-Zero-Api-Key: <token>`；当前配置 token 映射为 `admin`
- **权限名称**：`read`、`control`、`config`、`admin`，对外序列化使用 snake_case

这是一个内核，不是多租户 SaaS。内核只提供控制面能力边界；租户、角色、审计等业务权限应在上层面板/网关实现。

---

## 事件流端点

### GET /api/v1/events/stream

Server-Sent Events (SSE) 实时事件流。

| 参数/头 | 说明 |
|---------|------|
| `?types=flow.completed,policy.selected` | 事件类型过滤 |
| `?since=<sequence>` | 断点续传，从指定 sequence 之后开始 |
| `Last-Event-ID: <sequence>` | 同上，HTTP 头形式 |

事件格式：
```
id: 42
event: flow.completed
data: {"schema_id":"zero.event.v1","event_id":"...","event_type":"flow.completed",...}
```

连接断开后可使用 `Last-Event-ID` 续传，服务端先发送追赶事件再切回实时流。详见 [events.md](./events.md)。

### GET /api/v1/events

事件快照（一次性返回当前事件日志中的所有事件）。不如 `/events/stream` 实时，适合一次性调试。
