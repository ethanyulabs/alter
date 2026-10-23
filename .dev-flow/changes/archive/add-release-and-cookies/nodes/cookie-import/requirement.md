---
id: add-release-and-cookies
node: N1
title: 从 Chrome / Arc 导入 Cookie
repos: [alter]
revision: 1
approved_revision: 1
---

# N1 从 Chrome / Arc 导入 Cookie

<!-- 流程路线型需求中一个节点（子流程）的需求文档。这里写本节点的 Spec Delta；整体（背景、范围、非功能需求）在父级 requirement.md，节点顺序在 flow.md。保留字保持英文："Spec Delta:"、"ADDED/MODIFIED/REMOVED/RENAMED Requirement:"、"Scenario:"、WHEN/THEN/AND、SHALL/MUST。语法见 references/formats.md §3 和 §14 -->

## 范围
- 交付：
  - `import.run` / `alter import` / 导入向导支持 `cookies` 这一类，这一类之前一直以 `E_INVALID_ARGUMENT` 拒绝。
  - 先从 macOS 钥匙串取「Chrome Safe Storage」或「Arc Safe Storage」口令，再解密源 profile `Cookies` 库里的 Cookie，最后经新的 Shell 指令 `shell.importCookies`，由 Electron Main 写进目标 Space 的 partition（`persist:space-<id>`）。
  - 钥匙串拒绝授权、没有 Shell 连接时部分完成；无法解密的 Cookie 跳过并计数；明文不写日志、不落盘。
- 交给下游：Shell 协议里「alterd 下发指令、Main 回报结果」这类带 `requestId` 的往返方式；Renderer 导入向导的「部分完成」原因文案。N2 的更新提示条沿用同一套文案样式。
- 依赖：无。只用已有的 ImportService（`crates/alterd/src/import/`）、Shell Control 通道、`imports` 表（0001 已有 `cookies_count` 列与 `partial` 状态）和 `ImportCounts.cookies` 字段。
- 不做：
  - 密码、表单、扩展的导入。
  - Linux 与 Windows 的 Cookie 格式（v11 与 DPAPI）。
  - 导入后的持续同步。
  - Safari / Firefox。
  - 省略 `items` 时的默认值不变：仍是书签与历史，Cookie 必须显式选择（见父级假设）。

术语：
- **Safe Storage 口令**：Chromium 系浏览器存在钥匙串里的通用密码，服务名为「Chrome Safe Storage」或「Arc Safe Storage」，用来派生 Cookie 的加密密钥。
- **v10 密文**：`encrypted_value` 以字节 `v10` 开头的 Cookie 值。
- **标准 Cookie fixture**：测试在 profile 目录里构造的 `Cookies` 库，`meta.version` 为 24，口令为 `peanuts`，有 3 行：
  - `sid=abc`：`host_key` 为 `127.0.0.1`，path 为 `/`，30 天后过期，httpOnly。
  - `theme=dark`：`host_key` 为 `.example.com`，会话 Cookie（`has_expires` 为 0）。
  - `old=1`：`host_key` 为 `127.0.0.1`，1 天前已过期。
- **钥匙串替身**：仅 debug 构建读取的环境变量 `ALTER_KEYCHAIN_FAKE`。值为口令时直接当作取到的口令，值为 `denied` 时当作用户拒绝授权。release 构建忽略它，一定走真实钥匙串。

## Spec Delta: import

### ADDED Requirement: R1 导入 Cookie
系统 SHALL 在 `items` 含 `cookies` 时，用来源浏览器的 Safe Storage 口令解密源 profile `Cookies` 库中未过期的 Cookie，经 Shell 指令 `shell.importCookies` 交给最近一次上报 `shell.event.cdpReady` 的 Shell 连接写入目标 Space，并把该连接回报写入成功的条数计入 `counts.cookies`。

#### Scenario: R1.S1 导入未过期的 Cookie
- WHEN `ALTER_KEYCHAIN_FAKE` 为 `peanuts`，fixture profile 只有标准 Cookie fixture，一个已发送 `shell.event.cdpReady` 的假 Electron 端已连接，客户端调用 `import.run {"source":"chrome","profilePath":"<fixture>","spaceId":"<Work 的 id>","items":["cookies"]}`
- THEN 假 Electron 端收到一条 `shell.importCookies`，`spaceId` 为 `Work` 的 id，`cookies` 恰好 2 项：`sid`（`value` 为 `abc`，`domain` 为 `127.0.0.1`，`httpOnly` 为 true，带 `expirationDate`）与 `theme`（`value` 为 `dark`，`domain` 为 `.example.com`，没有 `expirationDate`）
- AND 假 Electron 端回 `{"type":"shell.event.cookiesImported","requestId":<同一 requestId>,"imported":2,"rejected":0}` 后，`import.done` 的 `status` 为 `done`，`counts.cookies` 为 2，`counts.skipped` 为 0
- AND `imports` 表该行的 `cookies_count` 为 2

#### Scenario: R1.S2 Electron 拒绝写入的 Cookie 计入跳过
- WHEN 条件与上一场景相同，但假 Electron 端回 `"imported":1,"rejected":1`
- THEN `import.done` 的 `status` 为 `done`，`counts.cookies` 为 1，`counts.skipped` 为 1

#### Scenario: R1.S3 Arc 用 Arc Safe Storage
- WHEN `ALTER_ARC_DIR` 的 `Default` 子目录只有标准 Cookie fixture，`ALTER_KEYCHAIN_FAKE` 为 `peanuts`，客户端调用 `import.run {"source":"arc","spaceId":"<id>","items":["cookies"]}`，假 Electron 端如实回报
- THEN `import.done` 的 `counts.cookies` 为 2
- AND alterd 的 JSON 日志里有一行 `取 Safe Storage 口令`，其 `service` 为 `Arc Safe Storage`

#### Scenario: R1.S4 新版库的主机名前缀
- WHEN fixture 的 `Cookies` 库 `meta.version` 为 24，`sid` 的明文前 32 字节是 `SHA256("127.0.0.1")`、其后才是 `abc`，另一行 `bad` 的前 32 字节是 `SHA256("other.example")`、而 `host_key` 为 `127.0.0.1`，客户端导入 `cookies`
- THEN `shell.importCookies` 里 `sid` 的 `value` 恰好为 `abc`
- AND 没有 `bad` 这一项，`import.done` 的 `counts.skipped` 至少为 1

### ADDED Requirement: R2 钥匙串拒绝授权时部分完成
系统 SHALL 在取不到 Safe Storage 口令（用户拒绝授权或钥匙串里没有这一项）时不导入任何 Cookie，其余所选类别照常导入，导入以 `status` 为 `partial`、`warning` 为 `keychain_denied` 结束。

#### Scenario: R2.S1 拒绝授权
- WHEN `ALTER_KEYCHAIN_FAKE` 为 `denied`，fixture profile 有标准书签、历史与标准 Cookie fixture，客户端调用 `import.run`，`items` 为 `["bookmarks","history","cookies"]`
- THEN `import.done` 的 `status` 为 `partial`，`warning` 为 `keychain_denied`，`counts` 为 `{"bookmarks":3,"history":2,"cookies":0,"skipped":0}`
- AND 假 Electron 端没有收到 `shell.importCookies`
- AND 之后 `import.status` 返回同样的 `status` 与 `warning`

### ADDED Requirement: R3 跳过无法解密的 Cookie
系统 SHALL 跳过无法解密、填充不合法或解密后不是 UTF-8 的 Cookie，计入 `counts.skipped`，并继续导入其余 Cookie。

#### Scenario: R3.S1 一行密文被截断
- WHEN 标准 Cookie fixture 之外再加一行 `broken`，其 `encrypted_value` 为 `v10` 加 7 个字节，`ALTER_KEYCHAIN_FAKE` 为 `peanuts`，客户端导入 `cookies`，假 Electron 端如实回报
- THEN `import.done` 的 `status` 为 `done`，`counts.cookies` 为 2，`counts.skipped` 为 1

#### Scenario: R3.S2 明文值不加密
- WHEN `Cookies` 库只有一行 `plain`：`value` 为 `p1`，`encrypted_value` 为空，`ALTER_KEYCHAIN_FAKE` 为 `peanuts`，客户端导入 `cookies`
- THEN `shell.importCookies` 里 `plain` 的 `value` 为 `p1`，`counts.cookies` 为 1

### ADDED Requirement: R4 没有 Shell 连接时部分完成
系统 SHALL 在需要写入 Cookie 而没有任何上报过 `shell.event.cdpReady` 的 Shell 连接，或该连接 10 秒内没有回报 `shell.event.cookiesImported` 时，把这一批 Cookie 记为未导入，导入以 `status` 为 `partial`、`warning` 为 `shell_unavailable` 结束。

#### Scenario: R4.S1 alterd 独立运行
- WHEN alterd 以 `alterd run` 独立运行，没有任何 Shell 连接，`ALTER_KEYCHAIN_FAKE` 为 `peanuts`，客户端调用 `import.run`，`items` 为 `["bookmarks","cookies"]`
- THEN `import.done` 的 `status` 为 `partial`，`warning` 为 `shell_unavailable`，`counts.bookmarks` 为 3，`counts.cookies` 为 0

#### Scenario: R4.S2 Electron 不回报
- WHEN 已上报 `cdpReady` 的假 Electron 端收到 `shell.importCookies` 后不回复，`ALTER_IMPORT_COOKIE_TIMEOUT_MS` 为 500
- THEN 2 秒内收到 `import.done`，`status` 为 `partial`，`warning` 为 `shell_unavailable`，`counts.cookies` 为 0

### ADDED Requirement: R5 Cookie 明文不落盘、不写日志
系统 SHALL 只在内存中持有 Safe Storage 口令、派生密钥与解密后的 Cookie 值，导入结束后 `ALTER_HOME` 下的任何文件与 alterd 的日志输出都不包含它们。

#### Scenario: R5.S1 导入后找不到明文
- WHEN 标准 Cookie fixture 的 `sid` 值改为 `s3cr3t-value-7f2c`，`ALTER_KEYCHAIN_FAKE` 为 `peanuts`，客户端导入 `cookies` 并等到 `import.done`，然后停掉 alterd
- THEN alterd 的 stderr 全文不包含 `s3cr3t-value-7f2c` 也不包含 `peanuts`
- AND 递归读取 `ALTER_HOME` 下的每个文件，没有一个包含字节串 `s3cr3t-value-7f2c`
- AND `ALTER_HOME/import-tmp` 不存在或为空

## Spec Delta: shell-control

### ADDED Requirement: R6 把 Cookie 写进 Space 的 partition
系统 SHALL 在 Electron Main 收到 `shell.importCookies` 时，把其中每个 Cookie 写进 partition `persist:space-<spaceId>`，再回一条带同一 `requestId` 的 `shell.event.cookiesImported`，其中 `imported` 为写入成功数、`rejected` 为被拒绝数。

#### Scenario: R6.S1 只有目标 Space 带上导入的 Cookie
- WHEN app 有 Space `Personal` 与 `Work`，`ALTER_CHROME_DIR` 的 `Default` 有标准 Cookie fixture，alterd 带 `ALTER_KEYCHAIN_FAKE=peanuts` 运行，从导入向导把 Chrome `Default` 的 Cookie 导入 `Work`
- THEN 在 `Work` 里打开本地测试服务器的 `http://127.0.0.1:<port>/echo-cookie`，页面显示的请求头 `Cookie` 包含 `sid=abc`
- AND 在 `Personal` 里打开同一地址，页面显示的 `Cookie` 不包含 `sid=abc`

## Spec Delta: import-wizard

### MODIFIED Requirement: 导入向导
系统 SHALL 在侧边栏菜单的「导入…」中提供向导，让用户选择来源浏览器、profile、目标 Space 和数据类别（书签、历史、Cookie，Cookie 默认勾选），开始后显示进度与各类计数；部分完成时显示原因。

#### Scenario: R7.S1 从向导导入
- WHEN `ALTER_CHROME_DIR` 下的 `Default` 是标准 fixture profile，app 有 Space `Personal` 与 `Work`，用户打开「导入…」，选 `Chrome`、`Default`、`Work`，勾选书签与历史、取消勾选 Cookie，点「开始导入」
- THEN 5 秒内向导显示 `书签 3`、`历史 2`，并出现「完成」按钮
- AND `bookmarks` 中 `Work` 有 3 行

#### Scenario: R7.S2 Cookie 可选且默认勾选
- WHEN 用户打开「导入…」
- THEN 「Cookie」复选框可用且已勾选，旁边有说明文字「macOS 会请求钥匙串授权」

#### Scenario: R7.S3 拒绝授权时显示原因
- WHEN alterd 带 `ALTER_KEYCHAIN_FAKE=denied` 运行，`Default` 有标准书签、历史与标准 Cookie fixture，用户勾选三类并开始导入
- THEN 5 秒内向导显示「部分完成」、`书签 3`、`Cookie 0`，以及「未获得钥匙串授权，Cookie 未导入」

## Spec Delta: cli

### MODIFIED Requirement: alter import
系统 SHALL 在 `alter import --from chrome|arc [--profile <path>] --space <id> [--items …]` 时经 Gateway 运行导入，等待结束后打印各类计数；部分完成时退出码为 0 并在 stderr 打印 `warning: <原因>`；来源不存在时退出码为 1。

#### Scenario: R8.S1 导入成功
- WHEN alterd 在运行，执行 `alter import --from chrome --profile <标准 fixture profile> --space <Work 的 id> --items bookmarks,history`
- THEN stdout 为一行 `imported bookmarks=3 history=2 cookies=0 skipped=0`，退出码为 0

#### Scenario: R8.S2 来源不存在
- WHEN 执行 `alter import --from chrome --profile /nonexistent --space <id>`
- THEN 退出码为 1，stderr 包含 `E_IMPORT_SOURCE_NOT_FOUND`

#### Scenario: R8.S3 钥匙串拒绝授权
- WHEN alterd 带 `ALTER_KEYCHAIN_FAKE=denied` 运行，执行 `alter import --from chrome --profile <有标准 Cookie fixture 的 profile> --space <id> --items cookies`
- THEN stdout 为一行 `imported bookmarks=0 history=0 cookies=0 skipped=0`，stderr 有一行 `warning: keychain_denied`，退出码为 0

#### Scenario: R8.S4 帮助里不再说不支持 Cookie
- WHEN 执行 `alter import --help`
- THEN stdout 包含 `cookies`，且不包含 `暂不支持`

## Spec Delta: protocol

### MODIFIED Requirement: 协议类型导出
系统 SHALL 把 Rust 里标记为导出的协议类型生成为 TypeScript 声明文件写入指定目录。

#### Scenario: R9.S1 导出错误码与健康文件类型
- WHEN 执行 `cargo run -p alter-core --bin export-ts -- <空的临时目录>`
- THEN 退出码为 0
- AND 目录中存在 `ErrorCode.ts`，其类型是字符串字面量联合，恰好包含 `E_UNAUTHORIZED`、`E_PROTOCOL_VERSION`、`E_SPACE_NOT_FOUND`、`E_TASK_SPACE_NOT_FOUND`、`E_PAGE_NOT_FOUND`、`E_PAGE_BUDGET`、`E_USER_CONTROL`、`E_AMBIGUOUS_SELECTOR`、`E_NO_MATCH`、`E_TIMEOUT`、`E_DIALOG_OPEN`、`E_IMPORT_SOURCE_NOT_FOUND`、`E_KEYCHAIN_DENIED`、`E_INTERNAL`、`E_TAB_NOT_FOUND`、`E_INVALID_ARGUMENT` 这 16 个字面量
- AND 目录中存在 `HealthFile.ts`，其字段恰好为 `version`、`pid`、`startedAt`、`dbPath`

#### Scenario: R9.S2 导出 Shell Control 消息类型
- WHEN 执行 `cargo run -p alter-core --bin export-ts -- <空的临时目录>`
- THEN 目录中存在 `ShellCommand.ts`，其 `type` 字段的字符串字面量恰好为 `shell.stateChanged`、`shell.navigate`、`shell.error`、`shell.discard`、`shell.searchResults`、`shell.importSources`、`shell.importProgress`、`shell.importCookies`
- AND 目录中存在 `ShellEvent.ts`，其 `type` 字段的字符串字面量恰好为 `shell.event.openRequested`、`shell.event.tabUpdated`、`shell.event.tabActivated`、`shell.event.tabClosed`、`shell.event.spaceCreated`、`shell.event.spaceUpdated`、`shell.event.spaceActivated`、`shell.event.spaceDeleted`、`shell.event.tabPinned`、`shell.event.tabMoved`、`shell.event.cdpReady`、`shell.event.ownershipReturned`、`shell.event.userTookControl`、`shell.event.searchRequested`、`shell.event.splitRequested`、`shell.event.splitDissolved`、`shell.event.tabRestored`、`shell.event.importSourcesRequested`、`shell.event.importRequested`、`shell.event.cookiesImported`
- AND 目录中存在 `TabView.ts`，其字段包含 `pinned`、`taskSpaceId`、`label`、`splitGroup`
- AND 目录中存在 `ImportedCookie.ts`，其字段恰好为 `url`、`name`、`value`、`domain`、`path`、`secure`、`httpOnly`、`sameSite`、`expirationDate`

#### Scenario: R9.S3 导出 Gateway 类型
- WHEN 执行 `cargo run -p alter-core --bin export-ts -- <空的临时目录>`
- THEN 目录中存在 `GatewayFile.ts`，其字段恰好为 `port`、`token`、`cdpPort`、`version`
- AND 目录中存在 `Receipt.ts`，其字段包含 `ok`、`durationMs`、`popups`
- AND 目录中存在 `OwnershipChanged.ts`，其字段恰好为 `id`、`ownership`、`reason`
- AND 目录中存在 `ImportCounts.ts`，其字段恰好为 `bookmarks`、`history`、`cookies`、`skipped`

## 变更记录
- 2026-10-22: 创建。
