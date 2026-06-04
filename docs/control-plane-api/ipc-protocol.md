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
→ {"type":"query","request":{"type":"health"}}
← {"ok":true,"result":{"engine_version":"...","started_at_unix_ms":...,"healthy":true}}
```

`request` 字段是 `QueryRequest` 枚举，使用 serde tagged enum 格式（`type` 标签 + `snake_case`）：

| request | 说明 |
|---------|------|
| `{"type":"capabilities"}` | 能力查询 |
| `{"type":"health"}` | 健康检查 |
| `{"type":"config"}` | 配置快照 |
| `{"type":"runtime"}` | 运行时状态 |
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
→ {"type":"command","method":"policies.select","params":{"policy_tag":"proxy","target_tag":"direct"}}
← {"ok":true,"result":{"accepted":true}}
```

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

### Subscribe

```
→ {"type":"subscribe","events":["flow.completed"]}
← {"ok":true,"result":"subscribed"}
← {"event_type":"flow.completed","event_id":"...","occurred_at_unix_ms":...,"payload":{...}}
← :\n
← ...持续推送...
```

`events` 为可选的事件类型白名单，空或省略表示接收所有事件。

连接保持期间服务端持续推送事件帧，同时可以继续发送 Query/Command/Ping 帧获取即时响应。心跳用 `:\n`（SSE 注释格式，客户端忽略）。

## 响应格式

```json
{
  "ok": true,
  "id": null,
  "result": { },
  "error": null
}
```

- `id` — 回显请求的 `id` 字段，用于多路复用时配对。请求不带 `id` 时此字段为 `null`
- `ok` — `true` 成功 / `false` 失败
- `result` — 成功的响应数据
- `error` — 失败时的错误详情

错误响应：
```json
{
  "ok": false,
  "id": null,
  "result": null,
  "error": {
    "code": "not_found",
    "message": "Policy not found",
    "field_path": "policy_tag"
  }
}
```

## 多路复用

同一条连接上发 subscribe 后**不关闭连接**，继续发送 Query/Command/Ping 帧：

```
→ {"type":"subscribe","id":1,"events":["flow.completed"]}
← {"ok":true,"id":1,"result":"subscribed"}
← {"event_type":"flow.completed",...}
→ {"type":"query","id":2,"request":{"type":"stats"}}
← {"ok":true,"id":2,"result":{"active_sessions":3,...}}
→ {"type":"ping","id":3}
← {"ok":true,"id":3,"result":"pong"}
← {"event_type":"flow.completed",...}
```

这样只需要一条持久连接即可承载所有通信，无需为 query/command 单独创建短期连接。

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

# 查询（正确格式：request 是带 type 标签的对象）
print(ipc_request("~/.zero/control.sock", {
    "type": "query",
    "request": {"type": "health"}
}))

# 带 id 的多路复用查询
print(ipc_request("~/.zero/control.sock", {
    "type": "query",
    "id": 1,
    "request": {"type": "stats"}
}))

# 切换 selector
print(ipc_request("~/.zero/control.sock", {
    "type": "command",
    "method": "policies.select",
    "params": {"policy_tag": "proxy", "target_tag": "direct"}
}))
```

### Go

```go
conn, _ := net.Dial("unix", "/home/user/.zero/control.sock")
req, _ := json.Marshal(map[string]any{
    "type": "query", "request": map[string]any{"type": "health"},
})
conn.Write(append(req, '\n'))
buf := make([]byte, 4096)
n, _ := conn.Read(buf)
conn.Close()
fmt.Println(string(buf[:n]))
```
