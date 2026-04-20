# crate 目标

这版真正要落地的：

- `zero-traits`
- `zero-core`
- `zero-config`
- `zero-router`
- `zero-platform-tokio`
- `protocols/socks5`
- `protocols/http-connect`
- `zero-engine`
- 根包 `zero`

这版只留目录，不进交付：

- `zero-ffi`
- `zero-api`
- `zero-web`
- 其他平台后端
- 其他协议目录
- `zero-crypto-*`

职责别串：

- `config` 不管生命周期
- `engine` 不写协议状态机
- `protocols/*` 不写进程编排
