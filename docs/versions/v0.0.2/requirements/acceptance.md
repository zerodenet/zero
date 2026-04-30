# 验收

`v0.0.2` 至少满足下面这些：

## 功能

1. `fallback` 组可以通过配置解析和校验
2. `global mode -> fallback` 可用
3. `route -> fallback` 可用
4. `TCP` 在主出站不可达时，能自动切到下一个成员
5. `SOCKS5 UDP ASSOCIATE` 在主出站不可达时，能自动切到下一个成员
6. `fallback` 中包含 `block` 时，链路走到该成员会阻断
7. `selector` 当前选择可在运行时切换
8. 本地控制入口能成功切换 `selector` 组成员
9. 切换后新的 `TCP` 会话按新成员出站
10. 组成员可以引用另一个组
11. `TCP` 能通过嵌套组成功转发
12. `SOCKS5 UDP ASSOCIATE` 能通过嵌套组成功转发
13. 配置阶段能拒绝组循环引用
14. `urltest` 组可以通过配置解析和校验
15. `urltest` 能在后台探测后更新当前选中的成员
16. `global mode -> urltest` 可用
17. `urltest` 选中的成员可用于 `TCP`
18. `urltest` 选中的成员可用于 `SOCKS5 UDP ASSOCIATE`
19. 状态导出能看到 `urltest` 当前选择和最近一次探测结果
20. VLESS 入站和出站可选择 TLS
21. VLESS 入站和出站可选择 WebSocket
22. VLESS 入站和出站支持 TLS + WebSocket 组合（WSS）
23. `zero-engine` 不再承载监听、协议握手、relay 和 Tokio 运行时接线

## 工程

- `cargo check --workspace`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets`
- 至少提供一份 `fallback` 示例、一份 `nested-groups` 示例、一份 `urltest` 示例、VLESS TLS 示例和 VLESS WS 示例
