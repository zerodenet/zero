# TUN 端到端验证方案

## 前置条件

| 平台 | 要求 |
|------|------|
| Linux | `sudo ip tuntap add dev tun0 mode tun`（需要 root） |
| macOS | utun 自动创建，无需手动操作 |
| Windows | `wintun.dll` 在 `PATH` 或二进制同目录（需 Administrator） |

## Linux 端到端测试

### 1. 创建 TUN 设备

```bash
sudo ip tuntap add dev tun0 mode tun user $(whoami)
sudo ip addr add 10.0.0.1/24 dev tun0
sudo ip link set dev tun0 up
```

### 2. 启动 echo 服务器

```bash
# 在另一个终端
nc -l 127.0.0.1 8080
```

### 3. 配置 zero

```json
{
  "outbounds": [
    { "tag": "direct", "protocol": { "type": "direct" } }
  ],
  "route": {
    "rules": [],
    "final": { "type": "direct" }
  }
}
```

```bash
cargo run -- run config.json &
cargo run -- tun start --addr 10.0.0.1/24 --tag proxy config.json
```

### 4. 通过 TUN 发请求

```bash
# TUN 设备现在是 10.0.0.1，代理所有流量
# 从另一个网络命名空间或路由测试：
curl --interface tun0 http://httpbin.org/get
```

### 5. 验证：检查 zero 状态

```bash
cargo run -- status config.json
# 确认 TUN 状态 running=true，active_sessions > 0
cargo run -- tun status config.json
```

## macOS 端到端测试

```bash
# 1. utun 自动创建
cargo run -- run -c config.json &
# 2. 验证
cargo run -- tun status config.json
```

## 预期行为

| 步骤 | 预期 |
|------|------|
| `cargo run -- run` | TUN device created, "tun device created" 日志 |
| `cargo run -- tun status` | `running: true`, `name: <device_name>`, `addr: ...` |
| `cargo run -- status` | `active_sessions` 中有 TUN-tagged 会话 |
| `cargo run -- tun stop` | TUN 设备停止，`running: false` |

## 验证的三层握手

用 `tcpdump` 抓包可验证：

```bash
sudo tcpdump -i tun0 -nn -v
```

应该看到：
```
10.0.0.2.54321 > 10.0.0.1.443: Flags [S], seq 1000         # 客户端 SYN
10.0.0.1.443 > 10.0.0.2.54321: Flags [S.], seq 1000000, ack 1001  # 我们 SYN-ACK (MSS 1500)
10.0.0.2.54321 > 10.0.0.1.443: Flags [.], ack 1000001       # 客户端 ACK
# 连接建立 → 通过 serve_inbound() 代理
```

## 故障排查

| 问题 | 可能原因 | 排查 |
|------|---------|------|
| TUN 创建失败 | 缺少权限 | `sudo` 或 Administrator |
| 无数据流量 | 路由未配置 | `ip route` 检查 |
| checksum 错误 | checksum 计算 bug | `tcpdump -v` 检查校验和字段 |
| MSS 未协商 | build_tcp_with_mss 未调用 | 检查 SYN-ACK 包中是否有 MSS 选项（TCP options field）|
