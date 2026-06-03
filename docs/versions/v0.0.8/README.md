# v0.0.8

稳定性修复与构建改进。

## 交付内容

- 移除平台特定 `package-lock.json`，改用 `npm install` 确保 docs 跨平台构建
- Release workflow gate on docs build：tag push 时先验证 docs 构建成功再发布
- 修复 docs 站点控制面导航 404（`README.md` → `index.md`）

## 不做什么

- 无新功能
- 无 API 变更
