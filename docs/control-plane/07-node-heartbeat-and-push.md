# 节点心跳与主动上报设计

> [!IMPORTANT]
> 本文是连接器方案设计。当前可用配置和请求格式以[推送连接器](../control-plane-api/push-connector.md)为准。

本文档解决大规模机场场景下的节点主动上报机制：节点主动连接控制中心，而非被动等待中心轮询。

## 背景问题

### 当前被动模式的局限性

```
中心控制面 → 轮询 N 个节点
    (HTTP GET /api/v1/runtime)
```

问题：
- 1000+ 节点时轮询延迟大
- 节点离线发现慢（分钟级）
- 中心需要知道所有节点地址
- NAT/内网节点无法被中心访问
- 配置下发需要额外的长连接

### 目标主动模式

```
节点 → 建立长连接 → 中心控制面
    (心跳 + 状态上报 + 配置拉取)
```

优势：
- 节点上线即发现（秒级）
- 内网/NAT 节点可主动出站
- 中心不需要维护节点列表
- 配置/指令实时下发
- 状态增量推送

---

## 整体架构

```
┌─────────────────────────────────────────────────────────────┐
│                      机场中心控制面                          │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐            │
│  │ 节点管理   │  │配置下发   │  │ 数据聚合   │            │
│  └──────┬─────┘  └──────┬─────┘  └──────┬─────┘            │
└─────────┼────────────────┼────────────────┼──────────────────┘
          │                │                │
          └────────────────┼────────────────┘
                           ▼
                    ┌────────────┐
                    │  Connector │  ←─── 统一接入层
                    └────────────┘
                           ▲
         ┌─────────────────┼─────────────────┐
         │                 │                 │
         ▼                 ▼                 ▼
  ┌────────────┐    ┌────────────┐    ┌────────────┐
  │  Node 1    │    │  Node 2    │    │  Node N    │
  │  Zero内核  │    │  Zero内核  │    │  Zero内核  │
  │  +Connector│    │  +Connector│    │  +Connector│
  └────────────┘    └────────────┘    └────────────┘
```

---

## 核心设计原则

### Connector 作为可选组件

```
Zero 内核本身 ────────────── 不依赖中心
    ↑
    │ 可选编译
    ▼
Connector 插件 ────────────── 主动连接中心
    ↑
    │ 通过标准内核 API
    ▼
零侵入 ───────────────────── 不修改内核代码
```

**关键：Connector 是独立 crate，不是内核的必须部分**

内核只提供标准 API，Connector 通过本地 in-process API 采集数据并上报。

---

## Connector 协议设计

### 1. 注册与认证

节点启动时向中心注册：

```json
POST /v1/nodes/register
{
  "node_id": "node-uuid-123",
  "build_id": "<build-id>",
  "features": ["vmess", "vless", "url_test"],
  "listen_addrs": ["0.0.0.0:443"],
  "tags": ["region:us-west", "bandwidth:10G"],
  "secret": "node-shared-secret"
}
```

响应：

```json
{
  "node_id": "node-uuid-123",
  "server_time": 1713500000000,
  "heartbeat_interval_ms": 10000,
  "report_interval_ms": 60000,
  "config_hash": "abc123def",
  "commands_endpoint": "/v1/nodes/node-uuid-123/commands",
  "events_endpoint": "/v1/nodes/node-uuid-123/events"
}
```

---

### 2. 心跳保活

轻量级心跳，仅保活和状态同步：

```json
POST /v1/nodes/node-uuid-123/heartbeat
{
  "node_id": "node-uuid-123",
  "timestamp": 1713500005000,
  "uptime_ms": 123456,
  "load": {
    "cpu": 0.25,
    "mem_mb": 256,
    "active_flows": 1024,
    "bps_up": 125000000,
    "bps_down": 500000000
  }
}
```

心跳间隔由中心响应或本地 connector 配置决定。该间隔只用于远程节点存活判断，不影响本地控制面查询的新鲜度。
超时判定：3 次心跳无响应 = 节点离线

---

### 3. 状态增量上报

周期性上报聚合统计（非原始 flow 数据）：

```json
POST /v1/nodes/node-uuid-123/stats
{
  "node_id": "node-uuid-123",
  "interval_start_ms": 1713500000000,
  "interval_end_ms": 1713500030000,
  "total_flows": 5000,
  "bytes_up": 107374182400,    // 100GB
  "bytes_down": 536870912000,   // 500GB
  "flow_error_count": 12,
  "per_principal": [
    {
      "principal_key": "user-001",
      "bytes_up": 1048576,
      "bytes_down": 8388608,
      "flow_count": 15
    }
  ],
  "per_outbound": [
    {
      "tag": "direct",
      "bytes_up": 1073741824,
      "bytes_down": 5368709120,
      "rtt_ms": 25
    }
  ]
}
```

上报间隔由中心响应或本地 connector 配置决定。远程上报发送聚合窗口；本地 HTTP `GET /api/v1/stats` 和 IPC `{"stats":{}}` 始终返回调用时的当前内存快照。

---

### 4. 事件批量上报

重要事件实时或批量上报：

```json
POST /v1/nodes/node-uuid-123/events
{
  "node_id": "node-uuid-123",
  "events": [
    {
      "event_id": "evt-123",
      "event_type": "flow.completed",
      "timestamp_ms": 1713500000000,
      "payload": { ... }
    },
    {
      "event_id": "evt-456",
      "event_type": "policy.selected",
      "timestamp_ms": 1713500001000,
      "payload": { ... }
    }
  ]
}
```

上报策略：
- 实时事件立即上报（最多 1s 缓冲）
- 普通事件攒批上报（batch size = 100）
- 失败自动重试 + 指数退避

---

### 5. 指令下发通道

中心通过长连接向节点下发指令：

#### 方案 A: WebSocket 双向通道

```
节点建立 WebSocket: wss://center.example.com/v1/nodes/connect
```

中心可推送的指令类型：

```json
// 配置更新
{
  "command_id": "cmd-123",
  "type": "config.update",
  "params": {
    "config_hash": "new-config-hash",
    "full_config": { ... }
  }
}

// 策略切换
{
  "command_id": "cmd-456",
  "type": "policy.select",
  "params": {
    "policy_tag": "proxy",
    "target_tag": "cn2-gia"
  }
}

// 节点重启
{
  "command_id": "cmd-789",
  "type": "node.restart",
  "params": {
    "grace_period_ms": 30000
  }
}
```

#### 方案 B: 轮询指令队列

对于无法建立长连接的环境，节点轮询指令：

```
GET /v1/nodes/node-uuid-123/commands?since=12345
```

---

## Connector 内部结构

```
crates/zero-connector/
├── src/
│   ├── config.rs          # Connector 配置
│   ├── client.rs          # HTTP 客户端
│   ├── register.rs        # 节点注册
│   ├── heartbeat.rs       # 心跳循环
│   ├── stats_reporter.rs  # 统计上报
│   ├── event_sink.rs      # 事件上报
│   ├── command_stream.rs  # 指令接收
│   └── lib.rs
└── Cargo.toml
```

### 与内核的集成方式

**通过本地 API 零侵入对接：**

```rust
// Connector 不直接依赖内核内部结构
// 只依赖 zero-api 的公共 trait

pub struct NodeConnector {
    api_client: Arc<dyn QueryService>,  // 本地 in-process API
    command_service: Arc<dyn CommandService>,
    event_source: Arc<dyn EventSource>,
    center_client: CenterClient,        // 上报到中心
}

impl NodeConnector {
    pub async fn run(&self) {
        // 1. 注册节点
        self.register().await;

        // 2. 启动心跳循环
        tokio::spawn(self.heartbeat_loop());

        // 3. 启动统计上报循环
        tokio::spawn(self.stats_report_loop());

        // 4. 订阅内核事件并上报
        tokio::spawn(self.event_forward_loop());

        // 5. 接收中心指令
        tokio::spawn(self.command_receive_loop());
    }
}
```

---

## 配置模型

### 节点侧配置

```yaml
# zero.yml
connector:
  enabled: true
  center_endpoint: "https://center.example.com"
  node_id: "node-uuid-123"
  secret: "node-shared-secret"

  heartbeat:
    interval_ms: 5000
    timeout_ms: 3000

  stats_report:
    interval_ms: 60000
    include_per_principal: true   # 是否上报按用户统计
    include_per_outbound: true    # 是否上报按出站统计

  event_report:
    enabled: true
    batch_size: 100
    max_buffer_size: 10000
    filter:
      - "flow.completed"
      - "policy.selected"
      - "engine.warning"

  command_stream:
    enabled: true
    use_websocket: true
    poll_interval_ms: 10000
```

### 中心侧配置（参考）

```yaml
nodes:
  auth_secret: "global-node-secret"
  heartbeat_timeout_ms: 15000
  default_heartbeat_interval_ms: 5000
  default_stats_interval_ms: 60000
```

---

## 容灾与可靠性

### 1. 网络中断恢复

- 中心不可用时，节点继续正常转发流量
- 统计数据在本地缓冲（最多 1 小时）
- 网络恢复后自动补传
- 事件按序列号去重

### 2. 降级策略

| 故障场景 | 降级策略 |
| --- | --- |
| 中心不可达 | 本地缓冲，继续转发 |
| 心跳超时 | 不影响流量，重试注册 |
| 上报失败 | 指数退避，磁盘缓冲 |
| 配置拉取失败 | 使用本地缓存配置 |

### 3. 安全

- 所有通信使用 TLS
- 节点使用独立密钥认证
- 中心下发指令有签名验证
- 不传输敏感配置（如用户 UUID）

---

## 实现阶段

### Phase 1: 基础上报 (MVP)
- 节点注册
- 心跳保活
- 聚合统计上报

### Phase 2: 事件上报
- `flow.completed` 批量上报
- 失败重试
- 本地磁盘缓冲

### Phase 3: 指令下发
- WebSocket 长连接
- 配置热更新指令
- 策略切换指令

### Phase 4: 高级特性
- 按用户统计聚合
- 增量配置更新
- 固件自动升级
- 远程诊断

---

## 边界重申

| Zero 内核提供 | Zero Connector 提供 | 机场中心提供 |
| --- | --- | --- |
| 标准 Query API | 注册/心跳 | 节点管理 |
| 标准 Command API | 统计聚合上报 | 用户/套餐/计费 |
| 事件订阅接口 | 事件批量上报 | Web 面板 |
| 配置热重载 | 指令接收执行 | 多节点调度 |

**Connector 是可选的、独立的组件，不影响内核的独立性。**
