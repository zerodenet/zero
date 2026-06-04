# GUI 接入指南

Zero 提供三通道控制面。GUI 应用推荐走 **IPC**（Unix Domain Socket / Windows Named Pipe）——零端口冲突，文件权限隔离，无需 API key。

## 架构

```
┌──────────────────────┐
│   GUI / Electron     │
│   / Tauri / Qt       │
├──────────────────────┤
│   JSON-line IPC      │  ~/.zero/control.sock (Unix)
│   或 HTTP            │  \\.\pipe\zero-control (Windows)
├──────────────────────┤    localhost:9090 (HTTP)
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

完整协议规范见 [ipc-protocol.md](../control-plane-api/ipc-protocol.md)。下面是 GUI 开发者需要的核心要点。

### Query 请求

`request` 字段使用 **externally-tagged** 格式——一个 snake_case 变体名作为 JSON key，值为查询参数对象：

```
→ {"type":"query","id":1,"request":{"health":{}}}
← {"api_version":"zero.api.v1","ok":true,"id":1,"result":{"health":{"engine_version":"0.0.9",...}}}
```

> **注意**：`request` 是一个 JSON 对象，**不是**字符串。`"request":"runtime"` 是错误的。`"request":{"type":"runtime"}` 也是错误的。

常用查询：

| request | 说明 |
|---------|------|
| `{"health":{}}` | 健康检查 |
| `{"config":{}}` | 配置快照 |
| `{"runtime":{}}` | 完整运行时状态 |
| `{"stats":{}}` | 统计摘要 |
| `{"policies":{}}` | 所有策略组 |
| `{"policy":{"policy_tag":"proxy"}}` | 单个策略 |
| `{"active_flows":{"limit":100,"filter":{}}}` | 活动流列表 |
| `{"sinks":{}}` | 事件接收器投递状态 |
| `{"tun_status":{}}` | TUN 状态 |

### Query 响应

`result` 字段包含 **externally-tagged** 的 `QueryResponse`——一个变体名 key 包裹内部数据。访问时需要多解一层：

```python
resp = ipc_request({"type": "query", "request": {"runtime": {}}})
# resp["result"]["runtime"] 里面才是实际数据
stats = resp["result"]["runtime"]["stats"]
print(f"活跃连接: {stats['active_sessions']}")
```

> **HTTP 通道不同**：HTTP 的 `result` 直接就是内部数据，没有外层变体名。例如 HTTP `GET /api/v1/runtime` 返回 `result.stats.active_sessions`，而 IPC 返回 `result.runtime.stats.active_sessions`。

### Command

```
→ {"type":"command","method":"policies.select","params":{"policy_tag":"proxy","target_tag":"direct"}}
← {"api_version":"zero.api.v1","ok":true,"result":{"accepted":true,"result":{"selected":"direct"}}}
```

支持的方法：`policies.select`、`policies.probe`、`flows.close`、`config.apply`、`config.validate`、`mode.set`、`tun.start`、`tun.stop`、`diagnostics.probe_target`、`diagnostics.dns_lookup`、`diagnostics.trace_route`。

### 事件订阅

```
→ {"type":"subscribe","events":["flow.completed"]}
← {"api_version":"zero.api.v1","ok":true,"result":"subscribed"}   ← 确认帧
← {"schema_version":"zero.event.v1","event_type":"flow.completed",...}   ← 事件帧
```

**重要**：第一条响应是确认帧（包含 `api_version`、`ok`、`result` 信封），后续是裸事件帧（`ApiEvent` JSON，无信封包裹）。用 `ok` 字段区分：有 `ok` → 确认帧；没有 `ok` → 事件帧。

`events` 为可选的事件类型白名单，空或省略表示接收所有事件。

## 完整接入示例

### Python（Unix）

```python
import json, socket, os

SOCK = os.path.expanduser("~/.zero/control.sock")

def ipc_request(req):
    """发送单次请求并读取响应。"""
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect(SOCK)
    s.sendall((json.dumps(req) + "\n").encode())
    resp = b""
    while b"\n" not in resp:
        chunk = s.recv(4096)
        if not chunk:
            break
        resp += chunk
    s.close()
    return json.loads(resp.split(b"\n")[0])

# 查询运行时状态
runtime = ipc_request({"type": "query", "id": 1, "request": {"runtime": {}}})
stats = runtime["result"]["runtime"]["stats"]
print(f"活跃连接: {stats['active_sessions']}")
print(f"总上行: {stats['bytes_up']}")

# 查询策略列表
policies = ipc_request({"type": "query", "id": 2, "request": {"policies": {}}})
for p in policies["result"]["policies"]:
    print(f"  {p['tag']} ({p['kind']}) → {p.get('selected', '-')}")

# 切换 selector
ipc_request({
    "type": "command",
    "id": 3,
    "method": "policies.select",
    "params": {"policy_tag": "proxy", "target_tag": "direct"}
})
```

### Python（Windows）

```python
import json

PIPE = r"\\.\pipe\zero-control"

def ipc_request(req):
    """Windows Named Pipe 用普通文件操作即可。"""
    with open(PIPE, "r+b") as f:
        f.write((json.dumps(req) + "\n").encode())
        f.flush()
        resp = b""
        while b"\n" not in resp:
            chunk = f.read(4096)
            if not chunk:
                break
            resp += chunk
        return json.loads(resp.split(b"\n")[0])

# 用法与 Unix 版本完全相同
runtime = ipc_request({"type": "query", "id": 1, "request": {"runtime": {}}})
stats = runtime["result"]["runtime"]["stats"]
print(f"活跃连接: {stats['active_sessions']}")
```

### 事件流订阅（Unix）

```python
import json, socket, os, select

SOCK = os.path.expanduser("~/.zero/control.sock")

s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.connect(SOCK)
s.sendall(json.dumps({
    "type": "subscribe",
    "id": 1,
    "events": ["flow.started", "flow.completed", "stats.sampled"]
}).encode() + b"\n")

while True:
    ready, _, _ = select.select([s], [], [], 1.0)
    if not ready:
        continue
    data = s.recv(4096)
    if not data:
        break
    for line in data.decode().strip().split("\n"):
        if not line or line.startswith(":"):   # 心跳行，跳过
            continue
        event = json.loads(line)
        if "ok" in event:
            # 确认帧
            print(f"[subscribe] ok={event['ok']}")
            continue
        # 事件帧
        print(f"[{event['event_type']}] {event.get('payload', {}).get('flow_id', '')}")
```

### Node.js / Electron（跨平台）

```javascript
const net = require('net');
const os = require('os');
const path = require('path');

// Unix 和 Windows 自动切换
const SOCK = process.platform === 'win32'
  ? '\\\\.\\pipe\\zero-control'
  : path.join(os.homedir(), '.zero', 'control.sock');

function ipcRequest(req) {
  return new Promise((resolve, reject) => {
    const client = net.createConnection(SOCK, () => {
      client.write(JSON.stringify(req) + '\n');
    });
    client.on('data', (data) => {
      client.destroy();
      resolve(JSON.parse(data.toString().split('\n')[0]));
    });
    client.on('error', (err) => {
      if (err.code === 'ENOENT' || err.code === 'ECONNREFUSED') {
        reject(new Error('Zero is not running'));
      } else {
        reject(err);
      }
    });
  });
}

// 查询运行时
async function main() {
  try {
    const resp = await ipcRequest({ type: 'query', id: 1, request: { runtime: {} } });
    const runtime = resp.result.runtime;
    console.log(`活跃连接: ${runtime.stats.active_sessions}`);

    // 查询策略
    const policies = await ipcRequest({ type: 'query', id: 2, request: { policies: {} } });
    for (const p of policies.result.policies) {
      console.log(`  ${p.tag} (${p.kind}) → ${p.selected ?? '-'}`);
    }

    // 切换
    await ipcRequest({
      type: 'command',
      id: 3,
      method: 'policies.select',
      params: { policy_tag: 'proxy', target_tag: 'direct' }
    });
  } catch (err) {
    console.error(err.message);
  }
}

main();
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
const data = await resp.json();
console.log(`活跃连接: ${data.result.stats.active_sessions}`);
// 注意：HTTP result 直接是内部数据，没有变体名包裹

// 实时事件
const es = new EventSource('http://127.0.0.1:9090/api/v1/events/stream?types=flow.completed');
es.onmessage = (e) => console.log(JSON.parse(e.data));
```

所有 HTTP 端点支持 CORS，可从 `localhost:*` 直接访问。

> **HTTP 和 IPC 的响应格式差异**：HTTP `result` 直接是内部数据（如 `{stats:{...}}`），IPC `result` 包含变体名 key（如 `{"runtime":{stats:{...}}}`）。两个通道的事件格式完全一致。

## 事件类型参考

| 事件 | 频率 | 用途 |
|------|------|------|
| `flow.started` | 每个连接 | 新连接通知 |
| `flow.updated` | 每 10s / 连接 | 实时流量速率 |
| `flow.completed` | 连接结束 | 流量统计、结果 |
| `policy.selected` | selector 切换 | 节点切换通知 |
| `policy.probe.completed` | urltest 探测完成 | 延迟结果 |
| `stats.sampled` | 每 30s | 系统级统计 |
| `engine.warning` | 按需 | 告警 |
| `config.changed` | 热重载 | 配置变更 |
| `ipc.connected` | IPC 连接 | GUI 连接/断开监控 |
| `ipc.disconnected` | IPC 断开 | GUI 重连检测 |

## 典型场景

### 实时流量面板

订阅 `flow.started` + `flow.updated` + `flow.completed`，内存维护一个连接列表，每 10s 更新一次速率。

### 节点管理

查询 `{"policies":{}}` → 展示 selector 列表 → 用户点击切换 → `policies.select`。

### 实时日志

订阅所有事件或 IPC 流，展示 timeline。

### 配置管理

查询 `{"config":{}}` → GUI 编辑 → `config.apply` 热重载（路由规则和分组均支持）。

## 错误处理

| 情况 | 检测方式 | 建议 |
|------|----------|------|
| Zero 未启动 | 连接 socket/pipe 失败（ENOENT / ECONNREFUSED） | 定时重连（1~5 秒间隔） |
| Zero 重启 | 连接断开，read 返回 0 | 关闭旧连接，延迟后重连 |
| 命令失败 | `ok:false`，检查 `error.code` | 按 code 分支处理，显示给用户 |
| 权限不足 | `error.code === "permission-denied"` | 提示权限问题 |
| 功能未编译 | `error.code === "feature-disabled"` | 提示版本不支持 |

## 兼容性

控制面遵循 **只增不改** 的演化原则。详见 [compatibility.md](../control-plane-api/compatibility.md)。

核心规则：
- 新增字段 → 旧消费者忽略（`#[serde(default)]`）
- 新增事件类型 → 未知类型直接跳过
- 新增命令 → 旧服务器返回 `unsupported` 错误
- `api_version` 字段标识协议版本，初始化时检查
