# ZRS 0.1 Golden Vector

该向量用于验证独立 writer/reader 是否与 ZRS 0.1 参考实现产生一致字节。

输入 MatcherSet：

```json
{
  "version": 1,
  "name": "test rules",
  "rules": [
    { "type": "domain_exact", "value": "api.example.com" },
    { "type": "domain_suffix", "value": "service.example" },
    { "type": "domain_keyword", "value": "keyword" },
    { "type": "ipv4_cidr", "value": "10.0.0.0/8" }
  ]
}
```

输入必须先经过 Zero Rule IR v1 的规范化和编译，再编码为 ZRS。预期结果：

| 属性 | 预期值 |
|---|---|
| 文件长度 | `416` 字节 |
| 整文件 CRC-32 | `b61fc5a3` |
| 整文件 SHA-256 | `6c4af864f13631c75168a701897f2c91441e92128aa23d6f589dcefcb5587be6` |
| Header body checksum | `b7529134` |
| 索引 section 总长度 | `161` 字节 |

Rust 可执行断言位于 `crates/rule/tests/zrs.rs`。SHA-256 是协议兼容性
fingerprint；任何导致该值变化的 writer 修改都必须先判断是否需要提升 ZRS
版本，不能只更新测试期望值。
