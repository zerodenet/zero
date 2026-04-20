# 2026-04-19 发布验证记录

当天做了两轮 release 验证。

做过的事：

- `status --json`
- `run --status-listen`
- `SOCKS5 -> direct`
- `HTTP CONNECT -> direct`
- `SOCKS5 -> SOCKS5 chained`
- `curl` 经 `SOCKS5`
- `curl` 经 `HTTP CONNECT`
- 规则阻断

结果：

- 成功链路都通了
- 阻断规则对真实客户端可见
- 运行态统计和请求结果能对上

额外发现：

- Windows PowerShell 写 JSON 可能带 UTF-8 BOM
- 配置解析后来补了 BOM 兼容

结论：

- 这版可以作为 `v0.0.x` 预发布基线继续推进
- 后面优先补可用性和稳定性，不按完整产品的标准来定义
