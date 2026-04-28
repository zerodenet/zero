# 分层

当前仓库按下面几层看就够了。

## 应用层

- 根包 `zero`
- `zero-api`

负责参数、配置文件路径、进程启动和状态输出。

控制面和观测模型以 Zero 自有规范为准。Clash、sing-box、Xray 等外部生态只作为设计参考；兼容能力应放在 adapter、gateway 或额外工具里，不反向约束内核和长期 API。

`zero-api` 负责定义对外控制、观测和事件导出能力，不等同于 HTTP 服务，也不按传输形态拆散能力。HTTP/HTTPS、本地 IPC、file、gRPC、二进制帧、Rust API 和 FFI 都应作为 trait 实现或 feature-gated adapter/sink 挂到同一套核心能力上。

## 配置和执行层

- `zero-config`
- `zero-engine`
- `zero-router`

`zero-config` 只管配置模型和解析。`zero-engine` 只管执行、编排、统计和内建动作。`zero-router` 只管规则匹配。

像 `direct / global / rule` 这种模式语义、`selector / urltest / fallback` 这种出站组语义，也属于这层，不属于客户端。

`zero-engine` 内部当前再按执行边界拆成：

- `RuntimeConfig`
  - 面向配置和 `serde`
- `EnginePlan`
  - 面向不可变执行结构
- `EngineState`
  - 面向运行时可变状态
- `view`
  - 面向 `status` / 导出 / 日志里的 tag 渲染

热路径优先读 plan/state，并尽量沿借用边界传递引用；只有控制面和展示面才回到字符串 tag。

## 协议层

- `zero-core`
- `protocols/*`

`zero-core` 放通用类型和接口。具体协议放在 `protocols/*`。

协议层按 feature 接入 `zero-engine`。核心内核始终编译，协议和控制面按需选择性编译，避免把嵌入式场景不需要的模块一起带进去。

## 抽象层

- `zero-traits`

只放 I/O、DNS 之类的抽象，不绑具体运行时。

## 平台层

- `zero-platform-tokio`
- 其他预留平台目录

当前只有 Tokio 后端是落地的。

## 依赖方向

只允许往下依赖：

- `zero` 可以依赖 `config`、`engine`
- `engine` 可以依赖 `config`、`router`、`protocols/*`、平台层
- `protocols/*` 可以依赖 `core`
- `core` 可以依赖 `traits`

不要反过来。
