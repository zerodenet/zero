# 2026-04-22 发布验证记录

这轮主要补的是 `rule_sets` 收口验证。

做过的事：

- `cargo fmt --all`
- `cargo check --workspace`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets`
- `cargo run -- status --json examples/v0.0.1/rule-set-files.json`
- 新增二进制级 smoke：`rule_sets` 文件规则

重点验证：

- `rule-set-files.json` 能从配置文件相对路径加载 `rules/ads.txt` 和 `rules/lan.txt`
- `blocked.example` 经 `SOCKS5` 命中 `rule-set` 后返回阻断
- `127.0.0.1/8` 命中 `cidr-list` 后能直连到本地目标

结果：

- 文件规则加载正常
- 域名阻断和 CIDR 直连都通过
- 自动化和状态导出都能覆盖当前实现

结论：

- `v0.0.1` 当前功能面已经基本收齐
- 后续如果继续留在 `v0.0.1`，只建议收 bugfix、文档和少量验证补丁
