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
