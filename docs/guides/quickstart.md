# 快速开始

5 分钟从零到代理跑起来，并通过控制面管理。

## 1. 构建

```bash
git clone https://github.com/zerodenet/zero.git
cd zero
cargo build --release
```

## 2. 最小配置

`config.json`：

```json
{
  "inbounds": [
    {
      "tag": "socks-in",
      "listen": { "address": "127.0.0.1", "port": 1080 },
      "protocol": { "type": "mixed" }
    }
  ],
  "outbounds": [
    { "tag": "direct", "protocol": { "type": "direct" } },
    {
      "tag": "proxy",
      "protocol": {
        "type": "vless",
        "server": "your-server.com",
        "port": 443,
        "id": "your-uuid",
        "tls": { "server_name": "your-server.com" }
      }
    }
  ],
  "route": {
    "mode": { "type": "global", "outbound": "proxy" },
    "rules": [],
    "final": { "type": "direct" }
  },
  "runtime": {
    "log": { "level": "info" }
  }
}
```

## 3. 启动

```bash
./target/release/zero run config.json
```

输出：

```
engine started  version=0.0.2
loaded proxy configuration  config=config.json
ipc server ready  socket=/home/user/.zero/control.sock
```

代理已运行。IPC socket 自动创建在 `~/.zero/control.sock`。

## 4. 验证

```bash
# CLI 查状态
./target/release/zero status

# curl 查状态（HTTP 在 status-listen 启用时可用）
curl -s http://127.0.0.1:9090/api/v1/runtime

# 设浏览器 SOCKS5 代理到 127.0.0.1:1080
```

## 5. 带控制面的完整启动

```bash
# HTTP + IPC 双通道
./target/release/zero run --status-listen 127.0.0.1:9090 config.json

# 实时事件流
./target/release/zero events

# 切换节点
./target/release/zero select proxy direct
```

## 6. 常见场景

### 分流路由

```json
{
  "route": {
    "mode": { "type": "rule" },
    "rules": [
      {
        "condition": { "type": "domain", "values": ["geosite:cn"] },
        "action": { "type": "direct" }
      }
    ],
    "final": { "type": "route", "outbound": "proxy" }
  }
}
```

### 故障转移

```json
{
  "outbound_groups": [
    {
      "tag": "fallback-proxy",
      "type": "fallback",
      "outbounds": ["server-a", "server-b", "direct"]
    }
  ]
}
```

### 延迟选优

```json
{
  "outbound_groups": [
    {
      "tag": "auto",
      "type": "urltest",
      "outbounds": ["server-a", "server-b"],
      "url": "http://www.gstatic.com/generate_204",
      "interval_seconds": 300
    }
  ]
}
```

### 链式代理

```json
{
  "outbound_groups": [
    {
      "tag": "hk-us",
      "type": "relay",
      "proxies": ["hk-vless", "us-socks5"]
    }
  ]
}
```

## 下一步

- [完整配置参考](../control-plane-api/configuration.md)
- [GUI 接入指南](gui-integration.md)
- [控制面 API 参考](../control-plane-api/README.md)
