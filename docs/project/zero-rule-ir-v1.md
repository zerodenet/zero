# Zero Rule IR v1

## 1. 范围

Zero Rule IR 是 Zero 自身的规则交换协议。它只描述一个 MatcherSet，不描述来源格式、订阅、下载、RouteRule、优先级、action、outbound 或更新时间。

Clash、sing-box、Surge 等格式必须由外部转换器转换为本协议；`zero-rule` 不识别这些来源格式。

## 2. 文档结构

编码使用 UTF-8 JSON。顶层对象必须严格符合：

```json
{
  "version": 1,
  "name": "AI services",
  "rules": [
    { "type": "domain_exact", "value": "api.example.com" },
    { "type": "domain_suffix", "value": "example.org" },
    { "type": "domain_keyword", "value": "special" },
    { "type": "ipv4_cidr", "value": "10.0.0.0/8" },
    { "type": "ipv6_cidr", "value": "fd00::/8" }
  ]
}
```

字段：

| 字段 | 必需 | 类型 | 说明 |
|---|---:|---|---|
| `version` | 是 | 无符号整数 | v1 固定为 `1` |
| `name` | 否 | 字符串 | 可选展示名称，编译后 UTF-8 不超过 63 字节 |
| `rules` | 是 | 数组 | MatcherSet 成员，内部为 OR 语义 |

未知顶层字段、规则字段、版本和规则类型必须拒绝，不能静默忽略。

## 3. 规则语义

- `domain_exact`：只匹配规范化后完全相同的域名。
- `domain_suffix`：匹配自身及标签边界内的子域名，不匹配字符串尾部巧合相同的域名。
- `domain_keyword`：在规范化后的域名中进行 ASCII 小写子串匹配；输入必须为
  非空 ASCII，不能包含 NUL。
- `ipv4_cidr`：IPv4 CIDR。
- `ipv6_cidr`：IPv6 CIDR。

同一 MatcherSet 内任一成员命中即返回 true。域名与调用方已经持有的目标 IP 可以同时参与查询；规则库不会主动执行 DNS。

公共检索 API 同时提供布尔匹配和命中类别。命中类别只描述
`domain_exact`、`domain_suffix`、`domain_keyword`、IPv4 range 或 IPv6 range，
不包含 RouteRule、action 或来源信息。多类规则同时命中时使用稳定顺序：
domain exact、domain suffix、domain keyword、IPv4、IPv6；查询同时携带域名和
目标 IP 时优先报告域名命中。

## 4. 规范化

域名编译与查询必须使用相同流程：去除首尾空白、去除末尾点、IDNA 转 ASCII、ASCII 小写化，并校验总长度、标签长度和空标签。

编译器在单个 MatcherSet 内执行确定性排序、重复删除、suffix 覆盖消除以及 CIDR 区间合并。不同 MatcherSet 之间不得跨集合消除规则。

## 5. 资源限制

- JSON 输入：最大 64 MiB。
- 规则数量：最大 4,000,000。
- 单条 `value`：最大 4096 UTF-8 字节。
- 展示名称：有效内容最大 63 UTF-8 字节，不允许 NUL。

协议层通过不代表语义编译必然通过。例如空规则集合、非法域名和非法名称仍会被编译器拒绝。
同样的 4,000,000 条上限也由公共编译器执行，因此直接构造 Rust `RuleSet`
不能绕过资源限制。

## 6. 兼容规则

v1 文档必须显式写入 `version: 1`。新增可选字段、改变现有语义或增加规则类型都需要新的协议版本；实现不得在 v1 下自行扩展并期待其他实现忽略。

Rust 参考入口位于 `zero_rule::protocol::{decode_json, encode_json}`。

真实规则基准可以直接使用本协议文件：

```powershell
$env:ZERO_RULE_IR = "C:\path\rules.json"
cargo bench -p zero-rule --bench zrs_pipeline
```

该基准会校验内存 matcher、借用字节的 ZRS matcher 与真实临时文件 mmap
matcher 的采样结果完全一致，并报告编译、编码、验证、映射、索引占用、
根页预热和三种查询路径的耗时。
