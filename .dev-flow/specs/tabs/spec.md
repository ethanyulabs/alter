# tabs Specification

## Purpose
Space 内的页面：打开、导航、激活、关闭与恢复。

## Requirements

### Requirement: 地址输入规范化
系统 SHALL 按以下规则把地址输入转换为 URL：去掉首尾空白后，以 `<scheme>://`、`about:` 或 `blob:` 开头的原样使用；不含空白且形如「主机名（含点号）[:端口][/路径]」的补 `https://`；主机为 `localhost` 或 IPv4 地址的补 `http://`；其余作为 `https://www.google.com/search?q=` 加 URL 编码后的输入。

#### Scenario: 带协议的 URL 原样打开
- WHEN 客户端发送 `openRequested`，`input` 为 `  https://example.com/a?x=1  `
- THEN 新快照里新增标签的 `url` 为 `https://example.com/a?x=1`

#### Scenario: 裸域名补 https
- WHEN 客户端发送 `openRequested`，`input` 为 `example.com/docs`
- THEN 新增标签的 `url` 为 `https://example.com/docs`

#### Scenario: localhost 与 IP 补 http
- WHEN 客户端依次发送 `input` 为 `localhost:8080` 与 `127.0.0.1:3000/x` 的两条 `openRequested`
- THEN 两个新增标签的 `url` 依次为 `http://localhost:8080` 与 `http://127.0.0.1:3000/x`

#### Scenario: 其余内容走 Google 搜索
- WHEN 客户端发送 `openRequested`，`input` 为 `rust 生命周期`
- THEN 新增标签的 `url` 为 `https://www.google.com/search?q=rust%20%E7%94%9F%E5%91%BD%E5%91%A8%E6%9C%9F`

#### Scenario: blob URL 原样打开
- WHEN 客户端发送 `openRequested`，`input` 为 `blob:https://example.com/0f6c1e2a-5b7d-4c3e-9a1f-2d8e7b6c5a40`
- THEN 新增标签的 `url` 为 `blob:https://example.com/0f6c1e2a-5b7d-4c3e-9a1f-2d8e7b6c5a40`

#### Scenario: 空输入被拒绝
- WHEN 客户端发送 `openRequested`，`input` 为 `   `
- THEN 客户端收到 `shell.error`，`code` 为 `E_INVALID_ARGUMENT`
- AND 之后 1 秒内没有收到 `shell.stateChanged`

### Requirement: 新建标签
系统 SHALL 在用户以「新标签」方式提交地址时，在当前 Space 的标签列表末尾新建一个标签、把它设为激活标签，并在内容区加载它的 URL。

#### Scenario: 从侧边栏新建标签
- WHEN app 已启动、本地测试服务器在 `127.0.0.1:<port>` 提供标题为 `Page A` 的 `/a`，用户点击「新标签」，在地址框输入 `127.0.0.1:<port>/a` 并回车
- THEN 2 秒内侧边栏标签列表恰好 1 行且处于激活样式
- AND 内容区页面的 URL 为 `http://127.0.0.1:<port>/a`

### Requirement: 在当前标签打开
系统 SHALL 在用户不以「新标签」方式提交地址时，在当前激活标签里打开该 URL，不新建标签；没有激活标签时按新建标签处理。

#### Scenario: 当前标签导航
- WHEN 当前 Space 有一个激活标签正显示 `http://127.0.0.1:<port>/a`，用户在地址框输入 `127.0.0.1:<port>/b` 并回车
- THEN 2 秒内该标签的页面 URL 为 `http://127.0.0.1:<port>/b`
- AND 侧边栏标签列表仍然恰好 1 行

#### Scenario: 指定标签时下发导航指令
- WHEN 客户端发送 `openRequested`，带上已存在标签的 `tabId`，`input` 为 `example.com`
- THEN 客户端收到 `{"type":"shell.navigate","tabId":"<该 tabId>","url":"https://example.com"}`
- AND 快照中的标签数量不变

### Requirement: 标题回写
系统 SHALL 在页面标题变化后 2 秒内，在侧边栏对应的标签行显示新标题，并把它写入该标签的数据库记录。

#### Scenario: 标题显示并落库
- WHEN 用户新建标签打开标题为 `Page A` 的本地测试页
- THEN 2 秒内侧边栏该行显示 `Page A`
- AND `ALTER_HOME/alter.db` 中该标签的 `tabs.title` 为 `Page A`

### Requirement: 激活标签
系统 SHALL 在用户点击侧边栏中的某个标签后，让内容区显示该标签的页面，并把它记为所在 Space 的激活标签。

#### Scenario: 点击切换
- WHEN 当前 Space 依次新建了标题为 `Page A` 与 `Page B` 的两个标签（`Page B` 激活），用户点击 `Page A` 那一行
- THEN 1 秒内内容区页面的 URL 为 `/a` 的地址，`Page A` 那一行处于激活样式
- AND 新快照中 `Page A` 的标签 `active` 为 `true`、`Page B` 的为 `false`

### Requirement: 关闭标签
系统 SHALL 在用户关闭一个非固定标签后把它从侧边栏移除并标记为已归档；若关闭的是激活标签，则激活它下方相邻的标签，没有下方标签时激活上方相邻的标签。

#### Scenario: 关闭中间的激活标签
- WHEN 当前 Space 有按顺序排列的标签 A、B、C，B 为激活标签，客户端发送 `{"type":"shell.event.tabClosed","tabId":"<B 的 id>"}`
- THEN 新快照的 `tabs` 依次为 A、C，C 的 `active` 为 `true`
- AND 数据库中 B 的 `tabs.archived_at` 不为 NULL

#### Scenario: 关闭最后一个标签
- WHEN app 中当前 Space 只有一个标签，用户点击该行的关闭按钮
- THEN 1 秒内侧边栏标签列表为空，内容区显示 `没有打开的标签`

### Requirement: 重启恢复标签
系统 SHALL 在 app 重启后按关闭前的顺序显示各标签与原激活标签，并且只为激活标签加载页面，其余标签在首次被激活时才加载。

#### Scenario: 顺序与激活标签恢复
- WHEN 同一个临时 `ALTER_HOME` 下 app 依次打开本地测试页 `/a`、`/b`、`/c` 三个标签，点击 `/b` 那一行，正常退出后再次启动 app
- THEN 3 秒内侧边栏标签依次显示 `Page A`、`Page B`、`Page C`，`Page B` 处于激活样式
- AND 内容区页面的 URL 为 `/b` 的地址

#### Scenario: 未激活的标签不预先加载
- WHEN 上一个场景重启后的 app 中，查询主进程的全部 webContents 的 URL
- THEN 包含 `/b` 的地址，不包含 `/a` 与 `/c` 的地址
- AND 用户点击 `Page C` 那一行后 2 秒内，内容区页面的 URL 为 `/c` 的地址

### Requirement: 网页弹窗在当前 Space 新建标签
系统 SHALL 在页面经 `target="_blank"` 链接或 `window.open` 请求打开新窗口时，不创建新的窗口，而是在该页面所属 Space 的标签列表末尾新建并激活一个打开该 URL 的标签。

#### Scenario: 点击新窗口链接
- WHEN 激活标签显示本地测试页 `/popup`（页面里有一个 `target="_blank"`、指向 `/b` 的链接），用户点击该链接
- THEN 2 秒内侧边栏标签列表依次为 `Popup`、`Page B`，`Page B` 处于激活样式
- AND 主进程的 `BrowserWindow.getAllWindows()` 仍然只有 1 个窗口

#### Scenario: window.open 打开 blob 页面
- WHEN 激活标签显示本地测试页 `/blob`，页面执行 `window.open(URL.createObjectURL(new Blob(["<title>Blob</title>"], {type: "text/html"})))`
- THEN 2 秒内侧边栏标签列表依次为 `Blob Opener`、`Blob`，`Blob` 处于激活样式

### Requirement: 固定与取消固定标签
系统 SHALL 在用户固定一个标签时把它移到该 Space 固定分区的末尾并记下它当前的 URL，在取消固定时把它移到普通分区的开头；标签列表总是固定分区在前、普通分区在后。

#### Scenario: 固定后移到固定分区
- WHEN 当前 Space 依次有普通标签 A、B、C（URL 分别为 `https://example.com/a`、`/b`、`/c`），客户端发送 `{"type":"shell.event.tabPinned","tabId":"<C 的 id>","pinned":true}`
- THEN 新快照里该 Space 的标签依次为 C、A、B，C 的 `pinned` 为 `true`，A、B 的为 `false`
- AND `ALTER_HOME/alter.db` 中 C 的 `tabs.pinned_url` 为 `https://example.com/c`

#### Scenario: 取消固定后移到普通分区开头
- WHEN 当前 Space 依次有固定标签 P、Q 和普通标签 A，客户端发送 `tabPinned`，`tabId` 为 P 的 id，`pinned` 为 `false`
- THEN 新快照里标签依次为 Q、P、A，P 的 `pinned` 为 `false`

#### Scenario: 标签不存在
- WHEN 客户端发送 `tabPinned`，`tabId` 为 `01JZZZZZZZZZZZZZZZZZZZZZZZ`，`pinned` 为 `true`
- THEN 客户端收到 `shell.error`，`code` 为 `E_TAB_NOT_FOUND`

#### Scenario: 在侧边栏固定并在重启后保持
- WHEN app 当前 Space 依次打开了本地测试页 `/a`、`/b`，用户把鼠标移到 `Page B` 那一行并点击图钉按钮，然后正常退出并以同一个 `ALTER_HOME` 再次启动
- THEN 重启后 3 秒内侧边栏标签依次为 `Page B`、`Page A`，`Page B` 那一行在固定分区（分隔线上方）

### Requirement: 关闭固定标签只卸载页面
系统 SHALL 在用户关闭一个固定标签时保留它在侧边栏中的行，把它的 URL 恢复为固定时记下的 URL 并卸载它的页面；若它是激活标签，按普通标签的相邻规则激活另一个标签。

#### Scenario: 关闭固定标签
- WHEN 当前 Space 依次有固定标签 P（固定时 URL 为 `https://example.com/p`，之后经 `tabUpdated` 导航到 `https://example.com/q`）和普通标签 A，P 为激活标签，客户端发送 `tabClosed`（P 的 id）
- THEN 客户端收到 `{"type":"shell.discard","tabId":"<P 的 id>"}`
- AND 新快照的标签依次为 P、A，P 的 `url` 为 `https://example.com/p`、`pinned` 为 `true`、`active` 为 `false`，A 的 `active` 为 `true`
- AND 数据库中 P 的 `tabs.archived_at` 为 NULL

#### Scenario: 在侧边栏关闭固定标签后再打开
- WHEN app 当前 Space 有固定在 `/a`、之后在该标签里导航到 `/b` 的固定标签，以及普通标签 `Page C`，固定标签为激活标签，用户点击它的关闭按钮
- THEN 1 秒内该行仍在固定分区，主进程全部 webContents 的 URL 中不包含 `/b` 的地址
- AND 用户点击该行后 2 秒内，内容区页面的 URL 为 `/a` 的地址

### Requirement: 拖拽排序标签
系统 SHALL 在用户把标签拖到某个分区的第 `index` 位（从 0 开始，超过该分区长度时放到末尾）后，按新顺序保存，拖入另一个分区时随之固定或取消固定。

#### Scenario: 在普通分区内移动
- WHEN 当前 Space 依次有普通标签 A、B、C，客户端发送 `{"type":"shell.event.tabMoved","tabId":"<C 的 id>","pinned":false,"index":0}`
- THEN 新快照里标签依次为 C、A、B

#### Scenario: 拖入固定分区即固定
- WHEN 当前 Space 依次有固定标签 P 和普通标签 A、B（B 的 URL 为 `https://example.com/b`），客户端发送 `tabMoved`，`tabId` 为 B 的 id，`pinned` 为 `true`，`index` 为 1
- THEN 新快照里标签依次为 P、B、A，B 的 `pinned` 为 `true`
- AND 数据库中 B 的 `tabs.pinned_url` 为 `https://example.com/b`

#### Scenario: index 超出范围时放到末尾
- WHEN 当前 Space 依次有普通标签 A、B、C，客户端发送 `tabMoved`，`tabId` 为 A 的 id，`pinned` 为 `false`，`index` 为 99
- THEN 新快照里标签依次为 B、C、A

#### Scenario: 标签不存在
- WHEN 客户端发送 `tabMoved`，`tabId` 为 `01JZZZZZZZZZZZZZZZZZZZZZZZ`，`pinned` 为 `false`，`index` 为 0
- THEN 客户端收到 `shell.error`，`code` 为 `E_TAB_NOT_FOUND`

#### Scenario: 在侧边栏拖动并在重启后保持
- WHEN app 当前 Space 依次打开了本地测试页 `/a`、`/b`、`/c`，用户把 `Page C` 那一行拖到 `Page A` 那一行上方
- THEN 1 秒内侧边栏标签依次为 `Page C`、`Page A`、`Page B`
- AND 正常退出并以同一个 `ALTER_HOME` 重启后，顺序仍为 `Page C`、`Page A`、`Page B`

### Requirement: 两个标签分屏
系统 SHALL 在用户把同一 Space 的两个标签组成分屏后，激活其中任一个时在内容区左右各半并排显示两者，左边是发起分屏时的激活标签。

#### Scenario: 组成分屏
- WHEN Space 有标签 A（激活）与 B，客户端发送 `{"type":"shell.event.splitRequested","tabIds":["<A 的 id>","<B 的 id>"]}`
- THEN 新快照中 A 与 B 的 `splitGroup` 相同且不为 null，两者顺序不变

#### Scenario: 跨 Space 被拒绝
- WHEN 标签 A 属于 `Personal`、B 属于 `Work`，客户端发送 `splitRequested`，`tabIds` 为 A 与 B
- THEN 客户端收到 `shell.error`，`code` 为 `E_INVALID_ARGUMENT`

#### Scenario: 不是恰好两个标签被拒绝
- WHEN 客户端发送 `splitRequested`，`tabIds` 只含 A
- THEN 客户端收到 `shell.error`，`code` 为 `E_INVALID_ARGUMENT`

#### Scenario: 在侧边栏分屏
- WHEN app 当前 Space 依次打开了本地测试页 `/a`、`/b`（`Page B` 激活），用户右键 `Page A` 那一行并点「与当前标签分屏」
- THEN 1 秒内窗口上挂载了 2 个 WebContentsView，左边的 URL 为 `/b` 的地址，右边的为 `/a` 的地址，两者宽度之差不超过 1 px
- AND 正常退出并以同一个 `ALTER_HOME` 重启后，仍是这两个视图并排显示

### Requirement: 解散分屏
系统 SHALL 在用户解散分屏或关闭分屏中的一个标签时，清除两者的分屏关系，内容区只显示一个标签。

#### Scenario: 关闭其中一个
- WHEN 标签 A 与 B 处于同一分屏，客户端发送 `tabClosed`（B 的 id）
- THEN 新快照中 A 的 `splitGroup` 为 null

#### Scenario: 显式解散
- WHEN 标签 A 与 B 处于同一分屏，客户端发送 `{"type":"shell.event.splitDissolved","tabId":"<A 的 id>"}`
- THEN 新快照中 A 与 B 的 `splitGroup` 都为 null

### Requirement: 自动归档不活跃的标签
系统 SHALL 把 `last_active_at` 早于当前时间减去阈值（默认 12 小时，可由 `ALTER_HOME/config.toml` 的 `archive_after_hours` 配置）的标签归档，固定标签、各 Space 的激活标签和被 TaskSpace 托管的标签除外。

#### Scenario: 只归档符合条件的标签
- WHEN 环境变量 `ALTER_ARCHIVE_AFTER_MS` 为 300，Space 有普通标签 A、固定标签 P、被 TaskSpace 托管的 agent 标签 G 和激活标签 B，四者此后都不再被激活，等待 1.5 秒
- THEN 新快照中没有 A，P、G、B 仍在
- AND 数据库中 A 的 `tabs.archived_at` 不为 NULL

### Requirement: 已归档列表与恢复
系统 SHALL 在快照里给出每个 Space 最近归档的至多 50 个标签，并在用户恢复其中一个时把它放回该 Space 普通分区的末尾并激活。

#### Scenario: 关闭的标签出现在已归档列表
- WHEN Space 依次有标签 A、B，客户端关闭 A
- THEN 新快照的 `archived` 中有一项 `id` 为 A 的 id、`spaceId` 为该 Space 的 id、`url` 为 A 的 URL

#### Scenario: 恢复
- WHEN A 已归档，Space 只剩标签 B，客户端发送 `{"type":"shell.event.tabRestored","tabId":"<A 的 id>"}`
- THEN 新快照的标签依次为 B、A，A 的 `active` 为 `true`，`archived` 中不再有 A

#### Scenario: 列表上限
- WHEN 一个 Space 依次关闭了 60 个标签
- THEN 快照的 `archived` 中该 Space 的项恰好 50 个，且是最后关闭的 50 个

#### Scenario: 在侧边栏恢复
- WHEN app 当前 Space 打开 `/a` 与 `/b` 后关闭了 `Page A`，用户点击侧边栏的「已归档」，再点击其中的 `Page A`
- THEN 1 秒内标签列表依次为 `Page B`、`Page A`，`Page A` 处于激活样式
