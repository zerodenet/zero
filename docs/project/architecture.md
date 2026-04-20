# 分层

当前仓库按下面几层看就够了。

## 应用层

- 根包 `zero`

负责参数、配置文件路径、进程启动和状态输出。

## 配置和执行层

- `zero-config`
- `zero-engine`
- `zero-router`

`zero-config` 只管配置模型和解析。`zero-engine` 只管执行、编排、统计和内建动作。`zero-router` 只管规则匹配。

## 协议层

- `zero-core`
- `protocols/*`

`zero-core` 放通用类型和接口。具体协议放在 `protocols/*`。

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
