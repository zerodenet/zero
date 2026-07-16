# ZRS 0.1 二进制格式

## 1. 范围与端序

ZRS 表示一个不可变 MatcherSet。所有整数使用 little-endian。0.1 是预稳定版本，但相同版本下的字节解释固定不变。

ZRS 不包含 RouteRule、action、下载地址、订阅元数据或外部格式信息。

## 2. Header

Header 固定为 128 字节：

| Offset | Size | 字段 | 0.1 约束 |
|---:|---:|---|---|
| 0 | 4 | magic | ASCII `ZRS!` |
| 4 | 2 | major | `0` |
| 6 | 2 | minor | `1` |
| 8 | 2 | header_size | `128` |
| 10 | 2 | section_count | 当前 writer 写 `5`，reader 上限 `64` |
| 12 | 4 | reserved | 全零 |
| 16 | 8 | file_size | 必须等于实际文件长度 |
| 24 | 4 | body_checksum | Header 之后全部字节的 IEEE CRC-32 |
| 28 | 4 | reserved | 全零 |
| 32 | 64 | display_name | UTF-8、NUL 结尾、剩余填零；全零表示未设置 |
| 96 | 32 | reserved | 全零 |

## 3. Section Directory

目录紧随 Header，每项固定 24 字节：

| 相对 Offset | Size | 字段 |
|---:|---:|---|
| 0 | 2 | kind |
| 2 | 2 | encoding |
| 4 | 4 | flags；bit 0 为 required |
| 8 | 8 | section offset |
| 16 | 8 | section length |

section 起始 offset 必须按 8 字节对齐，不能越界或互相重叠。未知 required section 必须拒绝；未知 optional section 可以跳过。

0.1 定义：

| kind | 数值 | encoding | 数值 |
|---|---:|---|---:|
| `DOMAIN_EXACT` | 1 | `FST_SET_V1` | 1 |
| `DOMAIN_SUFFIX` | 2 | `FST_SET_V1` | 1 |
| `DOMAIN_KEYWORD` | 3 | `STRING_TABLE_V1` | 2 |
| `IPV4_RANGE` | 4 | `IPV4_RANGE_V1` | 3 |
| `IPV6_RANGE` | 5 | `IPV6_RANGE_V1` | 4 |

kind 表示规则语义，encoding 表示物理索引，两者不能混用。

## 4. Section 编码

`FST_SET_V1` 使用 Rust `fst` 0.4 系列 raw set 字节格式。键必须是已规范化、严格递增且唯一的 UTF-8 字节串。参考 writer 固定使用 `fst` 0.4.7；独立实现应使用 golden vector 验证兼容性。
参考向量见 `docs/project/zrs-0.1-golden.md`。

`STRING_TABLE_V1`：

```text
u32 count
u32 reserved = 0
u64 offsets[count + 1]
u8  concatenated_utf8_strings[]
```

`reserved` 必须为零。第一个 offset 必须为 0，offset 单调不减，最后一个
offset 必须等于字符串区长度，每对相邻 offset 必须构成合法 UTF-8。每个值
还必须符合对应语义 kind 的 Zero 规范化形式；0.1 中该 section 保存 domain
keyword，因此不能是空值、包含 NUL 或包含 ASCII 大写字符。

`IPV4_RANGE_V1`：`u32 count`、`u32 reserved`，随后是 `count` 个 `{start: u32, end: u32}`。

`IPV6_RANGE_V1`：`u32 count`、`u32 reserved`，随后是 `count` 个 `{start: u128, end: u128}`。

两个 range encoding 的 `reserved` 都必须为零。range 按 start 严格递增，
`start <= end`，且不能重叠。地址整数使用网络地址对应的无符号数值，再按
ZRS little-endian 写入。

## 5. 验证与限制

`Structure` 模式验证 Header、目录、FST 可加载性、字符串表和 range table 的
内存安全结构，但不会遍历大型 FST 的全部键或 range 的全部排序关系。
`Semantic` 在此基础上全量验证 domain/keyword 是否已经规范化，以及 range
是否有序、有效且不重叠。`FullChecksum` 执行相同语义验证，并扫描
Header 后全部字节验证 CRC-32。下载与安装阶段应使用 `FullChecksum`；可信缓存
在进程启动时可以使用 `Structure`，以保留 mmap 惰性分页收益。

0.1 reader 限制：文件最大 1 GiB、section 最大 512 MiB、section 数量最大 64、每类条目最大 4,000,000。实现可以使用更低的本地策略限制，但不能接受结构非法的文件。
参考 writer 在构建 FST 或字符串表之前预检条目数、源字符串总字节数和可计算
的编码长度，避免先发生超大分配再返回资源限制错误。

参考 verifier 的回归测试覆盖完整截断、逐字节破坏、reserved 字段污染、
非规范化字符串以及确定性任意字节输入。独立 reader 也必须保证不可信输入只会
产生受控错误，不能越界访问或 panic。

## 6. mmap 生命周期

映射期间文件必须不可变，禁止原地 truncate 或 overwrite。发布者必须写入新文件、完成验证后原子替换路径；已经打开的 matcher 继续持有旧映射，直到所有查询释放。

Rust 参考入口：

- `zero_rule::zrs::encode`
- `zero_rule::zrs::verify`
- `zero_rule::zrs::VerifiedRuleSet`
- `zero_rule::zrs::MappedRuleSet`

规则库只提供编码、验证、映射、预热和匹配能力，不负责文件下载、安装调度或版本切换。
`RuleSetMetadata` 提供格式版本、展示名称、文件大小、checksum、各语义类型条目数
以及各物理 section 的字节占用；调用方不应自行重新解析 section directory 来
构造 inspect 或容量诊断。
