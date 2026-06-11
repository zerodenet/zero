# Mieru

Mieru 是 `partial` 协议能力。基线 TCP stream 和 UDP associate 路径存在。模块结构与 `protocols/mieru/src/` 对应。

## 当前能力

| 能力 | 状态 | 说明 |
|------|------|------|
| TCP 入站 | `partial` | 完整握手：多用户密钥尝试、openSessionRequest/Response、UDP 会话检测 |
| TCP 出站 | `partial` | 加密流上游 |
| UDP 入站 | `partial` | UDP associate 路径 |
| UDP 出站 | `partial` | 单跳及 TCP relay-prefix final-hop 路径 |
| MUX | `not_applicable` | 无独立 MUX 配置 |

## 剩余缺口

- 外部互操作覆盖不足
