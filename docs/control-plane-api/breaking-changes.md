# 控制面兼容性与破坏性变更

本文记录会影响 GUI、SDK、面板、事件 Sink 或进程内 Rust 集成的控制面语义变化。当前事实仍以同目录下的接口与事件文档为准；本文只维护版本边界和迁移要求。

## 消费者如何判断兼容性

外部消费者连接内核后应依次检查：

1. `health.engine_build_id`，确定实际运行的内核版本；
2. `capabilities.api_id` 和 `capabilities.schema_id`，确定请求与事件信封版本；
3. `capabilities.features`、`build_features` 和协议矩阵，确定当前构建实际启用的能力；
4. 本文对应版本的语义变更，再决定是否启用兼容分支。

兼容性标识的含义：

| 标识 | 当前值 | 何时必须变化 |
|------|--------|--------------|
| `api_id` | `zero.api.v1` | 请求、响应信封或既有字段出现不兼容 wire 变化 |
| `schema_id` | `zero.event.v1` | 事件信封或既有事件字段出现不兼容 wire 变化 |
| `engine_build_id` | Cargo 包版本 | wire 兼容但行为、时序或恢复语义发生变化 |

新增可选字段、未知事件类型和新增 capability 通常保持向前兼容；消费者必须忽略不认识的可选字段和事件。改变 ACK 时序、快照含义、增量合并规则、重放范围或既有字段含义，即使 JSON 形状不变，也必须在本文登记。

## 版本矩阵

| 版本 | 影响面 | 迁移结论 |
|------|--------|----------|
| `Unreleased` | — | 暂无待发布的兼容性变更 <!-- version-contract:unreleased-row --> |
| `0.0.15-rc.1` | 进程内 Rust `EventSource`、事件 Sink | Rust 实现者必须迁移到实时 `EventStream`；IPC/HTTP/gRPC GUI wire 无变化 |
| `0.0.15-rc` | GUI flow 生命周期 | 订阅 ACK 后以 `flow.snapshot` 建立活动连接基线，再合并 flow 增量 |

## Unreleased

<!-- 在这里登记已实现但尚未封板的兼容性变更。 -->

## 0.0.15-rc.1

### `EventSource` 统一为实时订阅

旧语义存在两个不同实现：

- `Engine::subscribe()` 返回一次性的 `Vec<RawApiEvent>` 历史快照；
- `EngineHandle::subscribe()` 返回实时 `EventSubscriber`。

新语义：

- 所有 `EventSource::subscribe()` 都返回实现 `EventStream` 的实时订阅；
- `latest(limit, filter)` 只用于读取近期历史；
- `since(sequence, limit, filter)` 用于按事件序号恢复，返回 `requested_after`、`actual_from` 和 `has_gap`；
- `has_gap = true` 时，消费者不得直接继续套用增量，必须先通过快照或 Query 重建状态；
- 包含 flow 生命周期的实时订阅仍可在增量前发送合成的 `flow.snapshot`。

进程内 Rust 实现者需要：

1. 将 `type Stream = Vec<RawApiEvent>` 替换为实现 `EventStream` 的实时流；
2. 实现阻塞 `recv()` 和非阻塞 `try_recv()`；
3. 实现新的 `EventSource::since()` 游标恢复方法；
4. 不再把 `subscribe()` 当作历史查询使用。

### EventDispatcher 投递时序

EventDispatcher 从周期性事件环扫描改为持有一个实时订阅：

- dispatcher 不再反复把历史快照当作新事件扫描；实时订阅仍按配置的轮询间隔排空并投递到 Sink；
- `flow.snapshot` 仍只用于实时客户端同步，不投递到 JSONL/Webhook；
- Webhook、重试、死信和 Sink 过滤语义保持不变；
- 外部 Sink 应继续使用 `event_id` 去重，并按 `source_id + sequence` 检测缺口。

### 对外 GUI 影响

IPC、HTTP SSE 和 gRPC 的 wire 格式保持 `zero.api.v1` / `zero.event.v1`，现有 GUI 不需要因本次待发布变更修改帧解析。GUI 仍需遵守 `0.0.15-rc` 建立的快照与增量合并规则。

## `0.0.15-rc`

### Flow 订阅改为“基线 + 增量”

包含任一 flow 生命周期事件的 IPC/SSE/CLI 实时订阅，在订阅确认后先收到 `flow.snapshot`：

1. 使用 `payload.records` **替换**当前活动连接集合；
2. 记录快照 `watermark`；
3. 按 `flow_id + revision` 合并后续 `flow.started`、`flow.routed`、`flow.updated`；
4. 收到 `flow.completed` 后从活动集合移除，并由 GUI 自行保存需要展示的历史；
5. 不把 `recent_flows` 当作断线重建或长期历史数据库。

`flow.snapshot` 是同步基线，不进入事件环，也不会投递到 JSONL/Webhook。`flow.completed.payload.record` 是自包含完成事实，新客户端应优先解析 `record`，同时容忍旧内核没有该字段。

### GUI 兼容分支建议

| 内核版本 | GUI 行为 |
|----------|----------|
| `< 0.0.15-rc` | 使用 `active_flows` 查询作为活动连接基线，并兼容旧 flow payload |
| `>= 0.0.15-rc` | 等待 subscribe ACK 和 `flow.snapshot`，之后按 revision 合并增量 |

## 新增条目的要求

后续每个破坏性或语义性变更必须在发布前补充：

- 首个受影响版本；
- 影响的通道和消费者；
- 旧语义与新语义；
- wire 标识是否变化；
- 兼容窗口和可检测条件；
- GUI/SDK/面板的明确迁移步骤；
- 对应回归测试位置。

开发期间只在版本矩阵和 `## Unreleased` 下登记，不预判最终发布版本，也不写入 Cargo 的 `-dev` 构建号。完整测试通过后，由 `scripts/release.ps1` 或 `scripts/release.sh` 将矩阵行、章节标题和 workspace 版本一起封板；禁止手工分别修改这些位置。
