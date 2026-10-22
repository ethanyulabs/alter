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

### Requirement: 后退、前进与刷新
系统 SHALL 在侧边栏提供后退、前进、刷新三个按钮，作用于当前激活标签；没有可后退或可前进的历史时，对应按钮不可点击。

#### Scenario: 后退与前进
- WHEN 激活标签先打开本地测试页 `/a`，再在当前标签打开 `/b`
- THEN 前进按钮不可点击
- AND 点击后退后 2 秒内页面 URL 为 `/a` 的地址，前进按钮可点击
- AND 再点击前进后 2 秒内页面 URL 为 `/b` 的地址

#### Scenario: 刷新
- WHEN 激活标签显示本地测试页 `/count`（服务器记录它被请求的次数，当前为 1），用户点击刷新
- THEN 2 秒内服务器记录的 `/count` 请求次数为 2

### Requirement: stdout / stderr 断开时照常运行
系统 SHALL 在 app 主进程的 stdout / stderr 管道读端关闭后照常运行：主进程写日志或发出进程警告不产生未捕获异常，并继续读走 alterd 写到 stderr 的日志。

#### Scenario: 写日志不产生未捕获异常
- WHEN app 已启动且窗口显示 `alterd 运行中`，关闭 app 的 stdout 与 stderr 管道的读端，之后主进程执行 `console.log`、`console.error` 并发出一条进程警告（`process.emitWarning`）
- THEN 1 秒内主进程没有未捕获异常（`uncaughtExceptionMonitor` 没有记录）
- AND 主进程在 2 秒内响应一次 `app.evaluate`

#### Scenario: 继续读走 alterd 的 stderr
- WHEN `ALTER_ALTERD_PATH` 指向一个假 alterd，app 显示 `alterd 运行中` 后关闭 app 的 stderr 管道的读端，假 alterd 随后向 stderr 写出 256 KB
- THEN 从让它开始写起 3 秒内写完（它在写完后建出标记文件）

### Requirement: 快捷键
系统 SHALL 在应用菜单中提供下表的菜单项，按下快捷键或点击菜单项时执行对应操作：

| 菜单项 id | 快捷键 | 操作 |
|---|---|---|
| `tab.new` | `CmdOrCtrl+T` | 打开命令栏 |
| `tab.close` | `CmdOrCtrl+W` | 关闭当前 Space 的激活标签 |
| `address.focus` | `CmdOrCtrl+L` | 聚焦地址框并选中其中文字 |
| `nav.reload` | `CmdOrCtrl+R` | 刷新激活标签 |
| `nav.back` | `CmdOrCtrl+[` | 激活标签后退 |
| `nav.forward` | `CmdOrCtrl+]` | 激活标签前进 |
| `space.1` … `space.9` | `CmdOrCtrl+1` … `CmdOrCtrl+9` | 切到第 N 个 Space，不存在时什么也不做 |
| `tab.next` | `Ctrl+Tab` | 激活列表中的下一个标签，到末尾后回到第一个 |
| `tab.prev` | `Ctrl+Shift+Tab` | 激活列表中的上一个标签，到开头后回到最后一个 |
| `tab.togglePin` | `CmdOrCtrl+Shift+D` | 固定或取消固定激活标签 |

#### Scenario: 菜单项与快捷键
- WHEN app 启动后在主进程读取 `Menu.getApplicationMenu()`
- THEN 上表每个菜单项 id 都存在，且其 `accelerator` 与表中的快捷键字符串完全相同

#### Scenario: 关闭激活标签
- WHEN app 当前 Space 依次打开了本地测试页 `/a`、`/b`（`Page B` 激活），在主进程点击菜单项 `tab.close`
- THEN 1 秒内侧边栏标签列表恰好为 `Page A`，且它处于激活样式

#### Scenario: 按序号切换 Space
- WHEN app 有按顺序排列的 Space `Personal`、`Work`，当前为 `Work`，在主进程点击菜单项 `space.1`
- THEN 1 秒内底部的 Space 名称显示 `Personal`
- AND 再点击菜单项 `space.3` 后 1 秒，底部的 Space 名称仍为 `Personal`

#### Scenario: 循环切换标签
- WHEN app 当前 Space 依次打开了本地测试页 `/a`、`/b`、`/c`（`Page C` 激活），在主进程点击菜单项 `tab.next`
- THEN 1 秒内 `Page A` 处于激活样式
- AND 再点击菜单项 `tab.prev` 后 1 秒内 `Page C` 处于激活样式

#### Scenario: 新标签与聚焦地址框
- WHEN app 当前 Space 打开了本地测试页 `/a`，在主进程点击菜单项 `tab.new`，然后在键盘上输入 `127.0.0.1:<port>/b` 并回车
- THEN 2 秒内侧边栏依次为 `Page A`、`Page B`，`Page B` 处于激活样式
- AND 点击菜单项 `address.focus` 后，Renderer 的 `document.activeElement` 是地址框

#### Scenario: 快捷键固定激活标签
- WHEN app 当前 Space 依次打开了本地测试页 `/a`、`/b`（`Page B` 激活），在主进程点击菜单项 `tab.togglePin`
- THEN 1 秒内侧边栏依次为 `Page B`、`Page A`，`Page B` 在固定分区

#### Scenario: 快捷键刷新
- WHEN 激活标签显示本地测试页 `/count`（服务器记录的请求次数为 1），在主进程点击菜单项 `nav.reload`
- THEN 2 秒内服务器记录的 `/count` 请求次数为 2
