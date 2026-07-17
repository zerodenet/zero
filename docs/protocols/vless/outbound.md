# VLESS 出站

VLESS 出站通过注册的 TCP 和 UDP 能力接入运行时。适配器把引擎解析后的出站投影为 VLESS 传输叶子；载体打开和协议握手分属传输层与协议层。

## TCP

1. 运行时解析直连或中继最终跳。
2. VLESS 适配器准备连接或 relay-hop 操作。
3. `zero-transport` 打开选定载体。
4. `protocols/vless` 完成 VLESS 请求与响应处理。
5. 运行时统一归一化结果、错误和流量记录。

## UDP

VLESS UDP 流计划属于 `protocols/vless` 的 `udp` 模块。传输桥保存协议流计划和中立载体选项，通用运行时只管理流生命周期、中继顺序和计量。
