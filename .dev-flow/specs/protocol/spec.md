# protocol Specification

## Purpose
协议类型以 Rust 为源头，生成 TypeScript 声明供 SDK 与壳使用；禁止手改生成文件。

## Requirements

### Requirement: 协议类型导出
系统 SHALL 把 Rust 里标记为导出的协议类型生成为 TypeScript 声明文件写入指定目录。

#### Scenario: 导出错误码与健康文件类型
- WHEN 执行 `cargo run -p alter-core --bin export-ts -- <空的临时目录>`
- THEN 退出码为 0
- AND 目录中存在 `ErrorCode.ts`，其类型是字符串字面量联合，恰好包含 `E_UNAUTHORIZED`、`E_PROTOCOL_VERSION`、`E_SPACE_NOT_FOUND`、`E_TASK_SPACE_NOT_FOUND`、`E_PAGE_NOT_FOUND`、`E_PAGE_BUDGET`、`E_USER_CONTROL`、`E_AMBIGUOUS_SELECTOR`、`E_NO_MATCH`、`E_TIMEOUT`、`E_DIALOG_OPEN`、`E_IMPORT_SOURCE_NOT_FOUND`、`E_KEYCHAIN_DENIED`、`E_INTERNAL`、`E_TAB_NOT_FOUND`、`E_INVALID_ARGUMENT` 这 16 个字面量
- AND 目录中存在 `HealthFile.ts`，其字段恰好为 `version`、`pid`、`startedAt`、`dbPath`

#### Scenario: 导出 Shell Control 消息类型
- WHEN 执行 `cargo run -p alter-core --bin export-ts -- <空的临时目录>`
- THEN 目录中存在 `ShellCommand.ts`，其 `type` 字段的字符串字面量恰好为 `shell.stateChanged`、`shell.navigate`、`shell.error`、`shell.discard`、`shell.searchResults`、`shell.importSources`、`shell.importProgress`、`shell.importCookies`
- AND 目录中存在 `ShellEvent.ts`，其 `type` 字段的字符串字面量恰好为 `shell.event.openRequested`、`shell.event.tabUpdated`、`shell.event.tabActivated`、`shell.event.tabClosed`、`shell.event.spaceCreated`、`shell.event.spaceUpdated`、`shell.event.spaceActivated`、`shell.event.spaceDeleted`、`shell.event.tabPinned`、`shell.event.tabMoved`、`shell.event.cdpReady`、`shell.event.ownershipReturned`、`shell.event.userTookControl`、`shell.event.searchRequested`、`shell.event.splitRequested`、`shell.event.splitDissolved`、`shell.event.tabRestored`、`shell.event.importSourcesRequested`、`shell.event.importRequested`、`shell.event.cookiesImported`
- AND 目录中存在 `TabView.ts`，其字段包含 `pinned`、`taskSpaceId`、`label`、`splitGroup`
- AND 目录中存在 `ImportedCookie.ts`，其字段恰好为 `url`、`name`、`value`、`domain`、`path`、`secure`、`httpOnly`、`sameSite`、`expirationDate`

#### Scenario: 导出 Gateway 类型
- WHEN 执行 `cargo run -p alter-core --bin export-ts -- <空的临时目录>`
- THEN 目录中存在 `GatewayFile.ts`，其字段恰好为 `port`、`token`、`cdpPort`、`version`
- AND 目录中存在 `Receipt.ts`，其字段包含 `ok`、`durationMs`、`popups`
- AND 目录中存在 `OwnershipChanged.ts`，其字段恰好为 `id`、`ownership`、`reason`
- AND 目录中存在 `ImportCounts.ts`，其字段恰好为 `bookmarks`、`history`、`cookies`、`skipped`

### Requirement: 导出更新提示类型
系统 SHALL 把更新提示的消息体与它在 Shell 快照中的字段导出为 TypeScript 声明。

#### Scenario: 导出 UpdateAvailable
- WHEN 执行 `cargo run -p alter-core --bin export-ts -- <空的临时目录>`
- THEN 目录中存在 `UpdateAvailable.ts`，其字段恰好为 `version`、`url`
- AND 目录中存在 `ShellState.ts`，其字段包含 `updateAvailable`
