# 文档

仓库文档分层：

- `project/`：长期约定，记录项目定位、分层和工程规则。
- `control-plane-api/`：外部控制面、事件、IPC、HTTP 和 CLI 接口参考。
- `control-plane/`：历史设计与方案背景，不作为当前 API 契约。
- `protocols/`：协议能力、配置速查和协议实现说明。
- `guides/`：面向使用者和 GUI 对接者的操作指南。
- `testing/`：需要特定平台或权限的验证方案。

维护时按这个规则来：

- 改长期定位或分层，先改 `project/`
- 改对外接口、配置形态或事件结构，同步更新 `control-plane-api/` 和相关指南
- 改协议能力，同步更新 `protocols/` 和 `project/protocol-capabilities.md`
- 修改后在 `docs/` 目录运行 `npm run check`
