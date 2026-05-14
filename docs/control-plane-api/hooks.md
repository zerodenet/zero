# FlowHook 扩展点

Hook 在 flow 生命周期中提供同步干预点，用于外部计费、认证、设备限制等场景。

## 架构

```
Flow 生命周期
  │
  ├─ prepare_session()
  │     │
  │     ├─ FlowHookChain::on_flow_start(&ctx)
  │     │     ├─ Hook 1 (e.g. IpcFlowHook)
  │     │     ├─ Hook 2
  │     │     └─ ...
  │     │     │
  │     │     ├─ Ok(()) → 放行，继续建立连接
  │     │     └─ Err(BlockReason) → 阻断，连接被拒绝
  │     │
  │     └─ (session committed, flow.started event emitted)
  │
  ├─ ... proxy relays data ...
  │
  └─ finish_session()
        │
        └─ FlowHookChain::on_flow_end(&ctx, outcome, &stats)
              ├─ Hook 1 (fire-and-forget)
              └─ Hook 2
```

## FlowHook Trait

```rust
pub trait FlowHook: Send + Sync {
    fn on_flow_start(&self, ctx: &FlowContext) -> Result<(), BlockReason>;
    fn on_flow_end(&self, ctx: &FlowContext, outcome: SessionOutcome, stats: &FlowTraffic);
}
```

- **`on_flow_start`** — 同步调用，必须快速返回。返回 `Err(BlockReason)` 阻断该 flow
- **`on_flow_end`** — 通知，返回值被忽略
- 所有调用被 `catch_unwind` 包裹，单个 hook panic 不影响其他
- Engine 无 hook 时为零开销（`Option` 为 `None`）

## FlowContext

传递给 hook 的只读快照：

```rust
pub struct FlowContext {
    pub flow_id: u64,
    pub inbound_tag: Option<String>,
    pub outbound_tag: Option<String>,
    pub target_host: String,
    pub target_port: u16,
    pub network: String,        // "tcp" | "udp"
    pub protocol: String,       // "socks5" | "vless" | ...
    pub auth: Option<AuthSnapshot>,
    pub mode: String,            // "rule" | "global" | "direct"
    pub started_at_unix_ms: u64,
    pub labels: Vec<(String, String)>,
}
```

## IpcFlowHook

将决策委托给外部进程，通过 UDS/Named Pipe 通信。

### 配置

```json
{
  "api": {
    "hooks": [
      { "type": "ipc", "socket": "/run/billing/hook.sock", "timeout_ms": 100 }
    ]
  }
}
```

或 CLI：`--ipc-hook-socket /run/billing/hook.sock`

### 协议

**flow_start** — 同步请求/响应：

```
→ {"type":"check_flow","flow_id":42,"inbound_tag":"socks5","target_host":"example.com","target_port":443,"network":"tcp","protocol":"socks5","auth_scheme":null,"principal_key":null,"mode":"rule"}
← {"allow":true}
← {"allow":false,"code":"quota_exhausted","message":"Daily quota exceeded"}
```

**flow_end** — fire-and-forget（不读响应）：

```
→ {"type":"flow_end","flow_id":42,"inbound_tag":"socks5","target_host":"example.com","target_port":443,"network":"tcp","principal_key":null,"outcome":"direct-relayed","bytes_up":1024000,"bytes_down":5120000,"duration_ms":30000}
```

### 安全策略

- **超时**：默认 100ms，可在配置中调整
- **Fail-open**：外部进程不可达时自动放行，发射 `engine.warning` 事件
- **连接复用**：保持长连接，加速后续请求
- **Panic 隔离**：外部进程异常不影响代理转发

### 示例：Python 计费 Hook

```python
import json, socketserver

class HookHandler(socketserver.BaseRequestHandler):
    def handle(self):
        for line in self.rfile:
            req = json.loads(line)
            if req["type"] == "check_flow":
                if check_quota(req.get("principal_key")):
                    self.wfile.write(b'{"allow":true}\n')
                else:
                    self.wfile.write(b'{"allow":false,"code":"quota","message":"no quota"}\n')
            # flow_end 只收不发

def check_quota(user):
    # 调用数据库/Redis 检查配额
    return True

socketserver.UnixStreamServer("/run/billing/hook.sock", HookHandler).serve_forever()
```

## FlowHookChain

多个 hook 按配置顺序组成链式调用：

- `on_flow_start` — 依次调用，第一个返回 `Err` 的 hook 阻断该 flow，后续不再调用
- `on_flow_end` — 全部调用，单个失败不影响其他
- Panic hook 被 `catch_unwind` 捕获，记录日志后继续
