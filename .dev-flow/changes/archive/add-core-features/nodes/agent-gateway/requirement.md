---
id: add-core-features
node: N2
title: agent 网关、CDP 与脚本运行时
repos: [alter]
revision: 1
approved_revision: 1
---

# N2 agent 网关、CDP 与脚本运行时

<!-- 流程路线型需求中一个节点（子流程）的需求文档。保留字保持英文。语法见 references/formats.md §3 和 §14 -->

## 范围
- 交付：
  - alterd 的 Gateway：loopback WebSocket JSON-RPC，token 鉴权，`gateway.json`。
  - TaskSpaceManager：TaskSpace / Page 的生命周期、page budget、每个 TaskSpace 一个串行命令队列。
  - CdpMux：连接 Electron 内 Chromium 的 remote-debugging 端口，实现 `page.*` 原语与事件。
  - CDP 转发端点，供 Playwright `connectOverCDP`。
  - `@alter/sdk`（Bun 内运行的 TaskSpace / Page API，形状与 ego-browser 一致）。
  - CLI 子命令 `alter nodejs` 与 `alter spaces list`。
  - 交给下游：
    - N3：Gateway 分发器与事件推送，命令队列，快照里的 `taskSpaces`。
    - N5：CdpMux 按 Space 写 Cookie 的接口。
    - N6：SDK 的公开 API。
- 依赖：N1 的 Space 语义（当前 Space、标签的 `position` 规则）与迁移 0002 后的表结构。
- 不做：ownership 切换与接管（N3），导入（N5）。ownership 字段在本节点固定为 `agent`。

术语：「本地测试页」指 e2e 测试服务器 `apps/browser/e2e/fixtures/server.ts` 提供的页面；`/form` 页面有一个 label 为 `Email` 的输入框、一个 `Submit` 按钮，提交后正文显示 `submitted: <输入值>`；`/two-buttons` 有两个文字都是 `Go` 的按钮；`/delay` 在 5 秒后才响应；`/links` 有一个 `target="_blank"` 指向 `/b` 的链接 `Open B`；`/download` 响应头为 `Content-Disposition: attachment; filename=report.txt`；`/alert` 加载后立即 `alert("hi")`。

## Spec Delta: gateway
alterd 对 SDK 与 CLI 暴露的本机 WebSocket JSON-RPC 接口。

### ADDED Requirement: R1 启动时发布网关地址
系统 SHALL 在 alterd 启动后监听 127.0.0.1 上的随机端口，并把 `{port, token, cdpPort, version}` 写入权限为 0600 的 `ALTER_HOME/gateway.json`，退出时删除该文件。

#### Scenario: R1.S1 写出 gateway.json
- WHEN 以空的临时目录为 `ALTER_HOME` 运行 `alterd run`
- THEN `ALTER_HOME/alterd.json` 出现时 `ALTER_HOME/gateway.json` 也已存在，权限位为 `0600`
- AND 其中 `token` 是 64 个十六进制字符，`version` 为 `0.1.0`，`port` 与 `cdpPort` 都能在 `127.0.0.1` 上建立 TCP 连接

#### Scenario: R1.S2 退出时删除
- WHEN 已写出 `gateway.json` 的 `alterd run` 收到 SIGTERM
- THEN 进程退出后 `ALTER_HOME/gateway.json` 不存在

### ADDED Requirement: R2 连接鉴权
系统 SHALL 只接受 WebSocket 握手请求头带有 `Authorization: Bearer <gateway.json 中的 token>` 的连接，其余握手以 HTTP 401 拒绝。

#### Scenario: R2.S1 缺少 token
- WHEN 客户端不带 `Authorization` 头连接 `ws://127.0.0.1:<port>/rpc`
- THEN 握手响应状态码为 401

#### Scenario: R2.S2 token 错误
- WHEN 客户端带 `Authorization: Bearer 0000` 连接
- THEN 握手响应状态码为 401

#### Scenario: R2.S3 token 一致
- WHEN 客户端带与 `gateway.json` 一致的 token 连接并发送 `{"jsonrpc":"2.0","id":1,"method":"hello","params":{"clientVersion":"0.1.0","protocolVersion":1}}`
- THEN 收到 `id` 为 1 的响应，`result.protocolVersion` 为 1，`result.pageBudget` 为 8，`result.spaces` 恰好 1 项且 `name` 为 `Personal`

### ADDED Requirement: R3 协议版本与错误格式
系统 SHALL 以 JSON-RPC 2.0 错误返回每个失败的调用，`error.data` 为 `{"code":"E_…","message":"<说明>"}`，并拒绝协议版本不是 1 的 `hello`。

#### Scenario: R3.S1 协议版本不匹配
- WHEN 已鉴权的客户端发送 `hello`，`protocolVersion` 为 2
- THEN 响应的 `error.data.code` 为 `E_PROTOCOL_VERSION`

#### Scenario: R3.S2 未知方法
- WHEN 已鉴权的客户端发送方法 `page.teleport`
- THEN 响应的 `error.code` 为 -32601，`error.data.code` 为 `E_INVALID_ARGUMENT`

#### Scenario: R3.S3 参数缺失
- WHEN 已鉴权的客户端发送 `taskSpace.create`，`params` 为 `{}`
- THEN 响应的 `error.data.code` 为 `E_INVALID_ARGUMENT`，`error.data.message` 包含 `name`

### ADDED Requirement: R4 列出 Space
系统 SHALL 在 `space.list` 时返回全部 Space 的 `id`、`name`、`color` 与未归档标签数 `tabCount`，顺序与侧边栏相同。

#### Scenario: R4.S1 列出两个 Space
- WHEN alterd 有 Space `Personal`（2 个标签）与 `Work`（0 个标签），客户端调用 `space.list`
- THEN `result.spaces` 依次为 `{name:"Personal", tabCount:2}`、`{name:"Work", tabCount:0}`（另含 `id`、`color`）

## Spec Delta: task-space
agent 的任务空间：挂在某个 Space 下，拥有若干带永久标签（`p1`、`p2`…）的 Page。

### ADDED Requirement: R5 创建 TaskSpace
系统 SHALL 在 `taskSpace.create` 时在指定 Space（省略时为当前 Space）下建一个 ownership 为 `agent` 的 TaskSpace，并在该 Space 标签列表末尾为它新建一个不抢激活状态的空白页面 `p1`。

#### Scenario: R5.S1 在当前 Space 创建
- WHEN 当前 Space 为 `Personal`，其中有激活标签 A，客户端调用 `taskSpace.create {"name":"demo"}`
- THEN 响应为 `{id:<整数>, spaceId:<Personal 的 id>, ownership:"agent", pages:["p1"]}`
- AND 新快照的标签依次为 A、新标签，新标签的 `url` 为 `about:blank`、`taskSpaceId` 等于响应的 `id`、`label` 为 `p1`，A 的 `active` 仍为 `true`

#### Scenario: R5.S2 Space 不存在
- WHEN 客户端调用 `taskSpace.create {"name":"demo","spaceId":"01JZZZZZZZZZZZZZZZZZZZZZZZ"}`
- THEN `error.data.code` 为 `E_SPACE_NOT_FOUND`

#### Scenario: R5.S3 侧边栏显示 agent 页面
- WHEN app 当前 Space 有激活标签 `Page A`，经 `alter nodejs` 运行 `await taskSpace("demo")`
- THEN 2 秒内侧边栏多出一行，带 `data-testid="agent-badge"` 标记并显示 `p1`，`Page A` 仍处于激活样式
- AND 主进程里存在一个 URL 为 `about:blank` 的 webContents

### ADDED Requirement: R6 恢复与列出 TaskSpace
系统 SHALL 在 `taskSpace.resume` 时按 id 返回已存在 TaskSpace 的当前 Page 标签，在 `taskSpace.list` 时返回全部 TaskSpace。

#### Scenario: R6.S1 跨连接恢复
- WHEN 客户端 A 调用 `taskSpace.create {"name":"demo"}` 得到 id 7 并断开，客户端 B 调用 `taskSpace.resume {"id":7}`
- THEN 响应为 `{id:7, spaceId:<同一 Space>, ownership:"agent", pages:["p1"]}`

#### Scenario: R6.S2 TaskSpace 不存在
- WHEN 客户端调用 `taskSpace.resume {"id":999}`
- THEN `error.data.code` 为 `E_TASK_SPACE_NOT_FOUND`

#### Scenario: R6.S3 列出
- WHEN 已创建名为 `a` 与 `b` 的两个 TaskSpace，客户端调用 `taskSpace.list`
- THEN `result.taskSpaces` 依次包含 `name` 为 `a` 与 `b` 的两项，`status` 都为 `active`

### ADDED Requirement: R7 新开 Page 与页面预算
系统 SHALL 在 `taskSpace.newPage` 时为 TaskSpace 新建下一个编号的空白页面，同一 TaskSpace 未关闭的页面数达到 page budget（默认 8）时拒绝。

#### Scenario: R7.S1 编号递增
- WHEN TaskSpace 已有 `p1`，客户端调用 `taskSpace.newPage {"id":<id>}`
- THEN 响应为 `{label:"p2"}`

#### Scenario: R7.S2 超出预算
- WHEN TaskSpace 已有 `p1`…`p8` 共 8 个未关闭的页面，客户端调用 `taskSpace.newPage`
- THEN `error.data.code` 为 `E_PAGE_BUDGET`，`error.data.details` 为 `{"budget":8,"open":8}`

#### Scenario: R7.S3 关闭后标签不复用
- WHEN TaskSpace 有 `p1`、`p2`，客户端调用 `page.close {"id":<id>,"label":"p2"}` 后再调用 `taskSpace.newPage`
- THEN 响应为 `{label:"p3"}`

### ADDED Requirement: R8 托管与归还用户标签
系统 SHALL 在 `taskSpace.adopt` 时把同一 Space 中未被任何 TaskSpace 托管的用户标签纳入该 TaskSpace 并分配下一个 Page 标签，在 `taskSpace.release` 时解除托管但保留标签本身。

#### Scenario: R8.S1 托管与列出
- WHEN Space `Personal` 有用户标签 A，TaskSpace 已有 `p1`，客户端调用 `taskSpace.adopt {"id":<id>,"tabId":"<A 的 id>"}`，再调用 `taskSpace.tabs {"id":<id>}`
- THEN `adopt` 的响应为 `{label:"p2"}`
- AND `tabs` 的结果包含 `{tabId:<A 的 id>, label:"p2", openedBy:"user"}` 与 `label` 为 `p1`、`openedBy` 为 `agent` 的一项

#### Scenario: R8.S2 已被另一个 TaskSpace 托管
- WHEN 标签 A 已被 TaskSpace X 托管，客户端对 TaskSpace Y 调用 `adopt` A
- THEN `error.data.code` 为 `E_INVALID_ARGUMENT`

#### Scenario: R8.S3 跨 Space 托管被拒
- WHEN TaskSpace 属于 `Personal`，标签 B 属于 `Work`，客户端调用 `adopt` B
- THEN `error.data.code` 为 `E_TAB_NOT_FOUND`

#### Scenario: R8.S4 归还
- WHEN TaskSpace 以 `p2` 托管着用户标签 A，客户端调用 `taskSpace.release {"id":<id>,"label":"p2"}`
- THEN 之后 `taskSpace.tabs` 中 A 那一项没有 `label`，快照里 A 仍在且没有 `taskSpaceId`

### ADDED Requirement: R9 结束 TaskSpace
系统 SHALL 在 `taskSpace.finish` 时关闭该 TaskSpace 中由 agent 打开且不在 `keep` 列表里的页面，解除其余页面的托管，并把状态设为 `finished`。

#### Scenario: R9.S1 保留指定页面
- WHEN TaskSpace 有 agent 打开的 `p1`、`p2` 和托管的用户标签 `p3`，客户端调用 `taskSpace.finish {"id":<id>,"keep":["p2"]}`
- THEN 响应为 `{closed:["p1"], kept:["p2","p3"]}`
- AND 快照中 `p1` 的标签已不在，`p2` 与 `p3` 的标签仍在且没有 `taskSpaceId`；`taskSpace.list` 中该项 `status` 为 `finished`

### ADDED Requirement: R10 页面被用户关闭
系统 SHALL 在 TaskSpace 的页面对应的标签被用户关闭后，对该 Page 标签的后续 `page.*` 调用返回 `E_PAGE_NOT_FOUND`。

#### Scenario: R10.S1 用户关掉 agent 页面
- WHEN TaskSpace 的 `p1` 对应标签 T，Electron 发送 `shell.event.tabClosed`（T 的 id），然后客户端调用 `page.info {"id":<id>,"label":"p1"}`
- THEN `error.data.code` 为 `E_PAGE_NOT_FOUND`

## Spec Delta: page-api
TaskSpace 内单个 Page 的动作，经 CdpMux 直接对 Chromium 发 CDP。

### ADDED Requirement: R11 导航与回执
系统 SHALL 在 `page.goto` 时把页面导航到给定 URL，在 `waitUntil`（默认 `load`）达成后返回回执 `{ok:true, durationMs, popups:[]}`，超过 `timeoutMs`（默认 30000）时返回 `E_TIMEOUT`。

#### Scenario: R11.S1 打开本地测试页
- WHEN 经 `alter nodejs` 运行 `const p = (await taskSpace("t")).page("p1"); console.log(JSON.stringify(await p.goto("<server>/a"))); console.log(await p.title())`
- THEN 输出第一行 JSON 的 `ok` 为 `true`、`durationMs` 为非负整数，第二行为 `Page A`
- AND 退出码为 0

#### Scenario: R11.S2 导航超时
- WHEN 脚本调用 `page.goto("<server>/delay", {timeoutMs: 500})`
- THEN 抛出的异常 `code` 为 `E_TIMEOUT`，脚本以退出码 1 结束

### ADDED Requirement: R12 无障碍快照
系统 SHALL 在 `page.snapshot` 时返回页面无障碍树的文本，每个可交互元素一行，形如 `- <role> "<name>" [@e<n>]`。

#### Scenario: R12.S1 表单页快照
- WHEN `p1` 已打开 `/form`，脚本打印 `await page.snapshot()`
- THEN 输出包含一行匹配 `- textbox "Email" [@e\d+]` 和一行匹配 `- button "Submit" [@e\d+]`

### ADDED Requirement: R13 元素动作
系统 SHALL 按选择器（`@e<n>`、`text=`、`loc=css:`、`loc=role:`、`xpath=` 或原生 CSS）定位唯一元素并执行 `click`、`dblclick`、`hover`、`fill`、`focus`、`press`、`selectOption`、`dragAndDrop`、`setInputFiles`；匹配 0 个元素时返回 `E_NO_MATCH`，多于 1 个时返回 `E_AMBIGUOUS_SELECTOR`。

#### Scenario: R13.S1 填表并提交
- WHEN `p1` 已打开 `/form`，脚本依次执行 `page.fill("loc=role:textbox[name=\"Email\"]", "a@b.c")` 与 `page.click("text=Submit")`，再打印 `await page.evaluate(() => document.body.innerText)`
- THEN 输出包含 `submitted: a@b.c`

#### Scenario: R13.S2 用快照引用点击
- WHEN `p1` 已打开 `/form` 并取过快照，脚本 `fill` 快照中 `textbox "Email"` 的 `@e` 引用为 `x@y.z`，再 `click` `button "Submit"` 的 `@e` 引用
- THEN 页面正文包含 `submitted: x@y.z`

#### Scenario: R13.S3 选择器匹配多个元素
- WHEN `p1` 已打开 `/two-buttons`，脚本执行 `page.click("text=Go")`
- THEN 异常 `code` 为 `E_AMBIGUOUS_SELECTOR`，`details.count` 为 2

#### Scenario: R13.S4 选择器无匹配
- WHEN `p1` 已打开 `/form`，脚本执行 `page.click("text=Nope", {timeoutMs: 500})`
- THEN 异常 `code` 为 `E_NO_MATCH`

### ADDED Requirement: R14 对话框
系统 SHALL 在页面有未处理的 JS 对话框时，对该页面的元素动作返回 `E_DIALOG_OPEN`，并在 `page.dialog` 时按 `accept` 处理它。

#### Scenario: R14.S1 先处理对话框再操作
- WHEN `p1` 打开 `/alert`（加载后弹出 `alert("hi")`），脚本执行 `page.click("body")`
- THEN 异常 `code` 为 `E_DIALOG_OPEN`，`details.message` 为 `hi`
- AND 脚本调用 `page.dialog({accept: true})` 后，再执行 `page.click("body")` 成功

### ADDED Requirement: R15 截图、执行脚本与等待
系统 SHALL 提供 `page.screenshot`（写到给定的绝对路径）、`page.evaluate`（在页面里执行函数并返回可序列化的结果）与 `page.waitFor`（等待 URL、选择器、函数或超时），等待超时返回 `E_TIMEOUT`。

#### Scenario: R15.S1 截图
- WHEN `p1` 已打开 `/a`，脚本调用 `page.screenshot({path: "<临时目录>/a.png"})`
- THEN 返回值的 `width` 与 `height` 都大于 0，文件以 PNG 签名 `89 50 4E 47` 开头

#### Scenario: R15.S2 相对路径被拒绝
- WHEN 脚本调用 `page.screenshot({path: "a.png"})`
- THEN 异常 `code` 为 `E_INVALID_ARGUMENT`

#### Scenario: R15.S3 evaluate 返回值
- WHEN 脚本打印 `await page.evaluate((x) => x + 1, 41)`
- THEN 输出为 `42`

#### Scenario: R15.S4 等待选择器超时
- WHEN `p1` 已打开 `/a`，脚本调用 `page.waitFor({selector: "#never"}, {timeoutMs: 300})`
- THEN 异常 `code` 为 `E_TIMEOUT`

### ADDED Requirement: R16 弹窗与下载
系统 SHALL 把 agent 页面打开的新窗口作为同一 TaskSpace 的下一个 Page 记录在回执的 `popups` 里，并把 agent 页面触发的下载保存到下载目录后推送 `page.download` 事件。

#### Scenario: R16.S1 点击新窗口链接
- WHEN `p1` 已打开 `/links`，脚本打印 `JSON.stringify(await page.click("text=Open B"))`
- THEN 回执的 `popups` 为 `[{label:"p2", …}]`
- AND `task.page("p2").url()` 为 `/b` 的地址

#### Scenario: R16.S2 下载文件
- WHEN 环境变量 `ALTER_DOWNLOAD_DIR` 指向临时目录 D，`p1` 执行 `const d = page.waitForEvent("download"); await page.goto("<server>/download"); console.log((await d).path)`
- THEN 输出为 `D/report.txt`，该文件存在

### ADDED Requirement: R17 CDP 透传
系统 SHALL 在 `page.cdp` 时把给定的 CDP 方法与参数发到该页面的会话并原样返回结果。

#### Scenario: R17.S1 透传 Runtime.evaluate
- WHEN 脚本打印 `JSON.stringify(await page.cdp("Runtime.evaluate", {expression: "1+1", returnByValue: true}))`
- THEN 输出 JSON 的 `result.value` 为 2

### ADDED Requirement: R18 CDP 转发端点
系统 SHALL 在 `cdpPort` 上为带有与 `gateway.json` 一致的 Bearer token 的请求转发 Chromium 的 CDP 发现接口与 WebSocket，供 Playwright `connectOverCDP` 使用。

#### Scenario: R18.S1 Playwright 连接
- WHEN app 当前 Space 打开了本地测试页 `/a`，测试用 `chromium.connectOverCDP("http://127.0.0.1:<cdpPort>", {headers: {Authorization: "Bearer <token>"}})` 连接
- THEN 连接成功，`browser.contexts()` 中某个页面的 `url()` 为 `/a` 的地址

#### Scenario: R18.S2 缺少 token
- WHEN 不带 `Authorization` 头请求 `http://127.0.0.1:<cdpPort>/json/version`
- THEN 响应状态码为 401

### ADDED Requirement: R19 动作回执日志
系统 SHALL 为每个 `page.*` 调用写一条 JSON 日志，字段包含 `task_space_id`、`label`、`method`、`duration_ms`、`ok`、`error_code`。

#### Scenario: R19.S1 成功与失败各一条
- WHEN 脚本依次执行一次成功的 `page.goto("<server>/a")` 和一次失败的 `page.click("text=Nope", {timeoutMs: 300})`
- THEN alterd 的 stderr 中有一行 `method` 为 `page.goto`、`ok` 为 `true` 的 JSON，和一行 `method` 为 `page.click`、`ok` 为 `false`、`error_code` 为 `E_NO_MATCH` 的 JSON

## Spec Delta: sdk
`@alter/sdk`：在 Bun 里运行的 TaskSpace / Page API。

### ADDED Requirement: R20 SDK 请求与错误映射
系统 SHALL 让 SDK 的每个方法发出与 Gateway 契约一致的 JSON-RPC 请求，并把错误响应转换为带 `code`、`message`、`details` 的 `AlterError` 异常，且不自动重试。

#### Scenario: R20.S1 taskSpace 按名称创建、按数字恢复
- WHEN 对一个记录请求的假 Gateway 调用 `taskSpace("demo")` 和 `taskSpace(7)`
- THEN 假 Gateway 依次收到 `taskSpace.create {"name":"demo"}` 与 `taskSpace.resume {"id":7}`

#### Scenario: R20.S2 错误映射且不重试
- WHEN 假 Gateway 对 `page.click` 返回 `error.data` 为 `{"code":"E_USER_CONTROL","message":"user has control"}`，SDK 调用 `page.click("text=Go")`
- THEN 抛出的异常是 `AlterError`，`code` 为 `E_USER_CONTROL`
- AND 假 Gateway 恰好收到 1 次 `page.click` 请求

#### Scenario: R20.S3 连接失败
- WHEN `gateway.json` 指向一个没有监听的端口，调用 `taskSpace("demo")`
- THEN 抛出的 `AlterError` 的 `code` 为 `E_INTERNAL`，`message` 包含 `alter.app`

## Spec Delta: cli

### ADDED Requirement: R21 alter nodejs
系统 SHALL 在 `alter nodejs` 时用 Bun 运行 `-e` 参数或 stdin 中的脚本，预置全局函数 `taskSpace`、`takeOverTaskSpace`、`claimTaskSpace`、`listTaskSpaces`，退出码与脚本相同；找不到 `gateway.json` 时以退出码 2 结束。

#### Scenario: R21.S1 -e 运行脚本
- WHEN app 在运行，执行 `alter nodejs -e 'const t = await taskSpace("demo"); console.log(t.page("p1").label)'`
- THEN stdout 为一行 `p1`，退出码为 0

#### Scenario: R21.S2 从 stdin 读脚本
- WHEN app 在运行，以 heredoc 向 `alter nodejs` 的 stdin 写入 `console.log((await listTaskSpaces()).length >= 0)`
- THEN stdout 为一行 `true`，退出码为 0

#### Scenario: R21.S3 脚本抛出异常
- WHEN 执行 `alter nodejs -e 'throw new Error("boom")'`
- THEN 退出码为 1，stderr 包含 `boom`

#### Scenario: R21.S4 alterd 未运行
- WHEN `ALTER_HOME` 指向一个没有 `gateway.json` 的临时目录，执行 `alter nodejs -e '1'`
- THEN 退出码为 2，stderr 包含 `alter.app`

### ADDED Requirement: R22 alter spaces list
系统 SHALL 在 `alter spaces list` 时经 Gateway 打印全部 Space，每行 `<id>  <name>  <tabCount> tabs`；带 `--json` 时打印 `space.list` 的结果 JSON。

#### Scenario: R22.S1 文本与 JSON 输出
- WHEN alterd 只有 `Personal`（0 个标签），执行 `alter spaces list` 与 `alter spaces list --json`
- THEN 第一个命令的 stdout 恰好一行，包含 `Personal` 与 `0 tabs`，退出码为 0
- AND 第二个命令的 stdout 是一行 JSON，`spaces[0].name` 为 `Personal`

## Spec Delta: protocol

### MODIFIED Requirement: 协议类型导出
系统 SHALL 把 Rust 里标记为导出的协议类型生成为 TypeScript 声明文件写入指定目录。

#### Scenario: R23.S1 导出错误码与健康文件类型
- WHEN 执行 `cargo run -p alter-core --bin export-ts -- <空的临时目录>`
- THEN 退出码为 0
- AND 目录中存在 `ErrorCode.ts`，其类型是字符串字面量联合，恰好包含 `E_UNAUTHORIZED`、`E_PROTOCOL_VERSION`、`E_SPACE_NOT_FOUND`、`E_TASK_SPACE_NOT_FOUND`、`E_PAGE_NOT_FOUND`、`E_PAGE_BUDGET`、`E_USER_CONTROL`、`E_AMBIGUOUS_SELECTOR`、`E_NO_MATCH`、`E_TIMEOUT`、`E_DIALOG_OPEN`、`E_IMPORT_SOURCE_NOT_FOUND`、`E_KEYCHAIN_DENIED`、`E_INTERNAL`、`E_TAB_NOT_FOUND`、`E_INVALID_ARGUMENT` 这 16 个字面量
- AND 目录中存在 `HealthFile.ts`，其字段恰好为 `version`、`pid`、`startedAt`、`dbPath`

#### Scenario: R23.S2 导出 Shell Control 消息类型
- WHEN 执行 `cargo run -p alter-core --bin export-ts -- <空的临时目录>`
- THEN 目录中存在 `ShellCommand.ts`，其 `type` 字段的字符串字面量恰好为 `shell.stateChanged`、`shell.navigate`、`shell.error`、`shell.discard`
- AND 目录中存在 `ShellEvent.ts`，其 `type` 字段的字符串字面量恰好为 `shell.event.openRequested`、`shell.event.tabUpdated`、`shell.event.tabActivated`、`shell.event.tabClosed`、`shell.event.spaceCreated`、`shell.event.spaceUpdated`、`shell.event.spaceActivated`、`shell.event.spaceDeleted`、`shell.event.tabPinned`、`shell.event.tabMoved`、`shell.event.cdpReady`
- AND 目录中存在 `TabView.ts`，其字段包含 `pinned`、`taskSpaceId`、`label`

#### Scenario: R23.S3 导出 Gateway 类型
- WHEN 执行 `cargo run -p alter-core --bin export-ts -- <空的临时目录>`
- THEN 目录中存在 `GatewayFile.ts`，其字段恰好为 `port`、`token`、`cdpPort`、`version`
- AND 目录中存在 `Receipt.ts`，其字段包含 `ok`、`durationMs`、`popups`

## 变更记录
- 2026-10-21: 创建。
