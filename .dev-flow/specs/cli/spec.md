# cli Specification

## Purpose
`alter` 命令行，agent 与用户共用。

## Requirements

### Requirement: 版本输出
系统 SHALL 在 `alter --version` 时打印 CLI 的名字与版本号。

#### Scenario: 打印版本
- WHEN 执行 `alter --version`
- THEN stdout 恰好为一行 `alter 0.1.0`
- AND 退出码为 0

### Requirement: 状态查询
系统 SHALL 在 `alter status` 时根据数据目录里的健康文件以及其中 pid 对应的进程是否存活来报告守护进程状态。

#### Scenario: 守护进程运行中
- WHEN `<ALTER_HOME>/alterd.json` 存在，其 `pid` 对应一个存活的进程（例如正在运行的 `alterd run`），`version` 为 `0.1.0`，执行 `alter status`
- THEN stdout 包含 `running`、该 pid 和 `0.1.0`
- AND 退出码为 0
- AND 改为执行 `alter status --json` 时，stdout 为一行 JSON `{"running": true, "pid": <该 pid>, "version": "0.1.0", "dbPath": "<ALTER_HOME>/alter.db"}`

#### Scenario: 没有健康文件
- WHEN `ALTER_HOME` 指向一个没有 `alterd.json` 的目录，执行 `alter status`
- THEN stdout 为一行 `not running`
- AND 退出码为 1
- AND 改为执行 `alter status --json` 时，stdout 为一行 JSON `{"running": false}`

#### Scenario: 健康文件残留
- WHEN `<ALTER_HOME>/alterd.json` 存在且其中 `pid` 为 999999（不存在的进程），执行 `alter status`
- THEN stdout 为一行 `not running (stale)`
- AND 退出码为 1
- AND 改为执行 `alter status --json` 时，stdout 为一行 JSON `{"running": false, "stale": true}`
