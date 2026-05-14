# IPC 协议

本地进程间通信使用 JSON-line 帧协议，底层传输在 Unix 上为 Domain Socket，Windows 上为 Named Pipe。协议语义完全一致。

## 连接

| 平台 | 默认路径 | 传输 |
|------|---------|------|
| Linux/macOS | `~/.zero/control.sock` | Unix Domain Socket (0600) |
| Windows | `\\.\pipe\zero-control` | Named Pipe |
| CLI 覆盖 | `--control-socket /path/to/sock` | |

## 帧格式

每条帧是一个完整的 JSON 对象，以 `\n` 结尾。请求和响应使用同一连接。

```
→ {"type":"query","request":"Runtime"}\n
← {"ok":true,"result":{...}}\n
```

## 请求类型

### Ping

```
→ {"type":"ping"}
← {"ok":true,"result":"pong"}
```

### Query

```
→ {"type":"query","request":"Runtime"}
← {"ok":true,"result":{...}}
```

`request` 字段与 HTTP `QueryRequest` 枚举对应：

| request 值 | 说明 |
|-----------|------|
| `"Capabilities"` | 能力查询 |
| `"Health"` | 健康检查 |
| `"Config"` | 配置快照 |
| `"Runtime"` | 运行时状态 |
| `"Stats"` | 统计摘要 |
| `"Policies"` | 所有策略 |
| `{"Policy":{"policy_tag":"proxy"}}` | 单个策略 |
| `{"ActiveFlows":{"limit":100,"filter":{}}}` | 活动流列表 |
| `{"Flow":{"flow_id":"42"}}` | 单流详情 |

### Command

```
→ {"type":"command","method":"policies.select","params":{"policy_tag":"proxy","target_tag":"direct"}}
← {"ok":true,"result":{"accepted":true,"result":{...}}}
```

支持的方法：`policies.select`、`policies.probe`、`flows.close`、`config.validate`

### Subscribe

```
→ {"type":"subscribe","events":["flow.completed"]}
← {"ok":true,"result":"subscribed"}
← {"event_type":"flow.completed","event_id":"...","occurred_at_unix_ms":...,"payload":{...}}
← {"event_type":"flow.completed","event_id":"...","occurred_at_unix_ms":...,"payload":{...}}
← :\n
← ...持续推送...
```

`events` 为可选的事件类型白名单，空或省略表示接收所有事件。

连接保持期间服务端持续推送事件帧。心跳用 `:\n`（SSE 注释格式，客户端忽略）。

## 响应格式

```json
{
  "ok": true,
  "result": { },
  "error": null
}
```

错误：
```json
{
  "ok": false,
  "result": null,
  "error": {
    "code": "not_found",
    "message": "Policy not found",
    "field_path": "policy_tag"
  }
}
```

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

# 查询
print(ipc_request("~/.zero/control.sock", {"type": "query", "request": "Runtime"}))

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
    "type": "query", "request": "Runtime",
})
conn.Write(append(req, '\n'))
buf := make([]byte, 4096)
n, _ := conn.Read(buf)
conn.Close()
fmt.Println(string(buf[:n]))
```
