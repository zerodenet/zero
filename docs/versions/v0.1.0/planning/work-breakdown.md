# 开发顺序

建议顺序：

1. `zero-traits`
2. `zero-core`
3. `zero-config`
4. `zero-platform-tokio`
5. `protocols/socks5`
6. `zero-engine` 内的 `direct` / `block`
7. `zero-router`
8. `protocols/http-connect`
9. `mixed inbound`
10. 根包 `zero`
11. 测试、示例、文档

原则只有两个：

- 先把下层做稳
- 先跑通主链路，再补附属能力
