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

### Requirement: alter nodejs
系统 SHALL 在 `alter nodejs` 时用 Bun 运行 `-e` 参数或 stdin 中的脚本，预置全局函数 `taskSpace`、`takeOverTaskSpace`、`claimTaskSpace`、`listTaskSpaces`，退出码与脚本相同；找不到 `gateway.json` 时以退出码 2 结束。

#### Scenario: -e 运行脚本
- WHEN app 在运行，执行 `alter nodejs -e 'const t = await taskSpace("demo"); console.log(t.page("p1").label)'`
- THEN stdout 为一行 `p1`，退出码为 0

#### Scenario: 从 stdin 读脚本
- WHEN app 在运行，以 heredoc 向 `alter nodejs` 的 stdin 写入 `console.log((await listTaskSpaces()).length >= 0)`
- THEN stdout 为一行 `true`，退出码为 0

#### Scenario: 脚本抛出异常
- WHEN 执行 `alter nodejs -e 'throw new Error("boom")'`
- THEN 退出码为 1，stderr 包含 `boom`

#### Scenario: alterd 未运行
- WHEN `ALTER_HOME` 指向一个没有 `gateway.json` 的临时目录，执行 `alter nodejs -e '1'`
- THEN 退出码为 2，stderr 包含 `alter.app`

### Requirement: alter spaces list
系统 SHALL 在 `alter spaces list` 时经 Gateway 打印全部 Space，每行 `<id>  <name>  <tabCount> tabs`；带 `--json` 时打印 `space.list` 的结果 JSON。

#### Scenario: 文本与 JSON 输出
- WHEN alterd 只有 `Personal`（0 个标签），执行 `alter spaces list` 与 `alter spaces list --json`
- THEN 第一个命令的 stdout 恰好一行，包含 `Personal` 与 `0 tabs`，退出码为 0
- AND 第二个命令的 stdout 是一行 JSON，`spaces[0].name` 为 `Personal`

### Requirement: alter import
系统 SHALL 在 `alter import --from chrome|arc [--profile <path>] --space <id> [--items …]` 时经 Gateway 运行导入，等待结束后打印各类计数；来源不存在时退出码为 1。

#### Scenario: 导入成功
- WHEN alterd 在运行，执行 `alter import --from chrome --profile <标准 fixture profile> --space <Work 的 id> --items bookmarks,history`
- THEN stdout 为一行 `imported bookmarks=3 history=2 cookies=0 skipped=0`，退出码为 0

#### Scenario: 来源不存在
- WHEN 执行 `alter import --from chrome --profile /nonexistent --space <id>`
- THEN 退出码为 1，stderr 包含 `E_IMPORT_SOURCE_NOT_FOUND`
