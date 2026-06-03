# Cloudflare Pages 部署指南

## 前置条件

1. Cloudflare 账户
2. GitHub 仓库 push 权限（用于设置 Secrets）

## 步骤

### 1. 获取 Cloudflare API Token

1. 登录 [Cloudflare Dashboard](https://dash.cloudflare.com/)
2. 右上角头像 → **My Profile** → **API Tokens** → **Create Token**
3. 选择 **Create Custom Token**，配置：
   - **Token name**: `zero-docs-deploy`
   - **Permissions**: `Account` → `Cloudflare Pages` → `Edit`
   - **Account Resources**: 选择你的账户
4. 复制生成的 Token

### 2. 获取 Account ID

在 Cloudflare Dashboard 首页右侧栏 → **Account ID**，复制。

### 3. 创建 Cloudflare Pages 项目

1. 进入 [Cloudflare Pages](https://dash.cloudflare.com/?to=/:account/pages)
2. **Create a project** → **Direct Upload** (不用 Git 连接，由 CI 负责推送)
3. **Project name**: `zero-docs`
4. 创建完成后记下项目名称

### 4. 在 GitHub 设置 Secrets

在仓库的 **Settings → Secrets and variables → Actions** 中添加：

| Name | Value |
|------|-------|
| `CLOUDFLARE_API_TOKEN` | 步骤 1 的 API Token |
| `CLOUDFLARE_ACCOUNT_ID` | 步骤 2 的 Account ID |

### 5. 初始化本地依赖

```bash
make docs-install
# 或: cd docs && npm install
```

生成 `docs/package-lock.json`（CI 需要），提交。

### 6. 测试构建

```bash
make docs-dev
# 浏览器打开 http://localhost:5173
```

## 自动部署

推送 `main` 分支且 `docs/**` 有变更时自动触发，也可手动在 Actions 页触发。

## 手动触发

1. GitHub → Actions → **Deploy Docs**
2. **Run workflow** → 选择分支 → **Run workflow**

## 自定义域名

1. Cloudflare Pages → zero-docs → **Custom domains**
2. 添加你的域名（如 `docs.zerodenet.org`）
3. Cloudflare 自动配置 DNS
