# CLI 控制命令

## 守护进程

```bash
zero run [--status-listen HOST:PORT] [--control-socket PATH] [--ipc-hook-socket PATH] [CONFIG_PATH]
```

| 选项 | 说明 |
|------|------|
| `--status-listen HOST:PORT` | HTTP 控制接口监听地址 |
| `--control-socket PATH` | IPC socket 路径（覆盖默认） |
| `--ipc-hook-socket PATH` | IPC flow hook socket（覆盖配置） |

IPC server 始终启动（不需要额外选项），默认路径：
- Linux/macOS: `~/.zero/control.sock`
- Windows: `\\.\pipe\zero-control`

## 控制命令

所有命令自动发现并连接运行中的 zero 守护进程。

### zero status

```bash
zero status               # 人类可读格式
zero status --json        # JSON 格式
zero status --socket /tmp/zero.sock  # 指定 socket
```

离线模式：指定配置路径时直接读取配置文件（不连接守护进程）：

```bash
zero status examples/v0.0.1/basic.json
```

### zero select

切换 selector 出站。

```bash
zero select proxy direct           # 将 proxy 组切换到 direct
zero select --socket /tmp/zero.sock proxy server-a
```

### zero flows

查询活动流列表（JSON）。

```bash
zero flows
```

### zero policies

查询所有策略组状态（JSON）。

```bash
zero policies
```

### zero events

实时追踪事件流（JSON-line，Ctrl-C 退出）。

```bash
zero events
```

输出示例：
```json
{"event_type":"flow.started","event_id":"...","occurred_at_unix_ms":...,"payload":{...}}
{"event_type":"flow.updated","event_id":"...","occurred_at_unix_ms":...,"payload":{...}}
{"event_type":"flow.completed","event_id":"...","occurred_at_unix_ms":...,"payload":{...}}
```

### zero help

```bash
zero help
```

## 退出码

| 码 | 说明 |
|-----|------|
| 0 | 成功 |
| 1 | 错误（socket 不存在、命令失败等） |
