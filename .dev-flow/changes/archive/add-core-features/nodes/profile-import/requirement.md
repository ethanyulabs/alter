---
id: add-core-features
node: N5
title: 从 Chrome / Arc 导入
repos: [alter]
revision: 2
approved_revision: 2
---

# N5 从 Chrome / Arc 导入

<!-- 流程路线型需求中一个节点（子流程）的需求文档。保留字保持英文。语法见 references/formats.md §3 和 §14 -->

## 范围
- 交付：
  - alterd 的 ImportService：读取 Chrome / Arc profile 的书签、最近 90 天历史，写进目标 Space。
  - Gateway 方法 `import.run` / `import.status` 与进度事件；CLI `alter import`。
  - app 内的导入向导。
  - 命令栏的书签结果。
- 依赖：
  - N2：Gateway 分发器与通知推送。
  - N4：命令栏搜索契约（`searchRequested` / `searchResults`）。
  - N1：Space。
- 不做：Arc 侧边栏专有数据（`StorableSidebar.json`）；密码、扩展、表单数据；Safari / Firefox；Cookie 的导入（Keychain 取口令、解密、写进 partition、拒绝授权时部分完成、损坏 Cookie 的跳过；revision 2 起移到后续 change）。

术语：
- 「fixture profile」是测试在临时目录里生成的 Chromium 格式 profile 目录，含 `Bookmarks`（JSON）、`History`（SQLite，`urls` 与 `visits` 表）。
- 「标准 fixture profile」的内容固定为：
  - 书签栏下有链接 `Rust`（`https://rust-lang.org/`）和文件夹 `Dev`（其中有链接 `MDN`），共 3 个书签项。
  - 历史里 `https://a.example/` 在 10 天前有 2 次访问，`https://b.example/` 在 100 天前有 1 次访问。
- 「Chrome 数据根目录」默认为 `~/Library/Application Support/Google/Chrome`，「Arc 数据根目录」默认为 `~/Library/Application Support/Arc/User Data`；测试用 `ALTER_CHROME_DIR` / `ALTER_ARC_DIR` 覆盖。

## Spec Delta: import
把其他浏览器的数据搬进某个 Space。

### ADDED Requirement: R1 导入书签
系统 SHALL 把源 profile `Bookmarks` 文件里书签栏与其他书签下的全部文件夹和链接，按原有层级与顺序写入目标 Space 的书签，`source` 记为来源浏览器。

#### Scenario: R1.S1 导入书签树
- WHEN fixture profile 的书签栏下有链接 `Rust`（`https://rust-lang.org/`）和文件夹 `Dev`（其中有链接 `MDN`），客户端调用 `import.run {"source":"chrome","profilePath":"<fixture>","spaceId":"<Work 的 id>","items":["bookmarks"]}`，等到 `import.done`
- THEN `import.done` 的 `counts.bookmarks` 为 3
- AND `ALTER_HOME/alter.db` 的 `bookmarks` 中 `space_id` 为 `Work` 的 id 的行有：`Dev` 文件夹（`url` 为 NULL），`parent_id` 指向它的 `MDN`，以及 `Rust`，`source` 都为 `chrome`

#### Scenario: R1.S2 重复导入不产生重复
- WHEN 已从标准 fixture profile 向 Space `Work` 导入过书签，对同一个 profile 和 Space 再运行一次 `import.run`（`items` 为 `["bookmarks"]`）
- THEN `bookmarks` 中 `Work` 的行数仍为 3

### ADDED Requirement: R2 导入最近 90 天的历史
系统 SHALL 只导入源 profile 中访问时间在最近 90 天内的历史记录，按（URL、访问时间）去重。

#### Scenario: R2.S1 跳过 90 天以前的访问
- WHEN fixture profile 的 `History` 中，`https://a.example/` 在 10 天前有 2 次访问，`https://b.example/` 在 100 天前有 1 次访问，客户端导入 `items` 为 `["history"]`
- THEN `import.done` 的 `counts.history` 为 2
- AND `history` 中该 Space 有 2 行 `https://a.example/`，`source` 为 `chrome`，`visited_at` 与 fixture 的访问时间（换算为 UTC 毫秒）相等，没有 `https://b.example/`

### ADDED Requirement: R5 来源不存在
系统 SHALL 在源 profile 目录不存在或其中三个数据文件都不存在时拒绝导入，并返回 `E_IMPORT_SOURCE_NOT_FOUND`。

#### Scenario: R5.S1 目录不存在
- WHEN 客户端调用 `import.run`，`profilePath` 为 `/nonexistent/profile`
- THEN `error.data.code` 为 `E_IMPORT_SOURCE_NOT_FOUND`，`imports` 表没有新行

### ADDED Requirement: R6 默认 profile 与 Arc
系统 SHALL 在省略 `profilePath` 时使用来源浏览器数据根目录下的 `Default` profile。

#### Scenario: R6.S1 Arc 默认 profile
- WHEN `ALTER_ARC_DIR` 指向一个临时目录，其 `Default` 子目录是标准 fixture profile，客户端调用 `import.run {"source":"arc","spaceId":"<id>","items":["bookmarks"]}`
- THEN `import.done` 的 `counts.bookmarks` 为 3，`bookmarks` 中这些行的 `source` 为 `arc`

#### Scenario: R6.S2 列出可导入的 profile
- WHEN `ALTER_CHROME_DIR` 下有 `Default` 与 `Profile 1` 两个含 `Bookmarks` 的目录，Electron 发送 `{"type":"shell.event.importSourcesRequested","requestId":5}`
- THEN Electron 收到 `shell.importSources`，`requestId` 为 5，`chrome` 为 `["Default","Profile 1"]`，`arc` 为空数组

### ADDED Requirement: R8 进度事件
系统 SHALL 在导入每完成一类数据时向发起导入的连接推送 `import.progress`，全部结束时推送 `import.done`，并让 `import.status` 返回同样的状态与计数；省略 `items` 时导入书签与历史。

#### Scenario: R8.S1 省略 items 时两类数据的进度
- WHEN 客户端从标准 fixture profile 调用 `import.run`，省略 `items`
- THEN 客户端依次收到 2 条 `import.progress`（`item` 依次为 `bookmarks`、`history`）和 1 条 `import.done`，其 `status` 为 `done`，`counts.bookmarks` 为 3、`counts.history` 为 2
- AND 之后调用 `import.status {"importId":<id>}` 返回的 `status` 与 `counts` 与 `import.done` 相同

## Spec Delta: cli

### ADDED Requirement: R9 alter import
系统 SHALL 在 `alter import --from chrome|arc [--profile <path>] --space <id> [--items …]` 时经 Gateway 运行导入，等待结束后打印各类计数；来源不存在时退出码为 1。

#### Scenario: R9.S1 导入成功
- WHEN alterd 在运行，执行 `alter import --from chrome --profile <标准 fixture profile> --space <Work 的 id> --items bookmarks,history`
- THEN stdout 为一行 `imported bookmarks=3 history=2 cookies=0 skipped=0`，退出码为 0

#### Scenario: R9.S2 来源不存在
- WHEN 执行 `alter import --from chrome --profile /nonexistent --space <id>`
- THEN 退出码为 1，stderr 包含 `E_IMPORT_SOURCE_NOT_FOUND`

## Spec Delta: import-wizard
app 内的导入入口。

### ADDED Requirement: R10 导入向导
系统 SHALL 在侧边栏菜单的「导入…」中提供向导，让用户选择来源浏览器、profile、目标 Space 和数据类别，开始后显示进度与各类计数。

#### Scenario: R10.S1 从向导导入
- WHEN `ALTER_CHROME_DIR` 下的 `Default` 是标准 fixture profile，app 有 Space `Personal` 与 `Work`，用户打开「导入…」，选 `Chrome`、`Default`、`Work`，勾选书签与历史，点「开始导入」
- THEN 5 秒内向导显示 `书签 3`、`历史 2`，并出现「完成」按钮
- AND `bookmarks` 中 `Work` 有 3 行

## Spec Delta: command-bar

### MODIFIED Requirement: 搜索结果
系统 SHALL 对命令栏的输入，按「标签、Space、书签、历史」的顺序返回标题或 URL 中包含输入（不区分大小写）的项，合计不超过 8 项，并在最后附加一项「打开或搜索该输入」。

#### Scenario: R11.S1 结果的来源与顺序
- WHEN Space `Personal` 有标签 `Docs Home`（`https://docs.example.com/`），存在名为 `Docs` 的 Space，`Personal` 的历史里有 `Docs Archive`（`https://docs.example.com/old`），客户端发送 `{"type":"shell.event.searchRequested","requestId":1,"query":"docs"}`
- THEN 客户端收到 `shell.searchResults`，`requestId` 为 1，`items` 依次为 `kind` 为 `tab`（`Docs Home`）、`space`（`Docs`）、`history`（`Docs Archive`）、`open`（`input` 为 `docs`）的 4 项

#### Scenario: R11.S2 已打开的 URL 不重复出现在历史结果里
- WHEN 标签 `Docs Home` 的 URL 也在历史里，客户端搜索 `docs home`
- THEN `items` 中 `kind` 为 `history` 的项没有 `https://docs.example.com/`

#### Scenario: R11.S3 最多 8 项
- WHEN 当前 Space 有 12 个标题含 `Page` 的标签，客户端搜索 `page`
- THEN `items` 共 9 项：前 8 项的 `kind` 为 `tab`，最后一项的 `kind` 为 `open`

#### Scenario: R11.S4 空输入
- WHEN 客户端搜索 `"  "`
- THEN `items` 为空数组

#### Scenario: R11.S5 书签排在历史之前
- WHEN 当前 Space 的书签里有 `Rust`（`https://rust-lang.org/`），历史里有 `Rust Blog`（`https://blog.rust-lang.org/`），客户端搜索 `rust`
- THEN `items` 依次为 `kind` 为 `bookmark`（`Rust`）、`history`（`Rust Blog`）、`open` 的 3 项

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
- THEN 目录中存在 `ShellCommand.ts`，其 `type` 字段的字符串字面量恰好为 `shell.stateChanged`、`shell.navigate`、`shell.error`、`shell.discard`、`shell.searchResults`、`shell.importSources`、`shell.importProgress`
- AND 目录中存在 `ShellEvent.ts`，其 `type` 字段的字符串字面量恰好为 `shell.event.openRequested`、`shell.event.tabUpdated`、`shell.event.tabActivated`、`shell.event.tabClosed`、`shell.event.spaceCreated`、`shell.event.spaceUpdated`、`shell.event.spaceActivated`、`shell.event.spaceDeleted`、`shell.event.tabPinned`、`shell.event.tabMoved`、`shell.event.cdpReady`、`shell.event.ownershipReturned`、`shell.event.userTookControl`、`shell.event.searchRequested`、`shell.event.splitRequested`、`shell.event.splitDissolved`、`shell.event.tabRestored`、`shell.event.importSourcesRequested`、`shell.event.importRequested`
- AND 目录中存在 `TabView.ts`，其字段包含 `pinned`、`taskSpaceId`、`label`、`splitGroup`

#### Scenario: R12.S3 导出 Gateway 类型
- WHEN 执行 `cargo run -p alter-core --bin export-ts -- <空的临时目录>`
- THEN 目录中存在 `GatewayFile.ts`，其字段恰好为 `port`、`token`、`cdpPort`、`version`
- AND 目录中存在 `Receipt.ts`，其字段包含 `ok`、`durationMs`、`popups`
- AND 目录中存在 `OwnershipChanged.ts`，其字段恰好为 `id`、`ownership`、`reason`
- AND 目录中存在 `ImportCounts.ts`，其字段恰好为 `bookmarks`、`history`、`cookies`、`skipped`

## 变更记录
- 2026-10-21: 创建。
- 2026-10-21: amended: 收窄范围——Cookie 导入（R3、R4、R7 与 R9.S3）移到后续 change；R6 去掉 Arc Safe Storage 口令；R8 改为省略 `items` 时导入书签与历史，R8.S1 改为两类数据的进度；任务 T5 移除（Cookie，暂缓未开始），新增 T8 覆盖 R8.S1; revision: 2; re-approved by the user
