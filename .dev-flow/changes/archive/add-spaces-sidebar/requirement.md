---
id: add-spaces-sidebar
title: 浏览器基础 UI：Shell Control 与侧边栏标签
type: feature
status: archived
revision: 4
approved_revision: 4
repos: [alter]
created: 2026-10-20
skip_specs: false
---

# 浏览器基础 UI：Shell Control 与侧边栏标签

<!-- 需求文档。正文用中文写；保留字保持英文："Spec Delta:"、"ADDED/MODIFIED/REMOVED/RENAMED Requirement:"、"Scenario:"、WHEN/THEN/AND、SHALL/MUST。语法见 references/formats.md -->

## 背景
骨架阶段的 app 只有一个显示 alterd 状态的窗口，既不能打开网页，也没有侧边栏（`apps/browser/src/renderer/src/App.tsx` 只渲染产品名和状态文字）。alter 的路线图里，agent 网关、命令栏、导入都要建立在「alterd 是 Space / 标签状态的唯一来源，Electron 是它的显示器和手」这条边界上（`infra/architecture.md`「分层与边界」）。这条边界目前还没有代码：Shell Control Channel、标签的落库与恢复、Electron 侧的页面视图都不存在。本需求先打通用户日常浏览的主线：一个默认 Space、侧边栏标签、地址输入、导航按钮、重启恢复。

路线图（2026-10-21 与用户确认）把原定的「浏览器基础 UI」拆成两个变更：本需求，以及之后的 add-space-management（Space 的新建、编辑、切换、删除，partition 隔离验证，固定标签，拖拽排序，快捷键）。

## 变更内容
- alterd 在 `ALTER_HOME/shell.sock` 上提供 Shell Control Channel（每行一个 JSON），连接后推送全量状态快照，状态变化后再广播。
- 首次启动自动建默认 Space「Personal」。
- 主窗口变为「侧边栏 + 内容区」：侧边栏顶部有后退 / 前进 / 刷新、地址输入框和「新标签」按钮，下面是当前 Space 的标签列表，底部显示 Space 名称和 alterd 状态。
- 地址输入：带协议的 URL 原样打开；`example.com` 这类域名补 `https://`；`localhost` 和 IP 补 `http://`；其余内容用 Google 搜索。
- 标签可新建、在当前标签打开、激活、关闭（关闭即归档，不硬删）；标题和 favicon 回写到侧边栏与数据库。
- 重启 app 后按原顺序恢复标签与激活标签；只加载激活标签，其余首次激活时再加载。
- Chromium 的用户数据目录改为 `ALTER_HOME/chromium`，每个 Space 的页面用 partition `persist:space-<spaceId>`。

## 范围
- 范围内：上面「变更内容」列出的全部；协议类型在 `crates/alter-core` 定义并导出 TS（`ErrorCode` 新增 `E_TAB_NOT_FOUND`、`E_INVALID_ARGUMENT`，新增 `ShellCommand`、`ShellEvent` 等类型）；对应的 Rust 契约测试与 Electron 端到端测试。
- 范围外：
  - Space 的新建 / 编辑 / 切换 / 删除、partition 隔离验证、固定标签、拖拽排序、快捷键（add-space-management）。
  - Cmd+T 命令栏、分屏、Peek、自动归档与归档列表的 UI（add-command-bar-split）。
  - agent 网关、CDP、AgentOverlay（add-agent-gateway / add-agent-handoff）；Shell Control 里给 agent 用的 `createTab` / `setOverlay` 等指令由那些需求再加。
  - 浏览历史写入 `history` 表、书签、下载、多窗口。
  - alterd 断开后的自动重连：alterd 退出时页面显示「alterd 已退出」，与现状一致。

## 用户故事
1. 作为用户，我希望在侧边栏输入网址或搜索词就能打开页面，以便把 alter 当日常浏览器用。（P1）
2. 作为用户，我希望侧边栏列出我打开的标签，可以切换和关闭，并看到每个页面的标题。（P1）
3. 作为用户，我希望关掉 app 再打开时，标签还按原样在那里。（P1）
4. 作为用户，我希望能后退、前进和刷新当前页面。（P2）
5. 作为后续需求的开发者，我希望 Space / 标签状态只由 alterd 维护，并有一个有契约测试的通道通知 Electron，以便 agent 网关接入同一份状态。（P1）

## Spec Delta: shell-control
alterd 与 Electron Main 之间的本地控制通道：alterd 下发状态与指令，Electron 上报用户事件。

### ADDED Requirement: R1 连接即收到全量快照
系统 SHALL 在 `ALTER_HOME/shell.sock` 上接受本机连接，并在每个连接建立后立即向它发送一条 `shell.stateChanged` 全量状态快照。

#### Scenario: R1.S1 新连接收到快照
- WHEN 以空的临时目录为 `ALTER_HOME` 运行 `alterd run`，等 `alterd.json` 出现后用 Unix socket 连接 `ALTER_HOME/shell.sock`
- THEN 1 秒内收到一行 JSON，`type` 为 `shell.stateChanged`
- AND 其中 `spaces` 恰好 1 项，`name` 为 `Personal`，`color` 为 `#6366f1`，`activeSpaceId` 等于该项的 `id`
- AND `tabs` 为空数组

#### Scenario: R1.S2 socket 只允许当前用户访问
- WHEN alterd 已启动且 `ALTER_HOME/alterd.json` 已出现
- THEN `ALTER_HOME/shell.sock` 的权限位为 `0600`

### ADDED Requirement: R2 事件生效后广播快照
系统 SHALL 在接受一个 Electron 上报的事件并改变状态后，向所有已连接的客户端各发送一条新的 `shell.stateChanged` 快照；某个连接积压的未送达快照超过 256 份时，可以跳过其中最旧的，但最后发布的那份快照一定送达。

#### Scenario: R2.S1 两个连接都收到变化
- WHEN 客户端 A 与 B 都已连接并读完初始快照，A 发送 `{"type":"shell.event.openRequested","spaceId":"<Personal 的 id>","input":"https://example.com/a"}`
- THEN A 与 B 在 1 秒内各收到一条 `shell.stateChanged`，其 `tabs` 恰好 1 项，`url` 为 `https://example.com/a`

#### Scenario: R2.S2 无法解析的行被丢弃且连接保持
- WHEN 已连接的客户端先发送一行 `not json`，再发送一条合法的 `shell.event.openRequested`（`input` 为 `https://example.com/b`）
- THEN 连接没有被关闭
- AND 客户端收到的下一条 `shell.stateChanged` 中有 `url` 为 `https://example.com/b` 的标签

#### Scenario: R2.S4 慢连接积压时仍收到最新快照
- WHEN 客户端 B 已连接并读完初始快照后停止读取，客户端 A 依次发送 300 条 `openRequested`（`input` 为 `https://example.com/0` … `https://example.com/299`）并读完自己的 300 份快照，然后 B 开始读取
- THEN B 的连接没有被关闭
- AND B 在 2 秒内读到一份 `tabs` 恰好 300 项的快照

#### Scenario: R2.S3 被拒绝的事件只回错误给发送方
- WHEN 客户端 A 与 B 都已连接，A 发送 `openRequested`，其 `spaceId` 为 `01JZZZZZZZZZZZZZZZZZZZZZZZ`（不存在）
- THEN A 在 1 秒内收到 `{"type":"shell.error","code":"E_SPACE_NOT_FOUND",…}`
- AND B 在 1 秒内没有收到任何消息

### ADDED Requirement: R3 socket 路径过长时启动失败
系统 SHALL 在 `ALTER_HOME/shell.sock` 的路径超过 macOS Unix socket 路径上限（103 字节）时，以退出码 1 退出，不写 `alterd.json`。

#### Scenario: R3.S1 路径过长
- WHEN `ALTER_HOME` 是一个路径长度为 120 字节的空目录，运行 `alterd run`
- THEN alterd 在 2 秒内退出，退出码为 1
- AND stderr 中有一行 JSON 日志带 `code` 为 1，且消息包含 `shell.sock`
- AND `ALTER_HOME/alterd.json` 不存在

## Spec Delta: spaces
用户侧的工作区：一组标签与独立的 Chromium partition。

### ADDED Requirement: R4 首次启动建默认 Space
系统 SHALL 在数据库里没有任何未归档的 Space 时，建立一个名为 `Personal`、颜色为 `#6366f1` 的 Space。

#### Scenario: R4.S1 重启不重复创建
- WHEN 以同一个临时 `ALTER_HOME` 先后两次运行 `alterd run`（第一次以 SIGTERM 结束），第二次启动后连接 `shell.sock`
- THEN 快照中的 `spaces` 恰好 1 项，`name` 为 `Personal`，`id` 与第一次启动时快照里的相同

#### Scenario: R4.S2 侧边栏显示默认 Space
- WHEN 以空的临时目录为 `ALTER_HOME` 启动 app
- THEN 3 秒内侧边栏底部的 Space 名称显示 `Personal`
- AND 标签列表为空，内容区显示 `没有打开的标签`

## Spec Delta: tabs
Space 内的页面：打开、导航、激活、关闭与恢复。

### ADDED Requirement: R5 地址输入规范化
系统 SHALL 按以下规则把地址输入转换为 URL：去掉首尾空白后，以 `<scheme>://`、`about:` 或 `blob:` 开头的原样使用；不含空白且形如「主机名（含点号）[:端口][/路径]」的补 `https://`；主机为 `localhost` 或 IPv4 地址的补 `http://`；其余作为 `https://www.google.com/search?q=` 加 URL 编码后的输入。

#### Scenario: R5.S1 带协议的 URL 原样打开
- WHEN 客户端发送 `openRequested`，`input` 为 `  https://example.com/a?x=1  `
- THEN 新快照里新增标签的 `url` 为 `https://example.com/a?x=1`

#### Scenario: R5.S2 裸域名补 https
- WHEN 客户端发送 `openRequested`，`input` 为 `example.com/docs`
- THEN 新增标签的 `url` 为 `https://example.com/docs`

#### Scenario: R5.S3 localhost 与 IP 补 http
- WHEN 客户端依次发送 `input` 为 `localhost:8080` 与 `127.0.0.1:3000/x` 的两条 `openRequested`
- THEN 两个新增标签的 `url` 依次为 `http://localhost:8080` 与 `http://127.0.0.1:3000/x`

#### Scenario: R5.S4 其余内容走 Google 搜索
- WHEN 客户端发送 `openRequested`，`input` 为 `rust 生命周期`
- THEN 新增标签的 `url` 为 `https://www.google.com/search?q=rust%20%E7%94%9F%E5%91%BD%E5%91%A8%E6%9C%9F`

#### Scenario: R5.S6 blob URL 原样打开
- WHEN 客户端发送 `openRequested`，`input` 为 `blob:https://example.com/0f6c1e2a-5b7d-4c3e-9a1f-2d8e7b6c5a40`
- THEN 新增标签的 `url` 为 `blob:https://example.com/0f6c1e2a-5b7d-4c3e-9a1f-2d8e7b6c5a40`

#### Scenario: R5.S5 空输入被拒绝
- WHEN 客户端发送 `openRequested`，`input` 为 `   `
- THEN 客户端收到 `shell.error`，`code` 为 `E_INVALID_ARGUMENT`
- AND 之后 1 秒内没有收到 `shell.stateChanged`

### ADDED Requirement: R6 新建标签
系统 SHALL 在用户以「新标签」方式提交地址时，在当前 Space 的标签列表末尾新建一个标签、把它设为激活标签，并在内容区加载它的 URL。

#### Scenario: R6.S1 从侧边栏新建标签
- WHEN app 已启动、本地测试服务器在 `127.0.0.1:<port>` 提供标题为 `Page A` 的 `/a`，用户点击「新标签」，在地址框输入 `127.0.0.1:<port>/a` 并回车
- THEN 2 秒内侧边栏标签列表恰好 1 行且处于激活样式
- AND 内容区页面的 URL 为 `http://127.0.0.1:<port>/a`

### ADDED Requirement: R7 在当前标签打开
系统 SHALL 在用户不以「新标签」方式提交地址时，在当前激活标签里打开该 URL，不新建标签；没有激活标签时按新建标签处理。

#### Scenario: R7.S1 当前标签导航
- WHEN 当前 Space 有一个激活标签正显示 `http://127.0.0.1:<port>/a`，用户在地址框输入 `127.0.0.1:<port>/b` 并回车
- THEN 2 秒内该标签的页面 URL 为 `http://127.0.0.1:<port>/b`
- AND 侧边栏标签列表仍然恰好 1 行

#### Scenario: R7.S2 指定标签时下发导航指令
- WHEN 客户端发送 `openRequested`，带上已存在标签的 `tabId`，`input` 为 `example.com`
- THEN 客户端收到 `{"type":"shell.navigate","tabId":"<该 tabId>","url":"https://example.com"}`
- AND 快照中的标签数量不变

### ADDED Requirement: R8 标题回写
系统 SHALL 在页面标题变化后 2 秒内，在侧边栏对应的标签行显示新标题，并把它写入该标签的数据库记录。

#### Scenario: R8.S1 标题显示并落库
- WHEN 用户新建标签打开标题为 `Page A` 的本地测试页
- THEN 2 秒内侧边栏该行显示 `Page A`
- AND `ALTER_HOME/alter.db` 中该标签的 `tabs.title` 为 `Page A`

### ADDED Requirement: R9 激活标签
系统 SHALL 在用户点击侧边栏中的某个标签后，让内容区显示该标签的页面，并把它记为所在 Space 的激活标签。

#### Scenario: R9.S1 点击切换
- WHEN 当前 Space 依次新建了标题为 `Page A` 与 `Page B` 的两个标签（`Page B` 激活），用户点击 `Page A` 那一行
- THEN 1 秒内内容区页面的 URL 为 `/a` 的地址，`Page A` 那一行处于激活样式
- AND 新快照中 `Page A` 的标签 `active` 为 `true`、`Page B` 的为 `false`

### ADDED Requirement: R10 关闭标签
系统 SHALL 在用户关闭一个标签后把它从侧边栏移除并标记为已归档；若关闭的是激活标签，则激活它下方相邻的标签，没有下方标签时激活上方相邻的标签。

#### Scenario: R10.S1 关闭中间的激活标签
- WHEN 当前 Space 有按顺序排列的标签 A、B、C，B 为激活标签，客户端发送 `{"type":"shell.event.tabClosed","tabId":"<B 的 id>"}`
- THEN 新快照的 `tabs` 依次为 A、C，C 的 `active` 为 `true`
- AND 数据库中 B 的 `tabs.archived_at` 不为 NULL

#### Scenario: R10.S2 关闭最后一个标签
- WHEN app 中当前 Space 只有一个标签，用户点击该行的关闭按钮
- THEN 1 秒内侧边栏标签列表为空，内容区显示 `没有打开的标签`

### ADDED Requirement: R11 重启恢复标签
系统 SHALL 在 app 重启后按关闭前的顺序显示各标签与原激活标签，并且只为激活标签加载页面，其余标签在首次被激活时才加载。

#### Scenario: R11.S1 顺序与激活标签恢复
- WHEN 同一个临时 `ALTER_HOME` 下 app 依次打开本地测试页 `/a`、`/b`、`/c` 三个标签，点击 `/b` 那一行，正常退出后再次启动 app
- THEN 3 秒内侧边栏标签依次显示 `Page A`、`Page B`、`Page C`，`Page B` 处于激活样式
- AND 内容区页面的 URL 为 `/b` 的地址

#### Scenario: R11.S2 未激活的标签不预先加载
- WHEN 上一个场景重启后的 app 中，查询主进程的全部 webContents 的 URL
- THEN 包含 `/b` 的地址，不包含 `/a` 与 `/c` 的地址
- AND 用户点击 `Page C` 那一行后 2 秒内，内容区页面的 URL 为 `/c` 的地址

### ADDED Requirement: R14 网页弹窗在当前 Space 新建标签
系统 SHALL 在页面经 `target="_blank"` 链接或 `window.open` 请求打开新窗口时，不创建新的窗口，而是在该页面所属 Space 的标签列表末尾新建并激活一个打开该 URL 的标签。

#### Scenario: R14.S1 点击新窗口链接
- WHEN 激活标签显示本地测试页 `/popup`（页面里有一个 `target="_blank"`、指向 `/b` 的链接），用户点击该链接
- THEN 2 秒内侧边栏标签列表依次为 `Popup`、`Page B`，`Page B` 处于激活样式
- AND 主进程的 `BrowserWindow.getAllWindows()` 仍然只有 1 个窗口

#### Scenario: R14.S2 window.open 打开 blob 页面
- WHEN 激活标签显示本地测试页 `/blob`，页面执行 `window.open(URL.createObjectURL(new Blob(["<title>Blob</title>"], {type: "text/html"})))`
- THEN 2 秒内侧边栏标签列表依次为 `Blob Opener`、`Blob`，`Blob` 处于激活样式

## Spec Delta: app-shell

### ADDED Requirement: R12 后退、前进与刷新
系统 SHALL 在侧边栏提供后退、前进、刷新三个按钮，作用于当前激活标签；没有可后退或可前进的历史时，对应按钮不可点击。

#### Scenario: R12.S1 后退与前进
- WHEN 激活标签先打开本地测试页 `/a`，再在当前标签打开 `/b`
- THEN 前进按钮不可点击
- AND 点击后退后 2 秒内页面 URL 为 `/a` 的地址，前进按钮可点击
- AND 再点击前进后 2 秒内页面 URL 为 `/b` 的地址

#### Scenario: R12.S2 刷新
- WHEN 激活标签显示本地测试页 `/count`（服务器记录它被请求的次数，当前为 1），用户点击刷新
- THEN 2 秒内服务器记录的 `/count` 请求次数为 2

## Spec Delta: protocol

### MODIFIED Requirement: R13 协议类型导出
系统 SHALL 把 Rust 里标记为导出的协议类型生成为 TypeScript 声明文件写入指定目录。

#### Scenario: R13.S1 导出错误码与健康文件类型
- WHEN 执行 `cargo run -p alter-core --bin export-ts -- <空的临时目录>`
- THEN 退出码为 0
- AND 目录中存在 `ErrorCode.ts`，其类型是字符串字面量联合，恰好包含 `E_UNAUTHORIZED`、`E_PROTOCOL_VERSION`、`E_SPACE_NOT_FOUND`、`E_TASK_SPACE_NOT_FOUND`、`E_PAGE_NOT_FOUND`、`E_PAGE_BUDGET`、`E_USER_CONTROL`、`E_AMBIGUOUS_SELECTOR`、`E_NO_MATCH`、`E_TIMEOUT`、`E_DIALOG_OPEN`、`E_IMPORT_SOURCE_NOT_FOUND`、`E_KEYCHAIN_DENIED`、`E_INTERNAL`、`E_TAB_NOT_FOUND`、`E_INVALID_ARGUMENT` 这 16 个字面量
- AND 目录中存在 `HealthFile.ts`，其字段恰好为 `version`、`pid`、`startedAt`、`dbPath`

#### Scenario: R13.S2 导出 Shell Control 消息类型
- WHEN 执行 `cargo run -p alter-core --bin export-ts -- <空的临时目录>`
- THEN 目录中存在 `ShellCommand.ts`，其 `type` 字段的字符串字面量恰好为 `shell.stateChanged`、`shell.navigate`、`shell.error`
- AND 目录中存在 `ShellEvent.ts`，其 `type` 字段的字符串字面量恰好为 `shell.event.openRequested`、`shell.event.tabUpdated`、`shell.event.tabActivated`、`shell.event.tabClosed`

## 非功能需求
- 安全：`shell.sock` 权限 0600，只监听本机 Unix socket；Electron Main 不打开数据库、不发 CDP；Renderer 只经 preload 暴露的 IPC 与 Main 通信（`contextIsolation`、`sandbox` 保持开启）；标签页面的 WebContentsView 不加载 preload，不开启 Node 集成。
- 性能：宪章预算里的「侧边栏 500 标签切换 Space < 50 ms」属于 add-space-management；本需求守住通道本身的开销（SC1）与重启恢复的可交互时间（SC2）。
- 可观测性：alterd 对每个被拒绝的事件记一条 WARN 日志，带 `code` 与事件 `type`；无法解析的行记 WARN 并附前 200 个字符。
- 兼容性：只支持 macOS；Electron 44 的 `WebContentsView`。
- 数据：关闭标签只置 `tabs.archived_at`，不删除行（`infra/data-model.md`「规则」）；本需求不改表结构，不加迁移。

## 成功标准
- SC1: 500 个标签时，alterd 从收到 `openRequested` 到发出新快照的耗时 — measure: 100 次的 p95 < 10 ms — check: `cargo test -p alterd --release --test shell_perf -- --ignored --nocapture`
- SC2: 重启恢复 100 个标签时，侧边栏 100 行全部渲染完成 — measure: 从 `daemon.status` 变为 running 起 < 1000 ms — check: e2e 用例 `SC2 重启恢复 100 个标签`（`bun run e2e --grep SC2`）

## 假设
- 激活标签不另存字段，取每个 Space 中未归档且 `last_active_at` 最大的标签：激活即更新 `last_active_at`，不需要迁移。
- 本需求里只有默认 Space 一个，`activeSpaceId` 取未归档 Space 中 `position` 最小的那个；激活 Space 的持久化由 add-space-management 负责。
- 「新标签」按钮让地址框进入新标签模式，回车后按新建标签处理；Cmd+T 留给命令栏需求。
- 侧边栏宽度固定 240 px，本需求不支持拖动调整。
- 关闭的标签按 Arc 的做法进入归档（`archived_at`），归档列表的 UI 留给自动归档需求。
- 网页弹窗一律转成新标签：`window.open` 返回的窗口对象不可用（打开方拿不到新页面的引用），依赖 opener 通信的页面（部分 OAuth 弹窗登录）在本需求里不支持，留给后续需求。

## 待确认问题
- 无。

## 澄清记录
### Session 2026-10-20
- Q: 「app + 网页 + skills」怎么拆？→ A: 按依赖拆成路线图；本次先做浏览器基础 UI。
- Q: 基础 UI 的附加范围？→ A: 重启恢复标签、固定标签、拖拽排序、常用快捷键都要。
- Q: Space 管理做到哪一步？→ A: 完整增删改查（删除时清理 partition 数据）。
- Q: 非 URL 输入用哪个搜索引擎？→ A: Google。
### Session 2026-10-21
- Q: 拆出来后约 19 条需求，要不要拆成两个变更？→ A: 拆。本需求做 Shell Control、默认 Space、标签主线、导航按钮、重启恢复；Space 管理、partition 隔离、固定、拖拽、快捷键放到 add-space-management。
- Q: 评审发现网页 `target=_blank` / `window.open` 会弹出不受管的独立窗口，怎么处理？→ A: 在同一 Space 新建标签（经 alterd），与 Arc 一致。
- Q: 评审第 2 轮：慢连接积压超过广播容量时会丢旧快照，怎么处理？→ A: 修订 R2，写明积压超过 256 份时可跳过最旧的，但最新快照一定送达，并补慢连接场景。
- Q: 评审第 2 轮：弹窗的 `blob:` URL 被当成搜索词，怎么修？→ A: R5 增加 `blob:` 原样使用（`data:` 不放行，避免钓鱼），补 R5.S6 与 R14.S2。
- Q: 执行 T1 前发现 protocol 现行规格要求 `ErrorCode.ts`「恰好 14 个字面量」，而设计要新增两个错误码与 Shell 消息类型，怎么处理？→ A: 走 update，加 protocol 的 MODIFIED 需求（16 个字面量）与 Shell 类型导出场景。

## 变更记录
- 2026-10-20: 创建。
- 2026-10-21: amended: 补上 protocol 能力的 MODIFIED「协议类型导出」（错误码从 14 个变为 16 个）与新场景 R13.S2（导出 ShellCommand / ShellEvent），T1 的 covers 加 R13.S1、R13.S2；原因是执行 T1 前发现现行规格写死了 14 个错误码，审批时漏掉; revision: 2; re-approved by the user
- 2026-10-21: amended: 新增需求 R14「网页弹窗在当前 Space 新建标签」与场景 R14.S1、任务 T18；原因是评审第 1 轮 CR2 发现弹窗会绕过 TabService 创建独立窗口，用户选择转为新标签; revision: 3; re-approved by the user
- 2026-10-21: amended: R2 正文写明慢连接积压超过 256 份时可跳过最旧快照、最新快照必达，新增 R2.S4；R5 正文增加 `blob:` 原样使用，新增 R5.S6；新增 R14.S2（blob 弹窗）；任务 T19–T22。原因是评审第 2 轮 CR3（剩余）与 CR9; revision: 4; re-approved by the user
