# GUI 接入指南

Zero 提供三通道控制面。GUI 应用推荐走 **IPC**（Unix Domain Socket / Windows Named Pipe）——零端口冲突，文件权限隔离，无需 API key。

## 架构

```
┌──────────────────────┐
│   GUI / Electron     │
│   / Tauri / Qt       │
├──────────────────────┤
│   JSON-line IPC      │  ~/.zero/control.sock
│   或 HTTP            │  localhost:9090
├──────────────────────┤
│   Zero 内核           │
└──────────────────────┘
```

## 连接

| 平台 | 路径 | 传输 |
|------|------|------|
| Linux / macOS | `~/.zero/control.sock` | Unix Domain Socket |
| Windows | `\\.\pipe\zero-control` | Named Pipe |

IPC socket 在 Zero 启动时自动创建，无需额外配置。CLI `--control-socket` 可自定义路径。

## IPC 协议

JSON-line 帧格式，一行一个 JSON 对象，`\n` 分隔。

### 查询

```
→ {"type":"query","request":"Runtime"}
← {"ok":true,"result":{...}}
```

支持的查询请求：

| request | 说明 |
|---------|------|
| `"Runtime"` | 运行时状态（统计+活动连接+策略） |
| `"Config"` | 当前配置快照 |
| `"Stats"` | 轻量统计摘要 |
| `"Policies"` | 所有策略组状态 |
| `{"Policy":{"policy_tag":"proxy"}}` | 单个策略 |
| `{"ActiveFlows":{"limit":100,"filter":{}}}` | 活动连接列表 |
| `{"Flow":{"flow_id":"42"}}` | 单连接详情 |
| `"Capabilities"` | API 能力和版本 |
| `"Health"` | 进程健康状态 |

### 命令

```
→ {"type":"command","method":"policies.select","params":{"policy_tag":"proxy","target_tag":"direct"}}
← {"ok":true,"result":{"accepted":true,"result":{"selected":"direct"}}}
```

支持的方法：`policies.select`、`policies.probe`、`flows.close`、`config.apply`。

### 事件订阅

```
→ {"type":"subscribe","events":["flow.completed","stats.sampled"]}
← {"ok":true,"result":"subscribed"}
← {"event_type":"flow.started","event_id":"...","occurred_at_unix_ms":...,"payload":{...}}
← {"event_type":"flow.updated","event_id":"...","occurred_at_unix_ms":...,"payload":{...}}
← {"event_type":"flow.completed","event_id":"...","occurred_at_unix_ms":...,"payload":{...}}
```

## Python 示例

```python
import json, socket, os

SOCK = os.path.expanduser("~/.zero/control.sock")

def ipc_request(req):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect(SOCK)
    s.sendall((json.dumps(req) + "\n").encode())
    resp = b""
    while b"\n" not in resp:
        resp += s.recv(4096)
    s.close()
    return json.loads(resp.split(b"\n")[0])

# 查询运行时状态
status = ipc_request({"type": "query", "request": "Runtime"})
print(f"活跃连接: {status['result']['stats']['active_sessions']}")
print(f"总流量: {status['result']['stats']['bytes_up']} / {status['result']['stats']['bytes_down']}")

# 切换 selector
ipc_request({
    "type": "command",
    "method": "policies.select",
    "params": {"policy_tag": "proxy", "target_tag": "direct"}
})

# 订阅事件流（实时连接列表）
import select
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.connect(SOCK)
s.sendall(json.dumps({"type": "subscribe", "events": ["flow.started", "flow.completed"]}).encode() + b"\n")
# 读取响应确认
s.recv(4096)
# 持续读事件
while True:
    ready, _, _ = select.select([s], [], [], 1.0)
    if ready:
        data = s.recv(4096)
        if not data:
            break
        for line in data.decode().strip().split("\n"):
            if line and not line.startswith(":"):
                event = json.loads(line)
                print(f"[{event['event_type']}] {event['payload'].get('flow_id', '')}")
```

## Go 示例

```go
package main

import (
    "bufio"
    "encoding/json"
    "net"
    "os"
)

func ipcRequest(req map[string]any) map[string]any {
    conn, _ := net.Dial("unix", os.Getenv("HOME")+"/.zero/control.sock")
    defer conn.Close()

    data, _ := json.Marshal(req)
    conn.Write(append(data, '\n'))

    line, _ := bufio.NewReader(conn).ReadString('\n')
    var resp map[string]any
    json.Unmarshal([]byte(line), &resp)
    return resp
}

// 查询运行时
runtime := ipcRequest(map[string]any{"type": "query", "request": "Runtime"})
// 切换 selector
ipcRequest(map[string]any{
    "type": "command",
    "method": "policies.select",
    "params": map[string]string{"policy_tag": "proxy", "target_tag": "direct"},
})
```

## Node.js / Electron 示例

```javascript
const net = require('net');
const os = require('os');

const SOCK = `${os.homedir()}/.zero/control.sock`;

function ipcRequest(req) {
  return new Promise((resolve, reject) => {
    const client = net.createConnection(SOCK, () => {
      client.write(JSON.stringify(req) + '\n');
    });
    client.on('data', (data) => {
      client.destroy();
      resolve(JSON.parse(data.toString().split('\n')[0]));
    });
    client.on('error', reject);
  });
}

// 查询
const runtime = await ipcRequest({ type: 'query', request: 'Runtime' });
console.log(`活跃连接: ${runtime.result.stats.active_sessions}`);

// 切换
await ipcRequest({
  type: 'command',
  method: 'policies.select',
  params: { policy_tag: 'proxy', target_tag: 'direct' }
});
```

## HTTP 通道（备选）

如果 GUI 不方便用 IPC（如浏览器 WebView），可用 HTTP：

```bash
# 启动时开启 HTTP
./target/release/zero run --status-listen 127.0.0.1:9090 config.json
```

```javascript
// HTTP + SSE
const resp = await fetch('http://127.0.0.1:9090/api/v1/runtime');
const runtime = await resp.json();

// 实时事件
const es = new EventSource('http://127.0.0.1:9090/api/v1/events/stream?types=flow.completed');
es.onmessage = (e) => console.log(JSON.parse(e.data));
```

所有 HTTP 端点支持 CORS，可从 `localhost:*` 直接访问。

## 事件类型参考

| 事件 | 频率 | 用途 |
|------|------|------|
| `flow.started` | 每个连接 | 新连接通知 |
| `flow.updated` | 每 10s / 连接 | 实时流量速率 |
| `flow.completed` | 连接结束 | 流量统计、结果 |
| `policy.selected` | selector 切换 | 节点切换通知 |
| `stats.sampled` | 每 30s | 系统级统计 |
| `engine.warning` | 按需 | 告警 |
| `config.changed` | 热重载 | 配置变更 |

## 典型场景

### 实时流量面板

订阅 `flow.started` + `flow.updated` + `flow.completed`，内存维护一个连接列表，每 10s 更新一次速率。

### 节点管理

查询 `/api/v1/policies` → 展示 selector 列表 → 用户点击切换 → `policies.select`。

### 实时日志

`zero events` 或订阅 SSE/ICP 事件流，展示所有事件的 timeline。

### 配置管理

读取 `/api/v1/config` → GUI 编辑 → `config.apply` 热重载（路由规则和分组均支持）。
