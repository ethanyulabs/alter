# app-shell Specification

## Purpose
Electron 壳：窗口、托管 alterd、给 Renderer 的 IPC。

## Requirements

### Requirement: 主窗口启动
系统 SHALL 在 app 启动后打开一个标题为 `alter` 的主窗口。

#### Scenario: 窗口打开
- WHEN 用 Playwright 的 `_electron.launch` 启动打包前的 app
- THEN 第一个窗口的标题为 `alter`
- AND 窗口页面的可见文本包含 `alter`

### Requirement: 守护进程托管
系统 SHALL 在 app 启动时拉起 alterd，并在 app 退出时关闭它。

#### Scenario: 启动拉起 alterd
- WHEN `ALTER_HOME` 指向一个空的临时目录，启动 app
- THEN 3 秒内出现 `<ALTER_HOME>/alterd.json`
- AND 文件中的 `pid` 对应的进程存活
- AND 窗口页面的可见文本包含 `alterd 运行中`

#### Scenario: 退出关闭 alterd
- WHEN app 已启动且 `<ALTER_HOME>/alterd.json` 已出现，关闭 app
- THEN 2 秒内该文件原先记录的 `pid` 不再存活
- AND `<ALTER_HOME>/alterd.json` 已被删除

#### Scenario: alterd 二进制缺失
- WHEN 环境变量 `ALTER_ALTERD_PATH` 为 `/nonexistent/alterd`，启动 app
- THEN 主窗口仍然打开
- AND 窗口页面的可见文本包含 `alterd 未找到`
- AND 启动后 5 秒时 app 进程仍在运行

### Requirement: app 被强制结束时 alterd 随之退出
系统 SHALL 在 app 主进程被强制结束（SIGKILL）后 2 秒内让它拉起的 alterd 退出，并删除 `ALTER_HOME/alterd.json`。

#### Scenario: SIGKILL app 后 alterd 退出
- WHEN 以空的临时目录为 `ALTER_HOME` 启动 app，窗口显示「alterd 运行中」，记下 `alterd.json` 中的 pid（例如 41237），然后对 app 主进程发送 SIGKILL
- THEN 2 秒内 pid 41237 不再存活
- AND `ALTER_HOME/alterd.json` 不存在

### Requirement: 复用已在运行的 alterd
系统 SHALL 在同一 `ALTER_HOME` 已有一个存活的 alterd 时复用它：页面状态显示「alterd 运行中」与该实例的 pid；app 退出时不终止这个实例。

#### Scenario: 已有实例时复用而不报已退出
- WHEN 以空的临时目录为 `ALTER_HOME`，先在 app 之外运行 `alterd run`（pid 例如 52001），等 `alterd.json` 出现后启动 app
- THEN 3 秒内页面文本包含「alterd 运行中」，`daemon.status` 的 pid 为 52001
- AND 正常退出 app 后 2 秒，pid 52001 仍存活，`ALTER_HOME/alterd.json` 仍存在且 pid 为 52001
