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
系统 SHALL 在用户关闭一个标签后把它从侧边栏移除并标记为已归档；若关闭的是激活标签，则激活它下方相邻的标签，没有下方标签时激活上方相邻的标签。

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
