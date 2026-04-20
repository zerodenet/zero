# 已知限制

这版不是全功能代理。

协议边界：

- `SOCKS5` 只支持 `no-auth`
- `SOCKS5` 只支持 `CONNECT`
- 不支持 `BIND`
- 不支持 `UDP ASSOCIATE`
- `HTTP` 只支持 `CONNECT`
- `mixed` 只识别 `SOCKS5` 和 `HTTP CONNECT`

传输边界：

- 只支持 TCP
- 不支持 UDP
- 不支持 TUN

产品边界：

- 没有完整 `zero-api`
- 没有 GUI
- 没有订阅
- 没有安装器

配置边界：

- 只支持 JSON
- 只支持静态域名和 CIDR 规则
- 不支持 GeoIP
- 不支持远程规则集
- 不支持热重载
