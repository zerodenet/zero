---
title: Zero 文档
author: Zero 团队
---

# Zero 文档

文档站按读者任务组织，不根据文件夹名称自动生成菜单：

- `guides/`：快速开始、GUI 集成和配置错误处理。
- `project/`：配置参考、架构、格式规范和工程边界。
- `protocols/`：协议能力矩阵及各协议的入站、出站和编解码说明。
- `control-plane-api/`：当前有效的控制面配置、接口、事件和 IPC 契约。
- `control-plane/`：历史设计与方案背景，不作为当前对外契约。
- `testing/`：专项测试和验证记录。

公开导航在 `.vitepress/config.ts` 中显式维护。新增页面时必须同时指定所属分组，并运行 `npm run check`；完整站点构建使用 `npm run check:build`。
