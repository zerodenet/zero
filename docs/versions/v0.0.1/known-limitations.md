# 已知限制

这一版是 `v0.0.x` 预发布，不按完整产品或稳定版定义。

## 协议边界

- `SOCKS5` 支持 `no-auth` 和 username/password
- `SOCKS5` 支持 `CONNECT` 和 `UDP ASSOCIATE`
- 不支持 `BIND`
- `HTTP` 只支持 `CONNECT`
- `mixed` 只识别 `SOCKS5` 和 `HTTP CONNECT`

## 传输边界

- 支持 `TCP`
- UDP 只支持 `SOCKS5 UDP ASSOCIATE`
- UDP 不支持 `HTTP CONNECT`
- 不支持 `TUN`

## 观测边界

- 活动会话的吞吐是 1 秒采样值，不是更短窗口的平滑速率
- `process_id` / `process_name` 目前还没有接平台实现
- 完成会话当前只保留最近一段固定长度历史

## 产品边界

- 没有完整 `zero-api`
- 没有 GUI
- 没有订阅
- 没有安装器

## 配置边界

- 只支持 JSON
- 只支持静态域名和 CIDR 规则
- 不支持 GeoIP
- 不支持远程规则集
- 不支持热重载
