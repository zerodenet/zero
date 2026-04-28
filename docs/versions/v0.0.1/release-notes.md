# v0.0.1 Release Notes

这是 `v0.0.x` 预发布线的一版，不按完整产品定义。

这一版的目标是把最小代理链路、最小 UDP 链路、本地可用性、云端最小节点和基础观测能力先打通。

## 这一版能做什么

- 跑 `SOCKS5`
- 跑 `HTTP CONNECT`
- 跑 `mixed` 同端口多协议入站
- 跑 `SOCKS5 UDP ASSOCIATE`
- 按规则走 `direct`、`block` 或上游 `SOCKS5`
- 从本地文件加载 `rule_sets`
- 通过 `selector` 组做最小手动选节点
- 导出本地只读状态

## 这一版的 UDP

UDP 当前通过 `SOCKS5 UDP ASSOCIATE` 进入，支持：

- `direct`
- `block`
- 上游 `SOCKS5` UDP 转发

这一版已经补上：

- 同一条本地 `UDP ASSOCIATE` 会话复用上游 `SOCKS5` UDP association
- 上游 UDP association 默认 `30s` 空闲超时
- 可通过 `runtime.udp_upstream_idle_timeout_seconds` 调整超时
- `status --json` 可导出上游 UDP association 统计

## 这一版的会话观测

当前状态导出已经支持：

- 活动会话累计上下行字节：`bytes_up` / `bytes_down`
- 活动会话 1 秒采样吞吐：`throughput_up_bps` / `throughput_down_bps`
- 最近完成会话结算：`recent_completed_sessions`

口径固定为：

- `bytes_*` 是事实源
- `throughput_*_bps` 是采样吞吐
- 完成会话和完成日志只保留结算值

## 启动

本地运行：

```powershell
target\release\zero.exe run examples\v0.0.1\basic.json
```

云端最小节点：

```powershell
target\release\zero.exe run examples\v0.0.1\server-socks5.json
```

查看状态：

```powershell
target\release\zero.exe status --json examples\v0.0.1\basic.json
```

## 示例配置

- [basic.json](../../../examples/v0.0.1/basic.json)
- [mixed.json](../../../examples/v0.0.1/mixed.json)
- [blocked-route.json](../../../examples/v0.0.1/blocked-route.json)
- [chained-socks5.json](../../../examples/v0.0.1/chained-socks5.json)
- [global-selector.json](../../../examples/v0.0.1/global-selector.json)
- [rule-set-files.json](../../../examples/v0.0.1/rule-set-files.json)
- [server-socks5.json](../../../examples/v0.0.1/server-socks5.json)
- [udp-socks5.json](../../../examples/v0.0.1/udp-socks5.json)

如果想看这一版没做什么，直接看 [known-limitations.md](known-limitations.md)。
