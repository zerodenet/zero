# 文档

仓库文档分层：

- `project/`：长期约定，记录项目定位、分层和工程规则。
- `control-plane-api/`：外部控制面、事件、IPC、HTTP 和 CLI 接口参考。
- `guides/`：面向使用者和 GUI 对接者的操作指南。

维护时按这个规则来：

- 改长期定位或分层，先改 `project/`
- 改对外接口、配置形态或事件结构，同步更新 `control-plane-api/` 和相关指南
- 代码不要先于文档越界
