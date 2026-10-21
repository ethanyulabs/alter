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
