# IPC 协议

本地进程间通信使用 JSON-line 帧协议，底层传输在 Unix 上为 Domain Socket，Windows 上为 Named Pipe。协议语义完全一致。

## 连接

| 平台 | 默认路径 | 传输 |
|------|---------|------|
| Linux/macOS | `~/.zero/control.sock` | Unix Domain Socket (0600) |
| Windows | `\\.\pipe\zero-control` | Named Pipe |
| CLI 覆盖 | `--control-socket /path/to/sock` | |

## 帧格式

每条帧是一个完整的 JSON 对象，以 `\n` 结尾。支持在同一条连接上多路复用多个请求（请求-响应用 `id` 字段配对），也可以只发一个请求后关闭连接（`id` 可省略）。

```
→ {"type":"query","id":1,"request":{"type":"health"}}\n
← {"ok":true,"id":1,"result":{...}}\n
```

## 请求类型

### Ping

```
→ {"type":"ping"}
← {"ok":true,"result":"pong"}
```

带 `id` 的多路复用形式：
```
→ {"type":"ping","id":42}
← {"ok":true,"id":42,"result":"pong"}
```

### Query

```
→ {"type":"query","id":1,"request":{"type":"health"}}
← {"ok":true,"id":1,"result":{"engine_version":"...","started_at_unix_ms":...,"healthy":true}}
```

`id` 是可选的，省略时响应不会回显（用于简单的一次性查询）。

`request` 字段是 `QueryRequest` 枚举，使用 serde tagged enum 格式（`type` 标签 + `snake_case`）：

| request | 说明 |
|---------|------|
| `{"type":"capabilities"}` | 能力查询 |
| `{"type":"health"}` | 健康检查 |
| `{"type":"config"}` | 配置快照 |
| `{"type":"runtime"}` | 运行时状态（含统计、日志配置、活动流） |
| `{"type":"stats"}` | 统计摘要 |
| `{"type":"policies"}` | 所有策略 |
| `{"type":"policy","policy_tag":"proxy"}` | 单个策略 |
| `{"type":"active_flows","limit":100,"filter":{}}` | 活动流列表 |
| `{"type":"recent_flows","limit":100,"filter":{}}` | 近期流列表 |
| `{"type":"flow","flow_id":"42"}` | 单流详情 |
| `{"type":"diagnostics"}` | 诊断信息 |
| `{"type":"sinks"}` | 事件接收器状态 |
| `{"type":"tun_status"}` | TUN 虚拟网卡状态 |

### Command

```
→ {"type":"command","id":1,"method":"policies.select","params":{"policy_tag":"proxy","target_tag":"direct"}}
← {"ok":true,"id":1,"result":{"accepted":true}}
```

`id` 可选，与 Query 一致。

支持的方法：

| method | params | 说明 |
|--------|--------|------|
| `policies.select` | `policy_tag`, `target_tag` | 切换 selector 出站 |
| `policies.probe` | `policy_tag` | 探测 urltest 组延迟 |
| `flows.close` | `flow_id` | 关闭指定流 |
| `config.validate` | `config` (完整 JSON) | 验证配置 |
| `config.apply` | `config` (完整 JSON) | 热加载配置 |
| `mode.set` | `mode`, `outbound?` | 设置全局模式 |
| `tun.start` | `name?`, `addr`, `mask?`, `mtu?`, `tag` | 启动 TUN |
| `tun.stop` | — | 停止 TUN |
| `diagnostics.probe_target` | `target_tag` | 探测出站连通性 |
| `diagnostics.dns_lookup` | `hostname` | DNS 查询 |
| `diagnostics.trace_route` | `target`, `port`, `protocol?` | 路由追踪 |

> **实现说明：** IPC Command 和 HTTP `POST /api/v1/commands` 共用同一条 serde 反序列化路径（`CommandRequest` 的 `#[serde(tag = "method", content = "params")]`）。新增 command 只需修改 `zero_api::CommandRequest`，传输层无需单独适配。

### Subscribe

```
→ {"type":"subscribe","id":1,"events":["flow.completed"]}
← {"api_version":"zero.api.v1","ok":true,"id":1,"result":"subscribed"}
← {"schema_version":"zero.event.v1","event_id":"...","event_type":"flow.completed",
   "occurred_at_unix_ms":1713500000000,"source_id":null,"sequence":4201,
   "principal_key":"user-001","labels":{},"payload":{...}}
← :\n
← ...持续推送...
```

`id` 可选，与 Query/Command 一致。

`events` 为可选的事件类型白名单，空或省略表示接收所有事件。

**事件格式与 SSE 完全相同** — 都是 `zero_api::ApiEvent<serde_json::Value>` JSON。消费者只需要一套解析代码即可同时消费 IPC 和 HTTP/SSE 两个通道的事件。事件信封详见 [events.md](./events.md)。

连接保持期间服务端持续推送事件帧，同时可以继续发送 Query/Command/Ping 帧获取即时响应。心跳用 `:\n`（SSE 注释格式，客户端忽略）。

## 响应格式

IPC 响应使用与 HTTP 相同的信封格式（`zero_api::ApiResponse`），包含 `api_version` 字段用于协议识别。

```json
{
  "api_version": "zero.api.v1",
  "ok": true,
  "id": null,
  "result": { },
  "error": null
}
```

- `api_version` — 协议版本，始终为 `"zero.api.v1"`
- `id` — 回显请求的 `id` 字段，用于多路复用时配对。请求不带 `id` 时此字段为 `null`
- `ok` — `true` 成功 / `false` 失败
- `result` — 成功的响应数据
- `error` — 失败时的错误详情

错误响应：
```json
{
  "api_version": "zero.api.v1",
  "ok": false,
  "id": null,
  "result": null,
  "error": {
    "code": "not-found",
    "message": "Policy not found",
    "field_path": "policy_tag"
  }
}
```

> **注意：** 错误码使用 kebab-case（如 `not-found`），与 HTTP 响应格式完全一致。

## 多路复用

同一条连接上发 subscribe 后**不关闭连接**，继续发送 Query/Command/Ping 帧：

```
→ {"type":"subscribe","id":1,"events":["flow.completed"]}
← {"api_version":"zero.api.v1","ok":true,"id":1,"result":"subscribed"}
← {"event_type":"flow.completed",...}
→ {"type":"query","id":2,"request":{"type":"stats"}}
← {"api_version":"zero.api.v1","ok":true,"id":2,"result":{"active_sessions":3,...}}
→ {"type":"ping","id":3}
← {"api_version":"zero.api.v1","ok":true,"id":3,"result":"pong"}
← {"event_type":"flow.completed",...}
```

这样只需要一条持久连接即可承载所有通信，无需为 query/command 单独创建短期连接。

## 内核日志

IPC server 在以下事件输出结构化日志，每条日志携带 `active=N` 表示当前活跃连接数：

| 事件 | 级别 | 示例 |
|------|------|------|
| 客户端连接 | `info` | `ipc client connected active=1` |
| 客户端正常断开 | `info` | `ipc client disconnected cleanly active=0` |
| 客户端异常断开 | `warn` | `ipc client disconnected error=BrokenPipe active=0` |
| 连接处理失败 | `warn` | `ipc connection failed error=... active=0` |
| 连接 task panic | `error` | `ipc connection task panicked` |
| 服务端就绪 | `info` | `ipc server ready pipe=\\.\pipe\zero-control` |
| 服务端停止 | `info` | `ipc server stopped` |
| Pipe connect 失败 | `warn` | `named pipe connect failed error=...` |
| Pipe create 失败 | `error` | `failed to create named pipe error=...` |

通过设置 `RUST_LOG` 控制日志级别：
```bash
RUST_LOG=zero=debug zero run config.json     # 调试级别
RUST_LOG=zero=warn zero run config.json      # 仅警告和错误
```

诊断 pipe 实例耗尽问题时，观察 `active=N` 是否持续增长（无减）。正常情况下连接数和断开数应平衡。

## 客户端示例

### CLI

```bash
zero status
zero select proxy direct
zero flows
zero policies
zero events
```

### Python

```python
import json, socket, sys

def ipc_request(sock_path, req):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect(sock_path)
    s.sendall((json.dumps(req) + "\n").encode())
    resp = b""
    while b"\n" not in resp:
        resp += s.recv(4096)
    s.close()
    return json.loads(resp.split(b"\n")[0])

# 查询（正确格式：request 是带 type 标签的对象，id 可选）
print(ipc_request("~/.zero/control.sock", {
    "type": "query",
    "id": 1,
    "request": {"type": "health"}
}))

# 切换 selector
print(ipc_request("~/.zero/control.sock", {
    "type": "command",
    "id": 2,
    "method": "policies.select",
    "params": {"policy_tag": "proxy", "target_tag": "direct"}
}))
```

### Go

```go
conn, _ := net.Dial("unix", "/home/user/.zero/control.sock")
req, _ := json.Marshal(map[string]any{
    "type": "query", "id": 1, "request": map[string]any{"type": "health"},
})
conn.Write(append(req, '\n'))
buf := make([]byte, 4096)
n, _ := conn.Read(buf)
conn.Close()
fmt.Println(string(buf[:n]))
```
