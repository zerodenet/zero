# API 兼容性承诺

Zero 控制面遵循 **只增不改** 的演化原则。在 `zero.api.v1` 生命周期内，以下承诺对所有消费者（HTTP / IPC / CLI / FFI）生效。

---

## 不可变（breaking → 需要 v2）

以下变更**仅在主版本升级时发生**，且必须通过 `api_version` 字段（如 `zero.api.v2`）显式通知消费者：

| 操作 | 示例 | 影响 |
|------|------|------|
| 删除或重命名 enum 变体 | 移除 `QueryResponse::Sinks` | 旧消费者反序列化失败 |
| 删除或重命名 struct 字段 | 移除 `FlowSnapshot.protocol` | 旧消费者丢失数据 |
| 改变信封格式 | 移除 `api_version` 字段 | 所有消费者断连 |
| 删除端点 | 移除 `GET /api/v1/stats` | 调用方收到 404 |
| 改变默认值 | `limit` 默认从 100 改为 50 | 行为变化 |
| 改变错误码 | `not-found` 改为 `not_found` | 消费者分支逻辑失效 |
| 改变认证协议 | Bearer → mTLS-only | 所有 HTTP 客户端断连 |

## 安全（additive → v1 内允许）

以下变更**不破坏**现有消费者，可在 v1 内自由发生：

| 操作 | 消费者影响 | 备注 |
|------|-----------|------|
| 新增 struct 字段 | 旧消费者忽略（`#[serde(default)]`） | 新字段必须有默认值 |
| 新增 enum 变体 | 使用 `RawResponse` 的消费者忽略 | 使用 `zero-api` 强类型的消费者需更新 |
| 新增端点 | 旧消费者不调用 | `capabilities` 端点反映新能力 |
| 新增事件类型 | 旧消费者过滤掉或忽略 | `event_type` 是字符串，天然可扩展 |
| 新增 command method | 旧服务器返回 `unsupported` | 旧客户端不发送新命令 |
| 新增错误码 | 旧消费者走 `default` 分支 | 消费者应 `match` + `_ =>` 兜底 |
| 新增 Query 类型 | 旧服务器返回 `invalid_argument` | 旧客户端不发送新查询 |
| 新增 HTTP 响应头 | 旧消费者忽略 | 如 `Retry-After` |

---

## 版本识别

| 信号 | 位置 | 用途 |
|------|------|------|
| `api_version` | 每个响应信封 | 判断 API 主版本 |
| `schema_version` | 每个事件信封 | 判断事件格式版本 |
| `GET /api/v1/capabilities` | 能力端点 | 探测具体能力、features、编译选项 |

消费者应在初始化时读取 `api_version` 和 `capabilities`，据此决定可用功能集。

---

## 推荐接入模式

### IPC 消费者

使用 `RawResponse`（`serde_json::Value`）反序列化响应，不依赖 `zero-api` 的枚举定义。这样新增的 Query/Command 变体不会导致反序列化失败：

```rust
// 推荐：类型安全 + 演化安全
let resp: RawResponse = send_request(&sock, &request)?;
if resp.ok {
    if let Some(result) = resp.result {
        // 按需提取字段，不依赖完整枚举
    }
}
```

### HTTP 消费者

按端点逐个解析 `result` 对象。每个端点返回的 struct 独立演化，新增字段被静默忽略。

### 事件消费者

以 `event_type` 字符串做分支处理，不依赖枚举。未知类型直接跳过：

```rust
match event.event_type.as_str() {
    "flow.completed" => { /* handle */ },
    "policy.selected" => { /* handle */ },
    _ => { /* unknown event, skip */ }
}
```

---

## 废弃策略

- 兼容端点（`/status`, `/config`, `/runtime`, `POST /selectors/...`）标注为 deprecated
- 废弃端点保留至少一个大版本周期
- 正式废弃通过 `capabilities.features` 列表标注
