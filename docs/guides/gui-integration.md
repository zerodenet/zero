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

## 短期接入清单

GUI 首屏启动建议按固定顺序拉取能力，避免把内核状态推断写散在各个页面里：

1. 连接 IPC；连接失败时按“内核未启动”处理
2. 查询 `{"health":{}}`，确认进程存活和 `engine_build_id`
3. 查询 `{"capabilities":{}}`，确认当前构建启用的 feature、adapter、sink 和权限
4. 查询 `{"config":{}}`，构建监听、出站、policy、规则数量等静态视图
5. 查询 `{"runtime":{}}`，构建统计、活动 flow、最近完成 flow、日志级别等运行时视图
6. 建立一条长期 `subscribe` 连接，接收 flow、policy、stats、config 和 warning 事件

GUI 不需要把所有事件类型固化为枚举。对已识别事件更新对应页面；对未知事件保留原始 JSON 或忽略。事件缺失或连接断开后，用 `runtime` / `stats` 查询重建界面状态。

推荐最小查询组合：

| 页面 | Query | Event |
|------|-------|-------|
| 总览 | `runtime`, `stats`, `health` | `stats.sampled`, `engine.warning`, `config.changed` |
| 连接 | `active_flows`, `recent_flows`, `flow` | `flow.started`, `flow.updated`, `flow.completed` |
| 节点 / 策略 | `policies`, `policy` | `policy.selected`, `policy.probe.completed` |
| 配置 | `config` | `config.changed` |
| 事件投递 | `sinks` | `engine.warning` |
| TUN | `tun_status` | `engine.warning` |

## GUI 状态模型

GUI 应把内核返回的数据当作权威状态，而不是维护一份可写副本：

- `config` 是当前有效配置视图，适合配置页面和差异展示
- `runtime` 是运行中快照，适合首屏和重连恢复
- `stats` 是调用时即时统计快照，适合手动刷新或事件断流兜底
- `events` 是增量事实，适合驱动界面局部更新
- `capabilities` 是功能发现，不应硬编码“默认一定支持某协议”

编辑配置时建议使用“草稿配置”模型：GUI 在本地维护草稿，提交前调用 `config.validate`，成功后再调用 `config.apply`。`config.apply` 成功后再重新查询 `config` 和 `runtime`，以确认内核实际接受的状态。

## 短期补齐建议

以下能力对 GUI 对接收益最高，但应作为内核通用能力实现，不应引入面板、订阅、用户套餐等上层业务概念：

| 能力 | 建议形态 | 对 GUI 的价值 |
|------|----------|---------------|
| 机器可读契约 | 导出 JSON Schema / OpenAPI | 自动生成表单、类型、校验和 SDK |
| 配置影响预检 | `config.plan_apply` command | 保存前展示哪些变更会热加载、哪些需要重启 listener |
| 结构化校验诊断 | `error.field_path` + `details[]` | 在表单具体字段旁展示错误 |
| 路由解释 | `diagnostics.trace_route` 扩展返回命中规则链 | 做“为什么走这个节点”的调试面板 |
| DNS 调试 | DNS 查询、路由、缓存、Fake IP 查询命令 | 排查透明代理和分流问题 |
| 日志流 | 受过滤的日志订阅或查询 | GUI 内置实时日志页，减少用户查文件 |
| flow 关闭原因 | completed flow 增加标准 close reason | 连接列表可区分正常结束、主动关闭、超时、上游失败 |
| policy 探测详情 | probe 事件保留成员错误、耗时和选择原因 | 节点页可展示延迟和失败原因 |

这些补齐项应继续遵守控制面契约：所有字段、枚举值、错误码使用 `snake_case`；能力通过 `capabilities` 暴露；未启用时返回 `feature_disabled` 或不出现在能力列表中。

## IPC 协议

JSON-line 帧格式，一行一个 JSON 对象，`\n` 分隔。

完整协议规范见 [ipc-protocol.md](../control-plane-api/ipc-protocol.md)。下面是 GUI 开发者需要的核心要点。

### Query 请求

`request` 字段使用 **externally-tagged** 格式——一个 snake_case 变体名作为 JSON key，值为查询参数对象：

```
→ {"type":"query","id":1,"request":{"health":{}}}
← {"api_id":"zero.api.v1","ok":true,"id":1,"result":{"health":{"engine_build_id":"0.0.9",...}}}
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
← {"api_id":"zero.api.v1","ok":true,"result":{"accepted":true,"result":{"selected":"direct"}}}
```

支持的方法：`policies.select`、`policies.probe`、`flows.close`、`config.apply`、`config.validate`、`mode.set`、`tun.start`、`tun.stop`、`diagnostics.probe_target`、`diagnostics.dns_lookup`、`diagnostics.trace_route`。

### 事件订阅

```
→ {"type":"subscribe","events":["flow.completed"]}
← {"api_id":"zero.api.v1","ok":true,"result":"subscribed"}   ← 确认帧
← {"schema_id":"zero.event.v1","event_type":"flow.completed",...}   ← 事件帧
```

**重要**：第一条响应是确认帧（`ApiResponse`，包含 `api_id`、`ok`、`result`），后续是裸事件帧（`ApiEvent`，包含 `schema_id`、`event_id`、`event_type`）。用顶层 `ok` 字段区分：有 `ok` → 响应帧；没有 `ok` → 事件帧。`id` 只是请求关联 ID，只有请求带 `id` 时确认帧才会回显，不能用来判断帧类型。

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

# 用法与 Unix 示例完全相同
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
| `policy.probe.completed` | url_test 探测完成 | 延迟结果 |
| `stats.sampled` | 每 1s | 系统级统计 |
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
| 权限不足 | `error.code === "permission_denied"` | 提示权限问题 |
| 功能未启用 | `error.code === "feature_disabled"` | 提示当前构建或运行时未启用该能力 |

## 解析建议

控制面契约详见 [contract.md](../control-plane-api/contract.md)。

GUI 解析时先判断响应信封的 `ok` 字段，再读取 `result` 或 `error`。事件流按
`event_type` 字符串分发；GUI 不需要把所有事件类型固化为枚举。
