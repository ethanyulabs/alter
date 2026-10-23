# import Specification

## Purpose
把其他浏览器的数据搬进某个 Space。

## Requirements

### Requirement: 导入书签
系统 SHALL 把源 profile `Bookmarks` 文件里书签栏与其他书签下的全部文件夹和链接，按原有层级与顺序写入目标 Space 的书签，`source` 记为来源浏览器。

#### Scenario: 导入书签树
- WHEN fixture profile 的书签栏下有链接 `Rust`（`https://rust-lang.org/`）和文件夹 `Dev`（其中有链接 `MDN`），客户端调用 `import.run {"source":"chrome","profilePath":"<fixture>","spaceId":"<Work 的 id>","items":["bookmarks"]}`，等到 `import.done`
- THEN `import.done` 的 `counts.bookmarks` 为 3
- AND `ALTER_HOME/alter.db` 的 `bookmarks` 中 `space_id` 为 `Work` 的 id 的行有：`Dev` 文件夹（`url` 为 NULL），`parent_id` 指向它的 `MDN`，以及 `Rust`，`source` 都为 `chrome`

#### Scenario: 重复导入不产生重复
- WHEN 已从标准 fixture profile 向 Space `Work` 导入过书签，对同一个 profile 和 Space 再运行一次 `import.run`（`items` 为 `["bookmarks"]`）
- THEN `bookmarks` 中 `Work` 的行数仍为 3

### Requirement: 导入最近 90 天的历史
系统 SHALL 只导入源 profile 中访问时间在最近 90 天内的历史记录，按（URL、访问时间）去重。

#### Scenario: 跳过 90 天以前的访问
- WHEN fixture profile 的 `History` 中，`https://a.example/` 在 10 天前有 2 次访问，`https://b.example/` 在 100 天前有 1 次访问，客户端导入 `items` 为 `["history"]`
- THEN `import.done` 的 `counts.history` 为 2
- AND `history` 中该 Space 有 2 行 `https://a.example/`，`source` 为 `chrome`，`visited_at` 与 fixture 的访问时间（换算为 UTC 毫秒）相等，没有 `https://b.example/`

### Requirement: 来源不存在
系统 SHALL 在源 profile 目录不存在或其中三个数据文件都不存在时拒绝导入，并返回 `E_IMPORT_SOURCE_NOT_FOUND`。

#### Scenario: 目录不存在
- WHEN 客户端调用 `import.run`，`profilePath` 为 `/nonexistent/profile`
- THEN `error.data.code` 为 `E_IMPORT_SOURCE_NOT_FOUND`，`imports` 表没有新行

### Requirement: 默认 profile 与 Arc
系统 SHALL 在省略 `profilePath` 时使用来源浏览器数据根目录下的 `Default` profile。

#### Scenario: Arc 默认 profile
- WHEN `ALTER_ARC_DIR` 指向一个临时目录，其 `Default` 子目录是标准 fixture profile，客户端调用 `import.run {"source":"arc","spaceId":"<id>","items":["bookmarks"]}`
- THEN `import.done` 的 `counts.bookmarks` 为 3，`bookmarks` 中这些行的 `source` 为 `arc`

#### Scenario: 列出可导入的 profile
- WHEN `ALTER_CHROME_DIR` 下有 `Default` 与 `Profile 1` 两个含 `Bookmarks` 的目录，Electron 发送 `{"type":"shell.event.importSourcesRequested","requestId":5}`
- THEN Electron 收到 `shell.importSources`，`requestId` 为 5，`chrome` 为 `["Default","Profile 1"]`，`arc` 为空数组

### Requirement: 进度事件
系统 SHALL 在导入每完成一类数据时向发起导入的连接推送 `import.progress`，全部结束时推送 `import.done`，并让 `import.status` 返回同样的状态与计数；省略 `items` 时导入书签与历史。

#### Scenario: 省略 items 时两类数据的进度
- WHEN 客户端从标准 fixture profile 调用 `import.run`，省略 `items`
- THEN 客户端依次收到 2 条 `import.progress`（`item` 依次为 `bookmarks`、`history`）和 1 条 `import.done`，其 `status` 为 `done`，`counts.bookmarks` 为 3、`counts.history` 为 2
- AND 之后调用 `import.status {"importId":<id>}` 返回的 `status` 与 `counts` 与 `import.done` 相同

### Requirement: 导入 Cookie
系统 SHALL 在 `items` 含 `cookies` 时，用来源浏览器的 Safe Storage 口令解密源 profile `Cookies` 库中未过期的 Cookie，经 Shell 指令 `shell.importCookies` 交给最近一次上报 `shell.event.cdpReady` 的 Shell 连接写入目标 Space，并把该连接回报写入成功的条数计入 `counts.cookies`。

#### Scenario: 导入未过期的 Cookie
- WHEN `ALTER_KEYCHAIN_FAKE` 为 `peanuts`，fixture profile 只有标准 Cookie fixture，一个已发送 `shell.event.cdpReady` 的假 Electron 端已连接，客户端调用 `import.run {"source":"chrome","profilePath":"<fixture>","spaceId":"<Work 的 id>","items":["cookies"]}`
- THEN 假 Electron 端收到一条 `shell.importCookies`，`spaceId` 为 `Work` 的 id，`cookies` 恰好 2 项：`sid`（`value` 为 `abc`，`domain` 为 `127.0.0.1`，`httpOnly` 为 true，带 `expirationDate`）与 `theme`（`value` 为 `dark`，`domain` 为 `.example.com`，没有 `expirationDate`）
- AND 假 Electron 端回 `{"type":"shell.event.cookiesImported","requestId":<同一 requestId>,"imported":2,"rejected":0}` 后，`import.done` 的 `status` 为 `done`，`counts.cookies` 为 2，`counts.skipped` 为 0
- AND `imports` 表该行的 `cookies_count` 为 2

#### Scenario: Electron 拒绝写入的 Cookie 计入跳过
- WHEN 条件与上一场景相同，但假 Electron 端回 `"imported":1,"rejected":1`
- THEN `import.done` 的 `status` 为 `done`，`counts.cookies` 为 1，`counts.skipped` 为 1

#### Scenario: Arc 用 Arc Safe Storage
- WHEN `ALTER_ARC_DIR` 的 `Default` 子目录只有标准 Cookie fixture，`ALTER_KEYCHAIN_FAKE` 为 `peanuts`，客户端调用 `import.run {"source":"arc","spaceId":"<id>","items":["cookies"]}`，假 Electron 端如实回报
- THEN `import.done` 的 `counts.cookies` 为 2
- AND alterd 的 JSON 日志里有一行 `取 Safe Storage 口令`，其 `service` 为 `Arc Safe Storage`

#### Scenario: 新版库的主机名前缀
- WHEN fixture 的 `Cookies` 库 `meta.version` 为 24，`sid` 的明文前 32 字节是 `SHA256("127.0.0.1")`、其后才是 `abc`，另一行 `bad` 的前 32 字节是 `SHA256("other.example")`、而 `host_key` 为 `127.0.0.1`，客户端导入 `cookies`
- THEN `shell.importCookies` 里 `sid` 的 `value` 恰好为 `abc`
- AND 没有 `bad` 这一项，`import.done` 的 `counts.skipped` 至少为 1

### Requirement: 钥匙串拒绝授权时部分完成
系统 SHALL 在取不到 Safe Storage 口令（用户拒绝授权或钥匙串里没有这一项）时不导入任何 Cookie，其余所选类别照常导入，导入以 `status` 为 `partial`、`warning` 为 `keychain_denied` 结束。

#### Scenario: 拒绝授权
- WHEN `ALTER_KEYCHAIN_FAKE` 为 `denied`，fixture profile 有标准书签、历史与标准 Cookie fixture，客户端调用 `import.run`，`items` 为 `["bookmarks","history","cookies"]`
- THEN `import.done` 的 `status` 为 `partial`，`warning` 为 `keychain_denied`，`counts` 为 `{"bookmarks":3,"history":2,"cookies":0,"skipped":0}`
- AND 假 Electron 端没有收到 `shell.importCookies`
- AND 之后 `import.status` 返回同样的 `status` 与 `warning`

### Requirement: 跳过无法解密的 Cookie
系统 SHALL 跳过无法解密、填充不合法或解密后不是 UTF-8 的 Cookie，计入 `counts.skipped`，并继续导入其余 Cookie。

#### Scenario: 一行密文被截断
- WHEN 标准 Cookie fixture 之外再加一行 `broken`，其 `encrypted_value` 为 `v10` 加 7 个字节，`ALTER_KEYCHAIN_FAKE` 为 `peanuts`，客户端导入 `cookies`，假 Electron 端如实回报
- THEN `import.done` 的 `status` 为 `done`，`counts.cookies` 为 2，`counts.skipped` 为 1

#### Scenario: 明文值不加密
- WHEN `Cookies` 库只有一行 `plain`：`value` 为 `p1`，`encrypted_value` 为空，`ALTER_KEYCHAIN_FAKE` 为 `peanuts`，客户端导入 `cookies`
- THEN `shell.importCookies` 里 `plain` 的 `value` 为 `p1`，`counts.cookies` 为 1

### Requirement: 没有 Shell 连接时部分完成
系统 SHALL 在需要写入 Cookie 而没有任何上报过 `shell.event.cdpReady` 的 Shell 连接，或该连接 10 秒内没有回报 `shell.event.cookiesImported` 时，把这一批 Cookie 记为未导入，导入以 `status` 为 `partial`、`warning` 为 `shell_unavailable` 结束。

#### Scenario: alterd 独立运行
- WHEN alterd 以 `alterd run` 独立运行，没有任何 Shell 连接，`ALTER_KEYCHAIN_FAKE` 为 `peanuts`，客户端调用 `import.run`，`items` 为 `["bookmarks","cookies"]`
- THEN `import.done` 的 `status` 为 `partial`，`warning` 为 `shell_unavailable`，`counts.bookmarks` 为 3，`counts.cookies` 为 0

#### Scenario: Electron 不回报
- WHEN 已上报 `cdpReady` 的假 Electron 端收到 `shell.importCookies` 后不回复，`ALTER_IMPORT_COOKIE_TIMEOUT_MS` 为 500
- THEN 2 秒内收到 `import.done`，`status` 为 `partial`，`warning` 为 `shell_unavailable`，`counts.cookies` 为 0

### Requirement: Cookie 明文不落盘、不写日志
系统 SHALL 只在内存中持有 Safe Storage 口令、派生密钥与解密后的 Cookie 值，导入结束后 `ALTER_HOME` 下的任何文件与 alterd 的日志输出都不包含它们。

#### Scenario: 导入后找不到明文
- WHEN 标准 Cookie fixture 的 `sid` 值改为 `s3cr3t-value-7f2c`，`ALTER_KEYCHAIN_FAKE` 为 `peanuts`，客户端导入 `cookies` 并等到 `import.done`，然后停掉 alterd
- THEN alterd 的 stderr 全文不包含 `s3cr3t-value-7f2c` 也不包含 `peanuts`
- AND 递归读取 `ALTER_HOME` 下的每个文件，没有一个包含字节串 `s3cr3t-value-7f2c`
- AND `ALTER_HOME/import-tmp` 不存在或为空
