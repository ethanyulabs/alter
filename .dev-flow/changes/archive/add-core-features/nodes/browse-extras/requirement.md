---
id: add-core-features
node: N4
title: 命令栏、分屏、Peek 与自动归档
repos: [alter]
revision: 1
approved_revision: 1
---

# N4 命令栏、分屏、Peek 与自动归档

<!-- 流程路线型需求中一个节点（子流程）的需求文档。保留字保持英文。语法见 references/formats.md §3 和 §14 -->

## 范围
- 交付：
  - 浏览历史的记录。
  - Cmd+T 命令栏：搜索标签、Space、历史，或打开网址 / 搜索。
  - 两个标签分屏。
  - Shift+点击链接的 Peek 预览。
  - 不活跃标签的自动归档、「已归档」列表与恢复。
  - 交给 N5：命令栏的搜索请求与结果契约（`shell.event.searchRequested` / `shell.searchResults`），N5 往结果里加「书签」；`history` 的写入规则。
- 依赖：N1 的固定标签（固定标签不参与归档）、`position` 规则、菜单注册表（`tab.new`）；N2 的快照字段 `taskSpaceId`（agent 托管的标签不参与归档）。
- 不做：书签的管理界面；三个及以上标签的分屏；Peek 的历史记录。

术语：「命令栏」是 Renderer 中居中的输入框加结果列表（`data-testid="command-bar"`）。结果项（`command-item`）有 `data-kind` 属性，取值为 `tab`、`space`、`history`、`open`。本地测试页 `/links` 的标题为 `Links`，其中有指向 `/b` 的链接 `Open B`；`/frame` 内嵌一个会改写自己 hash 的 iframe。

## Spec Delta: history
用户在各个 Space 里访问过的页面。

### ADDED Requirement: R1 记录浏览历史
系统 SHALL 在标签的主框架每次完成导航后，在该标签所属 Space 的历史里追加一条 `source` 为 `alter` 的记录，之后的标题回写更新这条记录的标题。

#### Scenario: R1.S1 导航与标题写入历史
- WHEN Space `Personal` 有标签 T，客户端依次发送 `tabUpdated {tabId:T, url:"https://example.com/a"}`、`tabUpdated {tabId:T, title:"A"}`、`tabUpdated {tabId:T, url:"https://example.com/b"}`
- THEN `ALTER_HOME/alter.db` 的 `history` 中 `space_id` 为 `Personal` 的 id 的行按 `visited_at` 依次为 `(https://example.com/a, A)`、`(https://example.com/b, A)`，`source` 都为 `alter`

#### Scenario: R1.S2 iframe 导航不写历史
- WHEN 激活标签显示本地测试页 `/frame`（内嵌 iframe 改写自己的 hash）
- THEN 2 秒后 `history` 中只有 `/frame` 的 1 条记录

## Spec Delta: command-bar
一个输入框完成打开网址、搜索、切换标签与 Space。

### ADDED Requirement: R2 打开与关闭命令栏
系统 SHALL 在 Cmd+T 时打开命令栏并聚焦输入框，此时内容区不显示页面；按 Esc 关闭命令栏，恢复显示原来的激活标签。

#### Scenario: R2.S1 打开与关闭
- WHEN app 激活标签显示本地测试页 `/a`，在主进程点击菜单项 `tab.new`
- THEN 1 秒内 `command-bar` 可见且 Renderer 的 `document.activeElement` 是它的输入框，窗口上挂载的 WebContentsView 数量为 0
- AND 按 Esc 后 1 秒内 `command-bar` 不可见，窗口上挂载的视图的 URL 为 `/a` 的地址

### ADDED Requirement: R3 搜索结果
系统 SHALL 对命令栏的输入，按「标签、Space、历史」的顺序返回标题或 URL 中包含输入（不区分大小写）的项，合计不超过 8 项，并在最后附加一项「打开或搜索该输入」。

#### Scenario: R3.S1 结果的来源与顺序
- WHEN Space `Personal` 有标签 `Docs Home`（`https://docs.example.com/`），存在名为 `Docs` 的 Space，`Personal` 的历史里有 `Docs Archive`（`https://docs.example.com/old`），客户端发送 `{"type":"shell.event.searchRequested","requestId":1,"query":"docs"}`
- THEN 客户端收到 `shell.searchResults`，`requestId` 为 1，`items` 依次为 `kind` 为 `tab`（`Docs Home`）、`space`（`Docs`）、`history`（`Docs Archive`）、`open`（`input` 为 `docs`）的 4 项

#### Scenario: R3.S2 已打开的 URL 不重复出现在历史结果里
- WHEN 标签 `Docs Home` 的 URL 也在历史里，客户端搜索 `docs home`
- THEN `items` 中 `kind` 为 `history` 的项没有 `https://docs.example.com/`

#### Scenario: R3.S3 最多 8 项
- WHEN 当前 Space 有 12 个标题含 `Page` 的标签，客户端搜索 `page`
- THEN `items` 共 9 项：前 8 项的 `kind` 为 `tab`，最后一项的 `kind` 为 `open`

#### Scenario: R3.S4 空输入
- WHEN 客户端搜索 `"  "`
- THEN `items` 为空数组

### ADDED Requirement: R4 执行结果项
系统 SHALL 在用户回车或点击某个结果项时执行它：标签项切到其 Space 并激活该标签，Space 项切到该 Space，历史项与「打开或搜索」项在当前 Space 新建标签打开。

#### Scenario: R4.S1 切到另一个 Space 的标签
- WHEN app 的 `Personal` 有标签 `Page A`，当前 Space 为 `Work`，用户按 Cmd+T（菜单项 `tab.new`），输入 `Page A`，回车
- THEN 1 秒内底部的 Space 名称为 `Personal`，`Page A` 处于激活样式，命令栏不可见

#### Scenario: R4.S2 输入网址打开新标签
- WHEN app 当前 Space 有 `Page A`，用户打开命令栏，输入 `127.0.0.1:<port>/b`，回车
- THEN 2 秒内侧边栏依次为 `Page A`、`Page B`，`Page B` 处于激活样式

#### Scenario: R4.S3 打开历史项
- WHEN app 当前 Space 访问过 `/c` 后关闭了它，用户打开命令栏，输入 `Page C`，用方向键选中 `data-kind="history"` 的项并回车
- THEN 2 秒内侧边栏出现 `Page C` 且处于激活样式

## Spec Delta: app-shell

### MODIFIED Requirement: 快捷键
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

#### Scenario: R5.S1 菜单项与快捷键
- WHEN app 启动后在主进程读取 `Menu.getApplicationMenu()`
- THEN 上表每个菜单项 id 都存在，且其 `accelerator` 与表中的快捷键字符串完全相同

#### Scenario: R5.S2 关闭激活标签
- WHEN app 当前 Space 依次打开了本地测试页 `/a`、`/b`（`Page B` 激活），在主进程点击菜单项 `tab.close`
- THEN 1 秒内侧边栏标签列表恰好为 `Page A`，且它处于激活样式

#### Scenario: R5.S3 按序号切换 Space
- WHEN app 有按顺序排列的 Space `Personal`、`Work`，当前为 `Work`，在主进程点击菜单项 `space.1`
- THEN 1 秒内底部的 Space 名称显示 `Personal`
- AND 再点击菜单项 `space.3` 后 1 秒，底部的 Space 名称仍为 `Personal`

#### Scenario: R5.S4 循环切换标签
- WHEN app 当前 Space 依次打开了本地测试页 `/a`、`/b`、`/c`（`Page C` 激活），在主进程点击菜单项 `tab.next`
- THEN 1 秒内 `Page A` 处于激活样式
- AND 再点击菜单项 `tab.prev` 后 1 秒内 `Page C` 处于激活样式

#### Scenario: R5.S5 新标签与聚焦地址框
- WHEN app 当前 Space 打开了本地测试页 `/a`，在主进程点击菜单项 `tab.new`，然后在键盘上输入 `127.0.0.1:<port>/b` 并回车
- THEN 2 秒内侧边栏依次为 `Page A`、`Page B`，`Page B` 处于激活样式
- AND 点击菜单项 `address.focus` 后，Renderer 的 `document.activeElement` 是地址框

#### Scenario: R5.S6 快捷键固定激活标签
- WHEN app 当前 Space 依次打开了本地测试页 `/a`、`/b`（`Page B` 激活），在主进程点击菜单项 `tab.togglePin`
- THEN 1 秒内侧边栏依次为 `Page B`、`Page A`，`Page B` 在固定分区

#### Scenario: R5.S7 快捷键刷新
- WHEN 激活标签显示本地测试页 `/count`（服务器记录的请求次数为 1），在主进程点击菜单项 `nav.reload`
- THEN 2 秒内服务器记录的 `/count` 请求次数为 2

## Spec Delta: tabs

### ADDED Requirement: R6 两个标签分屏
系统 SHALL 在用户把同一 Space 的两个标签组成分屏后，激活其中任一个时在内容区左右各半并排显示两者，左边是发起分屏时的激活标签。

#### Scenario: R6.S1 组成分屏
- WHEN Space 有标签 A（激活）与 B，客户端发送 `{"type":"shell.event.splitRequested","tabIds":["<A 的 id>","<B 的 id>"]}`
- THEN 新快照中 A 与 B 的 `splitGroup` 相同且不为 null，两者顺序不变

#### Scenario: R6.S2 跨 Space 被拒绝
- WHEN 标签 A 属于 `Personal`、B 属于 `Work`，客户端发送 `splitRequested`，`tabIds` 为 A 与 B
- THEN 客户端收到 `shell.error`，`code` 为 `E_INVALID_ARGUMENT`

#### Scenario: R6.S3 不是恰好两个标签被拒绝
- WHEN 客户端发送 `splitRequested`，`tabIds` 只含 A
- THEN 客户端收到 `shell.error`，`code` 为 `E_INVALID_ARGUMENT`

#### Scenario: R6.S4 在侧边栏分屏
- WHEN app 当前 Space 依次打开了本地测试页 `/a`、`/b`（`Page B` 激活），用户右键 `Page A` 那一行并点「与当前标签分屏」
- THEN 1 秒内窗口上挂载了 2 个 WebContentsView，左边的 URL 为 `/b` 的地址，右边的为 `/a` 的地址，两者宽度之差不超过 1 px
- AND 正常退出并以同一个 `ALTER_HOME` 重启后，仍是这两个视图并排显示

### ADDED Requirement: R7 解散分屏
系统 SHALL 在用户解散分屏或关闭分屏中的一个标签时，清除两者的分屏关系，内容区只显示一个标签。

#### Scenario: R7.S1 关闭其中一个
- WHEN 标签 A 与 B 处于同一分屏，客户端发送 `tabClosed`（B 的 id）
- THEN 新快照中 A 的 `splitGroup` 为 null

#### Scenario: R7.S2 显式解散
- WHEN 标签 A 与 B 处于同一分屏，客户端发送 `{"type":"shell.event.splitDissolved","tabId":"<A 的 id>"}`
- THEN 新快照中 A 与 B 的 `splitGroup` 都为 null

### ADDED Requirement: R8 自动归档不活跃的标签
系统 SHALL 把 `last_active_at` 早于当前时间减去阈值（默认 12 小时，可由 `ALTER_HOME/config.toml` 的 `archive_after_hours` 配置）的标签归档，固定标签、各 Space 的激活标签和被 TaskSpace 托管的标签除外。

#### Scenario: R8.S1 只归档符合条件的标签
- WHEN 环境变量 `ALTER_ARCHIVE_AFTER_MS` 为 300，Space 有普通标签 A、固定标签 P、被 TaskSpace 托管的 agent 标签 G 和激活标签 B，四者此后都不再被激活，等待 1.5 秒
- THEN 新快照中没有 A，P、G、B 仍在
- AND 数据库中 A 的 `tabs.archived_at` 不为 NULL

### ADDED Requirement: R9 已归档列表与恢复
系统 SHALL 在快照里给出每个 Space 最近归档的至多 50 个标签，并在用户恢复其中一个时把它放回该 Space 普通分区的末尾并激活。

#### Scenario: R9.S1 关闭的标签出现在已归档列表
- WHEN Space 依次有标签 A、B，客户端关闭 A
- THEN 新快照的 `archived` 中有一项 `id` 为 A 的 id、`spaceId` 为该 Space 的 id、`url` 为 A 的 URL

#### Scenario: R9.S2 恢复
- WHEN A 已归档，Space 只剩标签 B，客户端发送 `{"type":"shell.event.tabRestored","tabId":"<A 的 id>"}`
- THEN 新快照的标签依次为 B、A，A 的 `active` 为 `true`，`archived` 中不再有 A

#### Scenario: R9.S3 列表上限
- WHEN 一个 Space 依次关闭了 60 个标签
- THEN 快照的 `archived` 中该 Space 的项恰好 50 个，且是最后关闭的 50 个

#### Scenario: R9.S4 在侧边栏恢复
- WHEN app 当前 Space 打开 `/a` 与 `/b` 后关闭了 `Page A`，用户点击侧边栏的「已归档」，再点击其中的 `Page A`
- THEN 1 秒内标签列表依次为 `Page B`、`Page A`，`Page A` 处于激活样式

## Spec Delta: peek
不离开当前页面地预览链接。

### ADDED Requirement: R10 Shift+点击打开 Peek
系统 SHALL 在用户按住 Shift 点击页面中的链接时，在当前页面之上的浮层里加载该链接，不新建标签。

#### Scenario: R10.S1 打开 Peek
- WHEN 激活标签显示本地测试页 `/links`（其中有指向 `/b` 的链接 `Open B`），在该页面上按住 Shift 点击 `Open B`
- THEN 2 秒内主进程中有一个 URL 为 `/b` 的地址、`data-peek` 为真的视图挂在窗口上，其宽高分别为内容区的 80%（误差 1 px）
- AND 侧边栏标签列表仍然恰好 1 行

### ADDED Requirement: R11 关闭 Peek 或转为标签
系统 SHALL 在用户按 Esc 或点击浮层外的区域时关闭 Peek，在用户点「在新标签打开」时关闭 Peek 并在当前 Space 新建标签打开 Peek 当前的 URL。

#### Scenario: R11.S1 Esc 关闭
- WHEN Peek 正显示 `/b`，在 Renderer 按 Esc
- THEN 1 秒内窗口上不再挂载 Peek 视图，主进程中没有 URL 为 `/b` 的地址的 webContents

#### Scenario: R11.S2 转为标签
- WHEN Peek 正显示 `/b`，用户点击「在新标签打开」
- THEN 2 秒内侧边栏依次为 `Links`、`Page B`，`Page B` 处于激活样式，Peek 已关闭

## Spec Delta: protocol

### MODIFIED Requirement: 协议类型导出
系统 SHALL 把 Rust 里标记为导出的协议类型生成为 TypeScript 声明文件写入指定目录。

#### Scenario: R12.S1 导出错误码与健康文件类型
- WHEN 执行 `cargo run -p alter-core --bin export-ts -- <空的临时目录>`
- THEN 退出码为 0
- AND 目录中存在 `ErrorCode.ts`，其类型是字符串字面量联合，恰好包含 `E_UNAUTHORIZED`、`E_PROTOCOL_VERSION`、`E_SPACE_NOT_FOUND`、`E_TASK_SPACE_NOT_FOUND`、`E_PAGE_NOT_FOUND`、`E_PAGE_BUDGET`、`E_USER_CONTROL`、`E_AMBIGUOUS_SELECTOR`、`E_NO_MATCH`、`E_TIMEOUT`、`E_DIALOG_OPEN`、`E_IMPORT_SOURCE_NOT_FOUND`、`E_KEYCHAIN_DENIED`、`E_INTERNAL`、`E_TAB_NOT_FOUND`、`E_INVALID_ARGUMENT` 这 16 个字面量
- AND 目录中存在 `HealthFile.ts`，其字段恰好为 `version`、`pid`、`startedAt`、`dbPath`

#### Scenario: R12.S2 导出 Shell Control 消息类型
- WHEN 执行 `cargo run -p alter-core --bin export-ts -- <空的临时目录>`
- THEN 目录中存在 `ShellCommand.ts`，其 `type` 字段的字符串字面量恰好为 `shell.stateChanged`、`shell.navigate`、`shell.error`、`shell.discard`、`shell.searchResults`
- AND 目录中存在 `ShellEvent.ts`，其 `type` 字段的字符串字面量恰好为 `shell.event.openRequested`、`shell.event.tabUpdated`、`shell.event.tabActivated`、`shell.event.tabClosed`、`shell.event.spaceCreated`、`shell.event.spaceUpdated`、`shell.event.spaceActivated`、`shell.event.spaceDeleted`、`shell.event.tabPinned`、`shell.event.tabMoved`、`shell.event.cdpReady`、`shell.event.ownershipReturned`、`shell.event.userTookControl`、`shell.event.searchRequested`、`shell.event.splitRequested`、`shell.event.splitDissolved`、`shell.event.tabRestored`
- AND 目录中存在 `TabView.ts`，其字段包含 `pinned`、`taskSpaceId`、`label`、`splitGroup`

#### Scenario: R12.S3 导出 Gateway 类型
- WHEN 执行 `cargo run -p alter-core --bin export-ts -- <空的临时目录>`
- THEN 目录中存在 `GatewayFile.ts`，其字段恰好为 `port`、`token`、`cdpPort`、`version`
- AND 目录中存在 `Receipt.ts`，其字段包含 `ok`、`durationMs`、`popups`
- AND 目录中存在 `OwnershipChanged.ts`，其字段恰好为 `id`、`ownership`、`reason`

## 变更记录
- 2026-10-21: 创建。
