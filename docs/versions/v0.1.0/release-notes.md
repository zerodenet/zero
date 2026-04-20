# v0.1.0 发布说明

这版能做的事很简单：

- 跑 `SOCKS5`
- 跑 `HTTP CONNECT`
- 跑 `mixed` 同端口入站
- 按规则走 `direct`、`block` 或上游 `SOCKS5`
- 输出日志和状态

启动：

```powershell
target\release\zero.exe run examples\v0.1.0\basic.json
```

状态：

```powershell
target\release\zero.exe status --json examples\v0.1.0\basic.json
```

示例配置：

- [basic.json](/C:/Users/Administrator/develop/rs/zero-new/examples/v0.1.0/basic.json)
- [mixed.json](/C:/Users/Administrator/develop/rs/zero-new/examples/v0.1.0/mixed.json)
- [blocked-route.json](/C:/Users/Administrator/develop/rs/zero-new/examples/v0.1.0/blocked-route.json)
- [chained-socks5.json](/C:/Users/Administrator/develop/rs/zero-new/examples/v0.1.0/chained-socks5.json)

如果想知道这版没做什么，直接看 [known-limitations.md](/C:/Users/Administrator/develop/rs/zero-new/docs/versions/v0.1.0/known-limitations.md)。
