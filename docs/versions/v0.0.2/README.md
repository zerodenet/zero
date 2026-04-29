# v0.0.2

这一版继续留在 `v0.0.x` 预发布线，不按正式产品定义。

`v0.0.1` 已经把最小代理链路、本地/云端使用方式、UDP 和基础观测打通。`v0.0.2` 继续补节点组能力，当前已经落地：

- `selector` 运行时切换
- `fallback`
- `group -> group`
- `urltest`
- API 事件目录、`flow.completed` 事件源和可选 connector 配置骨架
- feature-gated webhook/jsonl 事件 sink，用于面板或本地观测对接
- VLESS TCP/TLS 入站/出站第一阶段，含 UUID 用户校验和 `principal_key` 观测归因
- `zero-proxy` 运行层拆分，`zero-engine` 收敛为决策、状态和观测核心

先看这些：

- [requirements/mvp-scope.md](requirements/mvp-scope.md)
- [requirements/acceptance.md](requirements/acceptance.md)
- [planning/milestones.md](planning/milestones.md)
