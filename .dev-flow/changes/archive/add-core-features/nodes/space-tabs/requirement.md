---
id: add-core-features
node: N1
title: Space 管理与标签整理
repos: [alter]
revision: 1
approved_revision: 1
---

# N1 Space 管理与标签整理

<!-- 流程路线型需求中一个节点（子流程）的需求文档。这里写本节点的 Spec Delta；整体（背景、范围、非功能需求）在父级 requirement.md，节点顺序在 flow.md。保留字保持英文："Spec Delta:"、"ADDED/MODIFIED/REMOVED/RENAMED Requirement:"、"Scenario:"、WHEN/THEN/AND、SHALL/MUST。语法见 references/formats.md §3 和 §14 -->

## 范围
- 交付：
  - Space 的新建、编辑（名称、颜色）、切换、删除；重启后回到上次所在的 Space；Space 之间 Cookie 隔离的端到端证明；删除后清理 partition。
  - 固定标签（Arc 式：关闭只卸载页面）、拖拽排序（拖过分隔线即固定或取消固定）。
  - 应用菜单与快捷键。
  - 迁移 0002 与迁移前备份。
  - 交给下游：新表结构、Space 事件与 `shell.discard`、快照里的 `pinned`、「同 Space 内连续编号」的 `position` 规则、`apps/browser/src/main/menu.ts` 里的菜单注册表。
- 依赖：无（基于 `main` 上 PR #3 的代码）。
- 不做：Space 拖拽排序、标签跨 Space 移动、命令栏（N4 会把 Cmd+T 改为打开命令栏）。

术语：本节点的「预设颜色」指 8 个颜色值 `#6366f1`、`#ef4444`、`#f97316`、`#eab308`、`#22c55e`、`#06b6d4`、`#3b82f6`、`#ec4899`。「当前 Space」指快照里的 `activeSpaceId`。

## Spec Delta: spaces

### ADDED Requirement: R1 新建 Space
系统 SHALL 在收到新建 Space 的请求时，用去掉首尾空白后长度为 1–32 个字符的名称和一个预设颜色，在全部 Space 的末尾新建一个 Space 并把它设为当前 Space。

#### Scenario: R1.S1 新建后成为当前 Space
- WHEN alterd 只有 `Personal` 一个 Space，客户端发送 `{"type":"shell.event.spaceCreated","name":"  Work ","color":"#f97316"}`
- THEN 新快照的 `spaces` 依次为 `Personal`、`Work`，`Work` 的 `color` 为 `#f97316`
- AND 快照的 `activeSpaceId` 等于 `Work` 的 `id`

#### Scenario: R1.S2 名称为空被拒绝
- WHEN 客户端发送 `spaceCreated`，`name` 为 `"   "`，`color` 为 `#f97316`
- THEN 客户端收到 `shell.error`，`code` 为 `E_INVALID_ARGUMENT`
- AND 之后 1 秒内没有收到 `shell.stateChanged`

#### Scenario: R1.S3 名称超过 32 个字符被拒绝
- WHEN 客户端发送 `spaceCreated`，`name` 为 33 个 `a`，`color` 为 `#f97316`
- THEN 客户端收到 `shell.error`，`code` 为 `E_INVALID_ARGUMENT`

#### Scenario: R1.S4 非预设颜色被拒绝
- WHEN 客户端发送 `spaceCreated`，`name` 为 `Work`，`color` 为 `#123456`
- THEN 客户端收到 `shell.error`，`code` 为 `E_INVALID_ARGUMENT`

#### Scenario: R1.S5 在侧边栏新建 Space
- WHEN 以空的临时目录为 `ALTER_HOME` 启动 app，用户点击侧边栏底部的「+」，在面板的名称框输入 `Work`，点第 3 个颜色（`#f97316`），再点「创建」
- THEN 2 秒内侧边栏底部有 2 个 Space 圆点，第 2 个处于激活样式
- AND 底部的 Space 名称显示 `Work`，标签列表为空，内容区显示 `没有打开的标签`

### ADDED Requirement: R2 编辑 Space
系统 SHALL 在收到编辑请求时，按与新建相同的名称和颜色规则更新指定 Space 的名称或颜色，不改变它的 `id`、顺序和当前 Space。

#### Scenario: R2.S1 改名
- WHEN 当前 Space 为 `Personal`，另有 Space `Work`，客户端发送 `{"type":"shell.event.spaceUpdated","spaceId":"<Work 的 id>","name":"Office"}`
- THEN 新快照的 `spaces` 依次为 `Personal`、`Office`，`Office` 的 `id` 与原 `Work` 相同，`color` 不变
- AND `activeSpaceId` 仍等于 `Personal` 的 `id`

#### Scenario: R2.S2 Space 不存在
- WHEN 客户端发送 `spaceUpdated`，`spaceId` 为 `01JZZZZZZZZZZZZZZZZZZZZZZZ`，`name` 为 `X`
- THEN 客户端收到 `shell.error`，`code` 为 `E_SPACE_NOT_FOUND`

#### Scenario: R2.S3 在侧边栏改名
- WHEN app 只有 `Personal` 一个 Space，用户右键它的圆点，在面板里把名称改成 `Home` 并点「保存」
- THEN 2 秒内底部的 Space 名称显示 `Home`
- AND `ALTER_HOME/alter.db` 中该 Space 的 `spaces.name` 为 `Home`

### ADDED Requirement: R3 切换 Space
系统 SHALL 在用户切换到某个 Space 后，把它设为当前 Space，侧边栏只列出它的标签，内容区显示它自己记住的激活标签。

#### Scenario: R3.S1 切换改变当前 Space
- WHEN 有 Space `Personal` 与 `Work`，当前为 `Work`，客户端发送 `{"type":"shell.event.spaceActivated","spaceId":"<Personal 的 id>"}`
- THEN 新快照的 `activeSpaceId` 等于 `Personal` 的 `id`

#### Scenario: R3.S2 Space 不存在
- WHEN 客户端发送 `spaceActivated`，`spaceId` 为 `01JZZZZZZZZZZZZZZZZZZZZZZZ`
- THEN 客户端收到 `shell.error`，`code` 为 `E_SPACE_NOT_FOUND`

#### Scenario: R3.S3 每个 Space 记住自己的激活标签
- WHEN app 的 `Personal` 里打开了本地测试页 `/a`，用户新建 Space `Work` 并在其中打开 `/b`，然后点击 `Personal` 的圆点
- THEN 1 秒内侧边栏标签列表恰好为 `Page A`，内容区页面的 URL 为 `/a` 的地址
- AND 再点击 `Work` 的圆点后 1 秒内，标签列表恰好为 `Page B`，内容区页面的 URL 为 `/b` 的地址

### ADDED Requirement: R4 重启后回到上次的 Space
系统 SHALL 在 alterd 重启后，把上次关闭前的当前 Space 作为当前 Space；从未切换过 Space 的数据库以排在最前的 Space 为当前 Space。

#### Scenario: R4.S1 重启保留当前 Space
- WHEN 以同一个临时 `ALTER_HOME` 运行 `alterd run`，客户端发送 `spaceCreated`（`Work`、`#f97316`）使 `Work` 成为当前 Space，然后以 SIGTERM 结束 alterd 并再次运行
- THEN 新连接收到的第一份快照中 `activeSpaceId` 等于 `Work` 的 `id`

#### Scenario: R4.S2 旧数据库没有记录时取第一个 Space
- WHEN `ALTER_HOME/alter.db` 是一个只应用了迁移 0001 的数据库，其中有 `position` 为 0 的 `Personal` 和 `position` 为 1 的 `Work`，运行 `alterd run`
- THEN 第一份快照的 `activeSpaceId` 等于 `Personal` 的 `id`

### ADDED Requirement: R5 删除 Space
系统 SHALL 在收到删除请求时，从数据库中删除该 Space 及其全部标签；如果删的是当前 Space，就把它后面相邻的 Space 设为当前 Space，没有后面的就取前面相邻的；只剩一个 Space 时拒绝删除。

#### Scenario: R5.S1 删除中间的当前 Space
- WHEN 有按顺序排列的 Space `Personal`、`Work`、`Play`，当前为 `Work`，`Work` 里有 1 个标签，客户端发送 `{"type":"shell.event.spaceDeleted","spaceId":"<Work 的 id>"}`
- THEN 新快照的 `spaces` 依次为 `Personal`、`Play`，`activeSpaceId` 等于 `Play` 的 `id`
- AND `ALTER_HOME/alter.db` 的 `spaces` 中没有 `Work` 的 `id`，`tabs` 中没有 `space_id` 为 `Work` 的 `id` 的行

#### Scenario: R5.S2 删除末尾的当前 Space
- WHEN 有按顺序排列的 Space `Personal`、`Work`，当前为 `Work`，客户端发送 `spaceDeleted`（`Work` 的 `id`）
- THEN 新快照的 `activeSpaceId` 等于 `Personal` 的 `id`

#### Scenario: R5.S3 删除非当前 Space
- WHEN 有 Space `Personal`、`Work`，当前为 `Personal`，客户端发送 `spaceDeleted`（`Work` 的 `id`）
- THEN 新快照的 `spaces` 只有 `Personal`，`activeSpaceId` 仍等于 `Personal` 的 `id`

#### Scenario: R5.S4 不能删除最后一个 Space
- WHEN 只有 `Personal` 一个 Space，客户端发送 `spaceDeleted`（`Personal` 的 `id`）
- THEN 客户端收到 `shell.error`，`code` 为 `E_INVALID_ARGUMENT`
- AND 之后 1 秒内没有收到 `shell.stateChanged`

#### Scenario: R5.S5 在侧边栏删除 Space 并清掉 Cookie
- WHEN app 有 `Personal` 与 `Work`，当前为 `Work`，`Work` 里打开了本地测试页 `/set-cookie`（响应头 `Set-Cookie: k=v`），用户右键 `Work` 的圆点，点「删除」，在确认框里点「删除」
- THEN 2 秒内底部只剩 1 个 Space 圆点，底部的 Space 名称显示 `Personal`
- AND 主进程里 `session.fromPartition("persist:space-<Work 的 id>").cookies.get({})` 返回空数组

#### Scenario: R5.S6 取消确认不删除
- WHEN app 有 `Personal` 与 `Work`，用户右键 `Work` 的圆点，点「删除」，在确认框里点「取消」
- THEN 1 秒后底部仍有 2 个 Space 圆点，`ALTER_HOME/alter.db` 的 `spaces` 中仍有 `Work`

### ADDED Requirement: R6 启动时清理已删除 Space 的数据目录
系统 SHALL 在 alterd 启动时删除 `ALTER_HOME/chromium/Partitions/` 下名为 `space-<id>`、而 `<id>`（不区分大小写）不属于任何现存 Space 的目录，其他目录保持不动。

#### Scenario: R6.S1 只删除孤儿目录
- WHEN `ALTER_HOME` 里已有 Space `Personal`（数据库中的 id 例如 `01JABCDEFGHJKMNPQRSTVWXYZ0`），`ALTER_HOME/chromium/Partitions/` 下有 `space-01jabcdefghjkmnpqrstvwxyz0`、`space-01jzzzzzzzzzzzzzzzzzzzzzzz`、`default` 三个目录，运行 `alterd run`
- THEN `ALTER_HOME/alterd.json` 出现时，`space-01jzzzzzzzzzzzzzzzzzzzzzzz` 已不存在
- AND `space-01jabcdefghjkmnpqrstvwxyz0` 与 `default` 仍然存在

### ADDED Requirement: R7 Space 之间 Cookie 隔离
系统 SHALL 让每个 Space 的页面只能读写自己 Space 的 Cookie。

#### Scenario: R7.S1 另一个 Space 看不到 Cookie
- WHEN app 的 `Personal` 里打开本地测试页 `/set-cookie`（响应头 `Set-Cookie: k=v`），然后新建 Space `Work` 并在其中打开 `/echo-cookie`（页面正文为 `cookie: ` 加请求的 Cookie 头）
- THEN `Work` 的页面正文为 `cookie: `
- AND 切回 `Personal` 并新开标签打开 `/echo-cookie` 后，页面正文为 `cookie: k=v`

## Spec Delta: tabs

### ADDED Requirement: R8 固定与取消固定标签
系统 SHALL 在用户固定一个标签时把它移到该 Space 固定分区的末尾并记下它当前的 URL，在取消固定时把它移到普通分区的开头；标签列表总是固定分区在前、普通分区在后。

#### Scenario: R8.S1 固定后移到固定分区
- WHEN 当前 Space 依次有普通标签 A、B、C（URL 分别为 `https://example.com/a`、`/b`、`/c`），客户端发送 `{"type":"shell.event.tabPinned","tabId":"<C 的 id>","pinned":true}`
- THEN 新快照里该 Space 的标签依次为 C、A、B，C 的 `pinned` 为 `true`，A、B 的为 `false`
- AND `ALTER_HOME/alter.db` 中 C 的 `tabs.pinned_url` 为 `https://example.com/c`

#### Scenario: R8.S2 取消固定后移到普通分区开头
- WHEN 当前 Space 依次有固定标签 P、Q 和普通标签 A，客户端发送 `tabPinned`，`tabId` 为 P 的 id，`pinned` 为 `false`
- THEN 新快照里标签依次为 Q、P、A，P 的 `pinned` 为 `false`

#### Scenario: R8.S3 标签不存在
- WHEN 客户端发送 `tabPinned`，`tabId` 为 `01JZZZZZZZZZZZZZZZZZZZZZZZ`，`pinned` 为 `true`
- THEN 客户端收到 `shell.error`，`code` 为 `E_TAB_NOT_FOUND`

#### Scenario: R8.S4 在侧边栏固定并在重启后保持
- WHEN app 当前 Space 依次打开了本地测试页 `/a`、`/b`，用户把鼠标移到 `Page B` 那一行并点击图钉按钮，然后正常退出并以同一个 `ALTER_HOME` 再次启动
- THEN 重启后 3 秒内侧边栏标签依次为 `Page B`、`Page A`，`Page B` 那一行在固定分区（分隔线上方）

### MODIFIED Requirement: 关闭标签
系统 SHALL 在用户关闭一个非固定标签后把它从侧边栏移除并标记为已归档；若关闭的是激活标签，则激活它下方相邻的标签，没有下方标签时激活上方相邻的标签。

#### Scenario: R9.S1 关闭中间的激活标签
- WHEN 当前 Space 有按顺序排列的标签 A、B、C，B 为激活标签，客户端发送 `{"type":"shell.event.tabClosed","tabId":"<B 的 id>"}`
- THEN 新快照的 `tabs` 依次为 A、C，C 的 `active` 为 `true`
- AND 数据库中 B 的 `tabs.archived_at` 不为 NULL

#### Scenario: R9.S2 关闭最后一个标签
- WHEN app 中当前 Space 只有一个标签，用户点击该行的关闭按钮
- THEN 1 秒内侧边栏标签列表为空，内容区显示 `没有打开的标签`

### ADDED Requirement: R10 关闭固定标签只卸载页面
系统 SHALL 在用户关闭一个固定标签时保留它在侧边栏中的行，把它的 URL 恢复为固定时记下的 URL 并卸载它的页面；若它是激活标签，按普通标签的相邻规则激活另一个标签。

#### Scenario: R10.S1 关闭固定标签
- WHEN 当前 Space 依次有固定标签 P（固定时 URL 为 `https://example.com/p`，之后经 `tabUpdated` 导航到 `https://example.com/q`）和普通标签 A，P 为激活标签，客户端发送 `tabClosed`（P 的 id）
- THEN 客户端收到 `{"type":"shell.discard","tabId":"<P 的 id>"}`
- AND 新快照的标签依次为 P、A，P 的 `url` 为 `https://example.com/p`、`pinned` 为 `true`、`active` 为 `false`，A 的 `active` 为 `true`
- AND 数据库中 P 的 `tabs.archived_at` 为 NULL

#### Scenario: R10.S2 在侧边栏关闭固定标签后再打开
- WHEN app 当前 Space 有固定在 `/a`、之后在该标签里导航到 `/b` 的固定标签，以及普通标签 `Page C`，固定标签为激活标签，用户点击它的关闭按钮
- THEN 1 秒内该行仍在固定分区，主进程全部 webContents 的 URL 中不包含 `/b` 的地址
- AND 用户点击该行后 2 秒内，内容区页面的 URL 为 `/a` 的地址

### ADDED Requirement: R11 拖拽排序标签
系统 SHALL 在用户把标签拖到某个分区的第 `index` 位（从 0 开始，超过该分区长度时放到末尾）后，按新顺序保存，拖入另一个分区时随之固定或取消固定。

#### Scenario: R11.S1 在普通分区内移动
- WHEN 当前 Space 依次有普通标签 A、B、C，客户端发送 `{"type":"shell.event.tabMoved","tabId":"<C 的 id>","pinned":false,"index":0}`
- THEN 新快照里标签依次为 C、A、B

#### Scenario: R11.S2 拖入固定分区即固定
- WHEN 当前 Space 依次有固定标签 P 和普通标签 A、B（B 的 URL 为 `https://example.com/b`），客户端发送 `tabMoved`，`tabId` 为 B 的 id，`pinned` 为 `true`，`index` 为 1
- THEN 新快照里标签依次为 P、B、A，B 的 `pinned` 为 `true`
- AND 数据库中 B 的 `tabs.pinned_url` 为 `https://example.com/b`

#### Scenario: R11.S3 index 超出范围时放到末尾
- WHEN 当前 Space 依次有普通标签 A、B、C，客户端发送 `tabMoved`，`tabId` 为 A 的 id，`pinned` 为 `false`，`index` 为 99
- THEN 新快照里标签依次为 B、C、A

#### Scenario: R11.S4 标签不存在
- WHEN 客户端发送 `tabMoved`，`tabId` 为 `01JZZZZZZZZZZZZZZZZZZZZZZZ`，`pinned` 为 `false`，`index` 为 0
- THEN 客户端收到 `shell.error`，`code` 为 `E_TAB_NOT_FOUND`

#### Scenario: R11.S5 在侧边栏拖动并在重启后保持
- WHEN app 当前 Space 依次打开了本地测试页 `/a`、`/b`、`/c`，用户把 `Page C` 那一行拖到 `Page A` 那一行上方
- THEN 1 秒内侧边栏标签依次为 `Page C`、`Page A`、`Page B`
- AND 正常退出并以同一个 `ALTER_HOME` 重启后，顺序仍为 `Page C`、`Page A`、`Page B`

## Spec Delta: app-shell

### ADDED Requirement: R12 快捷键
系统 SHALL 在应用菜单中提供下表的菜单项，按下快捷键或点击菜单项时执行对应操作：

| 菜单项 id | 快捷键 | 操作 |
|---|---|---|
| `tab.new` | `CmdOrCtrl+T` | 聚焦地址框并进入新标签模式 |
| `tab.close` | `CmdOrCtrl+W` | 关闭当前 Space 的激活标签 |
| `address.focus` | `CmdOrCtrl+L` | 聚焦地址框并选中其中文字 |
| `nav.reload` | `CmdOrCtrl+R` | 刷新激活标签 |
| `nav.back` | `CmdOrCtrl+[` | 激活标签后退 |
| `nav.forward` | `CmdOrCtrl+]` | 激活标签前进 |
| `space.1` … `space.9` | `CmdOrCtrl+1` … `CmdOrCtrl+9` | 切到第 N 个 Space，不存在时什么也不做 |
| `tab.next` | `Ctrl+Tab` | 激活列表中的下一个标签，到末尾后回到第一个 |
| `tab.prev` | `Ctrl+Shift+Tab` | 激活列表中的上一个标签，到开头后回到最后一个 |
| `tab.togglePin` | `CmdOrCtrl+Shift+D` | 固定或取消固定激活标签 |

#### Scenario: R12.S1 菜单项与快捷键
- WHEN app 启动后在主进程读取 `Menu.getApplicationMenu()`
- THEN 上表每个菜单项 id 都存在，且其 `accelerator` 与表中的快捷键字符串完全相同

#### Scenario: R12.S2 关闭激活标签
- WHEN app 当前 Space 依次打开了本地测试页 `/a`、`/b`（`Page B` 激活），在主进程点击菜单项 `tab.close`
- THEN 1 秒内侧边栏标签列表恰好为 `Page A`，且它处于激活样式

#### Scenario: R12.S3 按序号切换 Space
- WHEN app 有按顺序排列的 Space `Personal`、`Work`，当前为 `Work`，在主进程点击菜单项 `space.1`
- THEN 1 秒内底部的 Space 名称显示 `Personal`
- AND 再点击菜单项 `space.3` 后 1 秒，底部的 Space 名称仍为 `Personal`

#### Scenario: R12.S4 循环切换标签
- WHEN app 当前 Space 依次打开了本地测试页 `/a`、`/b`、`/c`（`Page C` 激活），在主进程点击菜单项 `tab.next`
- THEN 1 秒内 `Page A` 处于激活样式
- AND 再点击菜单项 `tab.prev` 后 1 秒内 `Page C` 处于激活样式

#### Scenario: R12.S5 新标签与聚焦地址框
- WHEN app 当前 Space 打开了本地测试页 `/a`，在主进程点击菜单项 `tab.new`，然后在键盘上输入 `127.0.0.1:<port>/b` 并回车
- THEN 2 秒内侧边栏依次为 `Page A`、`Page B`，`Page B` 处于激活样式
- AND 点击菜单项 `address.focus` 后，Renderer 的 `document.activeElement` 是地址框

#### Scenario: R12.S6 快捷键固定激活标签
- WHEN app 当前 Space 依次打开了本地测试页 `/a`、`/b`（`Page B` 激活），在主进程点击菜单项 `tab.togglePin`
- THEN 1 秒内侧边栏依次为 `Page B`、`Page A`，`Page B` 在固定分区

#### Scenario: R12.S7 快捷键刷新
- WHEN 激活标签显示本地测试页 `/count`（服务器记录的请求次数为 1），在主进程点击菜单项 `nav.reload`
- THEN 2 秒内服务器记录的 `/count` 请求次数为 2

## Spec Delta: storage

### MODIFIED Requirement: 数据库初始化与迁移
系统 SHALL 在打开数据目录时创建 SQLite 数据库文件并应用全部尚未应用的迁移，使表结构与 `data-model.md` 一致。

#### Scenario: R13.S1 空目录首次打开
- WHEN `ALTER_HOME` 指向一个空的临时目录，并打开存储
- THEN `<ALTER_HOME>/alter.db` 文件存在
- AND `sqlite_master` 中存在 `schema_version`、`spaces`、`tabs`、`bookmarks`、`history`、`task_spaces`、`task_pages`、`imports` 共 8 张表
- AND `SELECT MAX(version) FROM schema_version` 返回 2
- AND `spaces` 有 `last_active_at` 列，`tabs` 有 `pinned_url` 列

#### Scenario: R13.S2 重复打开不重复迁移
- WHEN 同一个 `ALTER_HOME` 下的存储已经打开并迁移过一次，再次打开
- THEN 打开成功，没有错误
- AND `SELECT COUNT(*) FROM schema_version` 返回 2

#### Scenario: R13.S3 WAL 与外键生效
- WHEN 存储打开后执行 `PRAGMA journal_mode` 与 `PRAGMA foreign_keys`
- THEN 两者分别返回 `wal` 与 `1`
- AND 向 `tabs` 插入一行 `space_id` 为 `no-such-space` 的记录被拒绝，错误信息包含 `FOREIGN KEY constraint failed`

#### Scenario: R13.S4 已有标签按分区重新编号
- WHEN 一个只应用了迁移 0001 的数据库里，Space `Personal` 有未归档标签 A（`position` 0）、B（`position` 5，`pinned` 1）、C（`position` 9），打开存储
- THEN A、B、C 的 `position` 分别为 1、0、2

### ADDED Requirement: R14 迁移前备份
系统 SHALL 在对已有数据库应用新的迁移之前，把数据库完整复制为同目录下的 `alter.db.bak-<迁移前的版本号>`。

#### Scenario: R14.S1 从版本 1 升级时生成备份
- WHEN `ALTER_HOME/alter.db` 是一个只应用了迁移 0001、含 Space `Old` 的数据库，打开存储
- THEN `ALTER_HOME/alter.db.bak-1` 存在，以 SQLite 打开它时 `SELECT MAX(version) FROM schema_version` 返回 1，`spaces` 中有 `Old`
- AND `alter.db` 的 `SELECT MAX(version) FROM schema_version` 返回 2，`spaces` 中仍有 `Old`

#### Scenario: R14.S2 新数据库和已是最新的数据库不生成备份
- WHEN `ALTER_HOME` 为空目录，打开存储后关闭，再打开一次
- THEN `ALTER_HOME` 下没有名字以 `alter.db.bak-` 开头的文件

## Spec Delta: alterd-lifecycle

### MODIFIED Requirement: 守护进程启动与退出
系统 SHALL 在 `alterd run` 启动后把进程信息写入数据目录的健康文件 `alterd.json`，并在收到终止信号后删除它再退出。

#### Scenario: R15.S1 启动后写出健康文件
- WHEN `ALTER_HOME` 指向一个空的临时目录，启动 `alterd run`
- THEN 1 秒内出现 `<ALTER_HOME>/alterd.json`
- AND 文件是 JSON，`pid` 等于该进程的 pid，`version` 为 `0.1.0`，`dbPath` 为 `<ALTER_HOME>/alter.db`，`startedAt` 是毫秒级 Unix 时间戳整数
- AND `<ALTER_HOME>/alter.db` 存在，且 `SELECT MAX(version) FROM schema_version` 返回 2

#### Scenario: R15.S2 SIGTERM 后清理退出
- WHEN 一个已写出健康文件的 `alterd run` 收到 SIGTERM
- THEN 进程在 2 秒内退出，退出码为 0
- AND `<ALTER_HOME>/alterd.json` 已被删除

#### Scenario: R15.S3 拒绝第二个实例
- WHEN 同一个 `ALTER_HOME` 下已有一个 `alterd run` 在运行，再启动一个 `alterd run`
- THEN 第二个进程在 2 秒内退出，退出码为 3
- AND 第二个进程的 stderr 包含 `already running`
- AND 第一个进程仍在运行，`alterd.json` 里的 `pid` 仍是第一个进程的 pid

#### Scenario: R15.S4 接管残留的健康文件
- WHEN `<ALTER_HOME>/alterd.json` 已存在且其中 `pid` 为 999999（不存在的进程），启动 `alterd run`
- THEN 1 秒内 `alterd.json` 被改写，`pid` 等于新进程的 pid
- AND 启动后 1 秒时新进程仍在运行

## Spec Delta: protocol

### MODIFIED Requirement: 协议类型导出
系统 SHALL 把 Rust 里标记为导出的协议类型生成为 TypeScript 声明文件写入指定目录。

#### Scenario: R16.S1 导出错误码与健康文件类型
- WHEN 执行 `cargo run -p alter-core --bin export-ts -- <空的临时目录>`
- THEN 退出码为 0
- AND 目录中存在 `ErrorCode.ts`，其类型是字符串字面量联合，恰好包含 `E_UNAUTHORIZED`、`E_PROTOCOL_VERSION`、`E_SPACE_NOT_FOUND`、`E_TASK_SPACE_NOT_FOUND`、`E_PAGE_NOT_FOUND`、`E_PAGE_BUDGET`、`E_USER_CONTROL`、`E_AMBIGUOUS_SELECTOR`、`E_NO_MATCH`、`E_TIMEOUT`、`E_DIALOG_OPEN`、`E_IMPORT_SOURCE_NOT_FOUND`、`E_KEYCHAIN_DENIED`、`E_INTERNAL`、`E_TAB_NOT_FOUND`、`E_INVALID_ARGUMENT` 这 16 个字面量
- AND 目录中存在 `HealthFile.ts`，其字段恰好为 `version`、`pid`、`startedAt`、`dbPath`

#### Scenario: R16.S2 导出 Shell Control 消息类型
- WHEN 执行 `cargo run -p alter-core --bin export-ts -- <空的临时目录>`
- THEN 目录中存在 `ShellCommand.ts`，其 `type` 字段的字符串字面量恰好为 `shell.stateChanged`、`shell.navigate`、`shell.error`、`shell.discard`
- AND 目录中存在 `ShellEvent.ts`，其 `type` 字段的字符串字面量恰好为 `shell.event.openRequested`、`shell.event.tabUpdated`、`shell.event.tabActivated`、`shell.event.tabClosed`、`shell.event.spaceCreated`、`shell.event.spaceUpdated`、`shell.event.spaceActivated`、`shell.event.spaceDeleted`、`shell.event.tabPinned`、`shell.event.tabMoved`
- AND 目录中存在 `TabView.ts`，其字段包含 `pinned`

## 变更记录
- 2026-10-21: 创建。
