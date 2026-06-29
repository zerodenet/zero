# 模式和节点组

这份文档说的是模式和节点组设计。当前已经落地：

- `mode = rule | global | direct`
- `selector`
- `selector` 运行时切换（IPC + CLI + HTTP）
- `mode` 运行时切换（`zero mode rule|direct|global <outbound>`）
- `fallback`
- `group -> group`
- `url_test`
- `reload` 热加载（route + mode + DNS 热换，inbounds/outbounds 需重启）

还没落地的部分：

- Inbounds/outbounds 热切换

目标很简单：入站尽量固定，节点尽量都放在出站里，真正变化的是“当前怎么选出站”。

## 基本拆分

- `inbounds`：固定监听口。典型是一个 `mixed` 在 `127.0.0.1:7890`
- `outbounds`：具体节点或内建动作，比如 `node-a`、`node-b`、`direct`、`block`
- `outbound_groups`：对一组出站做统一选择
- `mode`：决定当前流量按什么方式走

## mode

支持三种模式，可运行时切换：

- `direct`：全部直连
- `global`：全部走某个指定组或节点
- `rule`：先看规则，没命中再走 `route.final`

模式在启动时从配置文件读取，启动后可通过多种方式热切换：

```bash
zero mode rule              # CLI
zero mode direct            # 全部直连
zero mode global proxy      # 全局走 proxy 出站
```

IPC 等价命令：

```json
{ "method": "mode.set", "params": { "mode": "global", "outbound": "proxy" } }
```

切换即时生效，所有新连接立刻使用新模式。

## outbound_groups

当前已经实现五类：

- `selector`
  - 手动指定当前成员
  - 当前支持运行时切换
- `fallback`
  - 前一个成员建连失败时，顺序切到下一个
- `url_test`
  - 周期探测后，选择可用且延迟更低的成员
- `relay`
  - 链式代理，流量依次经过每个节点
- `load_balance`
  - 负载均衡，按策略（round_robin / random）分发连接

这些组的成员现在都可以引用另一个组。运行时会递归解析，配置阶段会拦掉循环引用。

### 默认选择逻辑

Selector 组优先级：

```
selected > default > outbounds[0]
```

| 配置 | 行为 |
|------|------|
| `”selected”: “node-b”` | 启动即用 node-b |
| `”default”: “node-c”`（无 selected） | 启动用 node-c，切换后不再回头 |
| 都不配 | 用 `outbounds` 数组第一个 |

`default` 仅用于初始值——一旦通过 API/CLI 切换过，`default` 就不再生效。`selected` 是持久选择，重启后依然生效。

Fallback 和 URL test 固定从 `outbounds[0]` 开始。

客户端只负责改”当前选哪个”或”当前 mode 是什么”。真正的选择逻辑、健康检查和最终出站决策都在内核里。

当前本地控制入口使用 `POST /api/v1/commands`，selector 切换通过
`method: "policies.select"` 完成。

`policies.select` 的 `target_tag` 是 selector `outbounds` 里的直接成员 tag。
这个成员可以是普通 outbound，也可以是 `url_test`、`load_balance`、`fallback`、`relay`
或另一个 `selector`。控制层只改变这一层 selector 的选择，不展开嵌套组。例如：

```json
{
  "method": "policies.select",
  "params": {
    "policy_tag": "manual",
    "target_tag": "probe"
  }
}
```

这表示 selector `manual` 选中 url_test 组 `probe`。`probe` 内部最终走哪个成员，由
url_test 探测状态决定；需要刷新延迟时对 `probe` 发送 `policies.probe`，再通过
policy 查询或 `policy.probe.completed` 事件读取 `latency_ms` 和
`url_test_members[].latency_ms`。

## 配置草案

```json
{
  "inbounds": [
    {
      "tag": "mixed-in",
      "listen": { "address": "127.0.0.1", "port": 7890 },
      "protocol": { "type": "mixed" }
    }
  ],
  "outbounds": [
    { "tag": "direct", "protocol": { "type": "direct" } },
    { "tag": "node-a", "protocol": { "type": "socks5", "server": "1.2.3.4", "port": 1080 } },
    { "tag": "node-b", "protocol": { "type": "socks5", "server": "5.6.7.8", "port": 1080 } }
  ],
  "outbound_groups": [
    {
      "tag": "manual",
      "type": "selector",
      "outbounds": ["node-a", "node-b"],
      "selected": "node-a"
    },
    {
      "tag": "fallback-proxy",
      "type": "fallback",
      "outbounds": ["node-a", "direct"]
    },
    {
      "tag": "probe",
      "type": "url_test",
      "outbounds": ["fallback-proxy", "node-b", "direct"],
      "url": "http://example.com/",
      "interval_seconds": 300
    },
    {
      "tag": "chain-hk-us",
      "type": "relay",
      "proxies": ["node-hk", "node-us"]
    },
    {
      "tag": "lb",
      "type": "load_balance",
      "outbounds": ["node-a", "node-b", "node-c"],
      "strategy": "round_robin"
    }
  ],
  "mode": {
    "type": "global",
    "outbound": "probe"
  },
  "route": {
    "rules": [
      {
        "condition": { "type": "domain", "values": ["internal.local"] },
        "action": { "type": "route", "outbound": "direct" }
      }
    ],
    "final": { "type": "route", "outbound": "probe" }
  }
}
```

## 边界

- 客户端负责交互，不负责转发实现
- 内核负责模式语义、节点组解析、健康检查和最终出站选择
- 本地控制入口当前复用 `--status-listen`，提供最小写接口用于切换 `selector`
- 云端最小节点不一定需要 `mode` 和 `outbound_groups`
- 本地用户侧一般最需要这套能力
