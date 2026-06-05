# 性能基准与限流分层设计

本文档明确内核的性能指标、资源开销，以及限流能力的分层边界。

## 一、性能基准预期

### 1.1 单节点承载能力

**硬件基准：4核 CPU / 8GB 内存**

| 指标 | 预期值 | 说明 |
| --- | --- | --- |
| 并发连接数 | 50,000 ~ 100,000 | TCP 活跃连接 |
| 同时在线用户 | 1,000 ~ 5,000 | 有活跃流量的用户 |
| 吞吐总量 | 5 Gbps ~ 10 Gbps | 取决于网卡和 CPU |
| 新建连接速率 | 5,000 ~ 10,000 /s | TCP 三次握手 + 协议握手 |
| 内存开销 | ~2KB / 连接 | 包括 buffer 和上下文 |

**1k 用户场景完全是轻量级负载，甚至用不到 2 核。**

---

### 1.2 控制面开销

控制面本身只占总开销的 **< 5%**：

| 组件 | CPU 开销 | 内存开销 |
| --- | --- | --- |
| 状态 API 查询 | < 1% | 可忽略 |
| 事件生成 | ~1% | ~1MB / 万条事件 |
| Connector 上报 | < 1% | ~512KB |
| 权限校验 | 可忽略 | 可忽略 |

**控制面开销主要在：**
- 统计聚合（增量计算，非实时）
- 事件序列化（批量处理）
- HTTP JSON 序列化（可替换为二进制格式优化）

---

### 1.3 千万级/天流量场景

**典型机场节点日流量：10TB ~ 100TB**

内核需要支持的规模：
- `flow.completed` 事件：100 万 ~ 1000 万 / 天
- 事件产生速率：10 ~ 100 / s
- 单事件大小：~200 bytes
- 单日事件总大小：200MB ~ 2GB

**完全在当前设计的处理能力范围内。**

---

## 二、限流分层架构

### 核心原则

> **内核做基础限流，上层做业务限流。**
>
> 内核只做高效的、无状态的、必须在转发路径上做的限流；
> 所有业务相关的限流（积分、设备数、共享检测等）在上层实现。

```
┌─────────────────────────────────────────────────────────┐
│              上层业务限流 (Connector / 中心)            │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │
│  │ 积分扣减 │ │设备限制 │ │ 共享检测 │ │ 地域限制 │  │  ← 业务逻辑
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘  │
└───────┼────────────┼─────────────┼─────────────┼────────┘
        │            │             │             │
        └────────────┴─────────────┴─────────────┘
                              ▼
                    ┌─────────────────┐
                    │   Hook 扩展点   │  ← 稳定 ABI
                    └────────┬────────┘
                              ▼
┌─────────────────────────────────────────────────────────┐
│                  内核基础限流                          │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │
│  │ 总带宽  │ │ 并发数  │ │ 新建速率 │ │ 单IP限  │  │  ← 转发路径
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘  │
└─────────────────────────────────────────────────────────┘
```

---

## 三、内核层限流（必须实现）

这些限流直接作用在转发路径上，高效且无法绕过。

### 3.1 入站全局限流

```yaml
inbounds:
  - tag: vmess-public
    type: vmess
    listen: 0.0.0.0:443
    limits:
      # 总带宽限制
      bandwidth:
        up: "1Gbps"
        down: "10Gbps"
      # 总并发连接数
      max_concurrent_connections: 50000
      # 每秒新建连接数
      new_connections_per_second: 5000
      # 单 IP 并发连接数
      per_ip_max_connections: 200
```

### 3.2 单用户限流

```yaml
inbounds:
  - tag: vmess-public
    type: vmess
    users:
      - id: user-uuid-123
        principal_key: user-001
        limits:
          bandwidth:
            up: "100Mbps"
            down: "500Mbps"
          max_concurrent_connections: 500
```

### 3.3 实现方式

**Token Bucket + 无锁原子操作：**

```rust
// 每个限流实体维护一组原子计数器
pub struct RateLimiter {
    bytes_up: AtomicU64,
    bytes_down: AtomicU64,
    concurrent_conns: AtomicUsize,
    new_conn_last: AtomicU64,
    new_conn_count: AtomicUsize,
}
```

- 所有操作都是 O(1)，无锁
- 每个包检查开销 < 100ns
- 对转发性能影响 < 1%

---

## 四、扩展 Hook 层（预留能力）

为上层业务限流提供稳定的扩展点，不侵入内核核心逻辑。

### 4.1 Flow 生命周期 Hook

```rust
// crates/traits/src/flow_hook.rs

#[async_trait]
pub trait FlowHook: Send + Sync {
    /// 连接建立前调用，返回 Ok(()) 允许连接，Err 拒绝连接
    async fn on_flow_accepted(&self, flow_info: &FlowInfo) -> Result<(), FlowReject>;

    /// 每个数据包转发前调用
    async fn on_packet(&self, flow_info: &FlowInfo, direction: Direction, bytes: usize);

    /// 连接结束时调用
    async fn on_flow_completed(&self, flow_info: &FlowInfo, stats: &FlowStats);
}

pub enum FlowReject {
    Busy,
    Forbidden,
    RateLimited,
    Custom(String),
}
```

### 4.2 Hook 注册机制

```rust
// Connector 或插件可以注册多个 Hook
pub fn register_flow_hook(hook: Arc<dyn FlowHook>);

// 内核在转发路径上按顺序调用 Hook
for hook in &hooks {
    hook.on_flow_accepted(flow_info).await?;
}
```

---

## 五、上层业务限流（Connector / 中心）

所有业务相关的逻辑都在上层实现，通过 Hook 扩展点接入。

### 5.1 设备数限制

**实现方式：**

```rust
// 在 Connector 中实现
pub struct DeviceLimitHook {
    center_client: CenterClient,
    active_ips: Arc<Mutex<HashMap<String, HashSet<IpAddr>>>>,
}

#[async_trait]
impl FlowHook for DeviceLimitHook {
    async fn on_flow_accepted(&self, flow_info: &FlowInfo) -> Result<(), FlowReject> {
        let user_key = flow_info.principal_key.as_ref().unwrap();
        let client_ip = flow_info.remote_addr.ip();

        // 1. 检查当前用户已在线 IP 数量
        let mut ips = self.active_ips.lock().await;
        let user_ips = ips.entry(user_key.clone()).or_default();

        // 2. 如果是新 IP，向中心查询是否超出设备限制
        if !user_ips.contains(&client_ip) {
            let limit = self.center_client.get_device_limit(user_key).await?;
            if user_ips.len() >= limit {
                return Err(FlowReject::Forbidden);
            }
            user_ips.insert(client_ip);
        }

        Ok(())
    }
}
```

**决策点在上层，内核只执行拒绝动作。**

---

### 5.2 积分 / 余额控制

**实现方式：**

```rust
pub struct CreditLimitHook {
    center_client: CenterClient,
    user_credits: Arc<Mutex<HashMap<String, u64>>>,
}

#[async_trait]
impl FlowHook for DeviceLimitHook {
    // 连接建立时检查是否有足够余额
    async fn on_flow_accepted(&self, flow_info: &FlowInfo) -> Result<(), FlowReject> {
        let user_key = flow_info.principal_key.as_ref().unwrap();
        let credit = self.user_credits.lock().await.get(user_key).copied().unwrap_or(0);

        if credit == 0 {
            return Err(FlowReject::Forbidden);
        }

        Ok(())
    }

    // 转发时实时扣减积分
    async fn on_packet(&self, flow_info: &FlowInfo, direction: Direction, bytes: usize) {
        let user_key = flow_info.principal_key.as_ref().unwrap();
        let cost = match direction {
            Direction::Up => bytes / 1024,    // 1KB = 1 积分
            Direction::Down => bytes / 1024,
        };

        let mut credits = self.user_credits.lock().await;
        if let Some(c) = credits.get_mut(user_key) {
            *c = c.saturating_sub(cost as u64);
        }
    }
}
```

---

### 5.3 共享检测 / 异常行为检测

**检测逻辑在上层实现：**

```rust
pub struct SharingDetectionHook {
    center_client: CenterClient,
    ip_asn_cache: Arc<Mutex<HashMap<IpAddr, String>>>,
}

impl SharingDetectionHook {
    fn detect(&self, user_key: &str, client_ip: IpAddr) -> bool {
        // 检测策略：
        // 1. 同一用户短时间内来自多个 ASN
        // 2. 同一用户同时在线 IP 超出阈值
        // 3. 流量特征异常（如同时在线多个操作系统）
        // ...
    }
}
```

**这部分逻辑与业务强相关，绝不应该进入内核。**

---

## 六、性能开销量化

| 限流层级 | 延迟开销 | CPU 开销 | 适用场景 |
| --- | --- | --- | --- |
| **内核基础限流** | < 100ns | < 1% | 总带宽、总并发、新建速率 |
| **Hook 同步调用** | ~1µs | ~5% | 需要立即拒绝的场景 |
| **Hook 异步上报** | 0 (旁路) | ~1% | 统计、审计、事后分析 |
| **中心侧策略** | 网络延迟 | 0 (在中心) | 积分、设备数、共享检测 |

### 关键优化点

1. **同步 vs 异步**：必须立即拒绝的走同步 Hook，其他全部异步旁路
2. **本地缓存**：设备限制、积分余额等在本地缓存，不每次请求中心
3. **批量上报**：流量扣减等操作批量上报，不实时同步

---

## 七、实现阶段

### Phase 1: 内核基础限流
- 入站全局带宽限制
- 单 IP 并发连接数限制
- 总并发连接数限制

### Phase 2: Hook 扩展点
- FlowHook trait 定义
- Hook 注册机制
- 转发路径集成

### Phase 3: Connector 业务限流
- 设备数限制 Hook 实现
- 积分控制 Hook 实现
- 与中心控制面联动

---

## 八、边界总结

| 能力 | 实现位置 | 原因 |
| --- | --- | --- |
| 总带宽限制 | ✅ 内核 | 在转发路径上，必须高效 |
| 总并发限制 | ✅ 内核 | 在转发路径上，必须高效 |
| 单 IP 限制 | ✅ 内核 | 在转发路径上，必须高效 |
| 单用户带宽 | ✅ 内核 | 在转发路径上，必须高效 |
| 设备数限制 | ❌ Connector | 业务逻辑，需要中心联动 |
| 积分 / 余额 | ❌ Connector | 业务逻辑，需要计费系统 |
| 共享检测 | ❌ 中心控制面 | 复杂算法，数据跨节点 |
| 地域 / 时段 | ❌ 中心控制面 | 业务规则，频繁变动 |

### 核心设计哲学再强调

> **内核只做所有用户都需要的、对性能敏感的通用能力；
> 所有因业务而异的、需要灵活调整的逻辑，都在上层实现。**
>
> 这样既保证了内核的高性能和稳定性，又提供了足够的业务灵活性。
