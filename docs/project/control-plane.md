# 控制面规范

Zero 的控制面和观测模型以自有核心规范为准。

Clash、sing-box、Xray 等项目只作为行业经验参考，不作为 Zero 内核的 API 契约来源。项目可以在外层提供兼容适配，但内核、SDK、面板和客户端应优先围绕 Zero 自己的统一模型设计。

## 原则

- 核心规范先行，外部兼容后置
- 内核能力不继承外部项目的历史包袱
- 面板、客户端、SDK 面向 Zero 规范，而不是面向某个第三方 API
- Clash / sing-box / Xray 兼容可以作为 adapter、gateway 或额外工具存在
- adapter 不应反向约束 `zero-engine`、配置模型、运行时状态和长期 API

## 边界

核心规范负责定义：

- 配置模型
- 运行时状态模型
- flow 观测模型
- policy 状态和控制动作
- 统计口径
- 错误和事件语义

## 命名

长期控制面命名优先使用 Zero 自己的核心概念：

- `flow`：一次可观测流量生命周期。TCP connection 是 flow，SOCKS5 UDP 到同一目标的报文序列也是 flow
- `outbound`：具体出站能力，例如 `direct`、`block`、`socks5`
- `policy`：选择或组合策略，例如 `selector`、`fallback`、`urltest`
- `target`：路由和模式引用的统一目标，可以指向 outbound 或 policy
- `route`：匹配条件到 target 的决策规则
- `event`：面向控制面和面板消费的运行时事件

不要把内部实现名或过渡配置名固化为长期 API：

- `Session` / `sessions` 是当前内核实现细节，对外应使用 `flow` / `flows`
- `outbound_groups` 是当前配置过渡命名，对外长期规范应使用 `policy` / `policies`

外部适配层负责处理：

- 第三方 API 路径和字段名
- 第三方面板协议
- 历史兼容字段
- 语义不完全一致时的映射和降级

## 当前阶段

当前 `--status-listen` 提供的是最小本地控制入口，不是长期 API 定稿。

在正式展开 `zero-api` 前，新增观测或控制能力应先回答：

- 这个字段是否属于 Zero 核心规范
- 它是否能被面板、客户端和 SDK 长期复用
- 它是否只是某个外部生态的兼容字段
- 如果是兼容字段，是否应该放在 adapter 而不是内核导出里

## 参考主流项目的方式

可以参考主流项目解决过的问题：

- Clash 的连接列表、代理组选择和流量观测
- sing-box 的多出站组、规则和平台边界
- Xray 的入站/出站统计和服务化 API

但参考的是问题拆分和成熟口径，不是直接继承 endpoint、字段名和历史行为。
