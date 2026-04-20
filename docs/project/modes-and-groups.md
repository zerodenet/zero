# 模式和节点组

这份文档说的是模式和节点组设计。其中一部分已经在 `v0.0.1` 落地：

- `mode = rule | global | direct`
- `selector` 组

还没落地的部分：

- `urltest`
- `fallback`
- 运行时动态切换接口

目标很简单：入站尽量固定，节点尽量都放在出站里，真正变化的是“当前怎么选出站”。

## 基本拆分

- `inbounds`：固定监听口。典型就是一个 `mixed` 在 `127.0.0.1:7890`
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

长期建议至少有三类：

- `selector`：手动选择一个节点
- `urltest`：定时探测后选延迟更优的节点
- `fallback`：主节点不可用时自动切到下一个

客户端只负责改“当前选哪个”或“当前 mode 是什么”。真正的选择逻辑、健康检查和切换结果都应该在内核里。

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
      "tag": "proxy",
      "type": "selector",
      "outbounds": ["node-a", "node-b"],
      "default": "node-a"
    },
  ],
  "mode": {
    "type": "global",
    "outbound": "proxy"
  },
  "route": {
    "rules": [
      {
        "condition": { "type": "domain", "values": ["internal.local"] },
        "action": { "type": "route", "outbound": "direct" }
      }
    ],
    "final": { "type": "route", "outbound": "proxy" }
  }
}
```

## 边界

- 客户端负责交互，不负责转发实现
- 内核负责模式语义、节点组解析、健康检查和最终出站选择
- 云端最小节点不一定需要 `mode` 和 `outbound_groups`
- 本地用户侧一般最需要这套能力
