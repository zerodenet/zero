# 模式和节点组

这份文档说的是模式和节点组设计。当前已经落地：

- `mode = rule | global | direct`
- `selector`
- `selector` 运行时切换
- `fallback`
- `group -> group`
- `urltest`

还没落地的部分：

- 更复杂的健康检查调度

目标很简单：入站尽量固定，节点尽量都放在出站里，真正变化的是“当前怎么选出站”。

## 基本拆分

- `inbounds`：固定监听口。典型是一个 `mixed` 在 `127.0.0.1:7890`
- `outbounds`：具体节点或内建动作，比如 `node-a`、`node-b`、`direct`、`block`
- `outbound_groups`：对一组出站做统一选择
- `mode`：决定当前流量按什么方式走

## mode

建议至少有三种：

- `direct`：全部直连
- `global`：全部走某个指定组或节点
- `rule`：先看规则，没命中再走默认组或节点

这里的 `global` 和 `rule` 都是内核能力，不是客户端自己做转发。

## outbound_groups

当前已经实现三类：

- `selector`
  - 手动指定当前成员
  - 当前支持运行时切换
- `fallback`
  - 前一个成员建连失败时，顺序切到下一个
- `urltest`
  - 周期探测后，选择可用且延迟更低的成员

这三类组的成员现在都可以引用另一个组。运行时会递归解析，配置阶段会拦掉循环引用。

客户端只负责改“当前选哪个”或“当前 mode 是什么”。真正的选择逻辑、健康检查和最终出站决策都在内核里。

当前本地最小控制入口复用了 `--status-listen`，支持：

```text
POST /selectors/{group_tag}/{target_tag}
```

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
      "type": "urltest",
      "outbounds": ["fallback-proxy", "node-b", "direct"],
      "url": "http://example.com/",
      "interval_seconds": 300
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
