# 实现边界

这版不要越下面这些线。

## 功能

- 只做 TCP
- 只做 `socks5`、`http-connect`、`mixed`
- 只做 `direct`、`block`、`socks5` 上游
- 只做静态域名和 CIDR 规则
- 只做 JSON 配置

## 平台

- 只落 Tokio
- 其他平台目录先放着，不实现

## 分层

- `zero-traits` 不依赖运行时
- `zero-core` 不依赖平台
- `zero-config` 不做连接生命周期
- `protocols/*` 不做进程管理
- `zero-engine` 不做协议细节和字符串配置解析
- 根包 `zero` 只做入口和参数
