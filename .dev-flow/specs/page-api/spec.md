# page-api Specification

## Purpose
TaskSpace 内单个 Page 的动作，经 CdpMux 直接对 Chromium 发 CDP。

## Requirements

### Requirement: 导航与回执
系统 SHALL 在 `page.goto` 时把页面导航到给定 URL，在 `waitUntil`（默认 `load`）达成后返回回执 `{ok:true, durationMs, popups:[]}`，超过 `timeoutMs`（默认 30000）时返回 `E_TIMEOUT`。

#### Scenario: 打开本地测试页
- WHEN 经 `alter nodejs` 运行 `const p = (await taskSpace("t")).page("p1"); console.log(JSON.stringify(await p.goto("<server>/a"))); console.log(await p.title())`
- THEN 输出第一行 JSON 的 `ok` 为 `true`、`durationMs` 为非负整数，第二行为 `Page A`
- AND 退出码为 0

#### Scenario: 导航超时
- WHEN 脚本调用 `page.goto("<server>/delay", {timeoutMs: 500})`
- THEN 抛出的异常 `code` 为 `E_TIMEOUT`，脚本以退出码 1 结束

### Requirement: 无障碍快照
系统 SHALL 在 `page.snapshot` 时返回页面无障碍树的文本，每个可交互元素一行，形如 `- <role> "<name>" [@e<n>]`。

#### Scenario: 表单页快照
- WHEN `p1` 已打开 `/form`，脚本打印 `await page.snapshot()`
- THEN 输出包含一行匹配 `- textbox "Email" [@e\d+]` 和一行匹配 `- button "Submit" [@e\d+]`

### Requirement: 元素动作
系统 SHALL 按选择器（`@e<n>`、`text=`、`loc=css:`、`loc=role:`、`xpath=` 或原生 CSS）定位唯一元素并执行 `click`、`dblclick`、`hover`、`fill`、`focus`、`press`、`selectOption`、`dragAndDrop`、`setInputFiles`；匹配 0 个元素时返回 `E_NO_MATCH`，多于 1 个时返回 `E_AMBIGUOUS_SELECTOR`。

#### Scenario: 填表并提交
- WHEN `p1` 已打开 `/form`，脚本依次执行 `page.fill("loc=role:textbox[name=\"Email\"]", "a@b.c")` 与 `page.click("text=Submit")`，再打印 `await page.evaluate(() => document.body.innerText)`
- THEN 输出包含 `submitted: a@b.c`

#### Scenario: 用快照引用点击
- WHEN `p1` 已打开 `/form` 并取过快照，脚本 `fill` 快照中 `textbox "Email"` 的 `@e` 引用为 `x@y.z`，再 `click` `button "Submit"` 的 `@e` 引用
- THEN 页面正文包含 `submitted: x@y.z`

#### Scenario: 选择器匹配多个元素
- WHEN `p1` 已打开 `/two-buttons`，脚本执行 `page.click("text=Go")`
- THEN 异常 `code` 为 `E_AMBIGUOUS_SELECTOR`，`details.count` 为 2

#### Scenario: 选择器无匹配
- WHEN `p1` 已打开 `/form`，脚本执行 `page.click("text=Nope", {timeoutMs: 500})`
- THEN 异常 `code` 为 `E_NO_MATCH`

### Requirement: 对话框
系统 SHALL 在页面有未处理的 JS 对话框时，对该页面的元素动作返回 `E_DIALOG_OPEN`，并在 `page.dialog` 时按 `accept` 处理它。

#### Scenario: 先处理对话框再操作
- WHEN `p1` 打开 `/alert`（加载后弹出 `alert("hi")`），脚本执行 `page.click("body")`
- THEN 异常 `code` 为 `E_DIALOG_OPEN`，`details.message` 为 `hi`
- AND 脚本调用 `page.dialog({accept: true})` 后，再执行 `page.click("body")` 成功

### Requirement: 截图、执行脚本与等待
系统 SHALL 提供 `page.screenshot`（写到给定的绝对路径）、`page.evaluate`（在页面里执行函数并返回可序列化的结果）与 `page.waitFor`（等待 URL、选择器、函数或超时），等待超时返回 `E_TIMEOUT`。

#### Scenario: 截图
- WHEN `p1` 已打开 `/a`，脚本调用 `page.screenshot({path: "<临时目录>/a.png"})`
- THEN 返回值的 `width` 与 `height` 都大于 0，文件以 PNG 签名 `89 50 4E 47` 开头

#### Scenario: 相对路径被拒绝
- WHEN 脚本调用 `page.screenshot({path: "a.png"})`
- THEN 异常 `code` 为 `E_INVALID_ARGUMENT`

#### Scenario: evaluate 返回值
- WHEN 脚本打印 `await page.evaluate((x) => x + 1, 41)`
- THEN 输出为 `42`

#### Scenario: 等待选择器超时
- WHEN `p1` 已打开 `/a`，脚本调用 `page.waitFor({selector: "#never"}, {timeoutMs: 300})`
- THEN 异常 `code` 为 `E_TIMEOUT`

### Requirement: 弹窗与下载
系统 SHALL 把 agent 页面打开的新窗口作为同一 TaskSpace 的下一个 Page 记录在回执的 `popups` 里，并把 agent 页面触发的下载保存到下载目录后推送 `page.download` 事件。

#### Scenario: 点击新窗口链接
- WHEN `p1` 已打开 `/links`，脚本打印 `JSON.stringify(await page.click("text=Open B"))`
- THEN 回执的 `popups` 为 `[{label:"p2", …}]`
- AND `task.page("p2").url()` 为 `/b` 的地址

#### Scenario: 下载文件
- WHEN 环境变量 `ALTER_DOWNLOAD_DIR` 指向临时目录 D，`p1` 执行 `const d = page.waitForEvent("download"); await page.goto("<server>/download"); console.log((await d).path)`
- THEN 输出为 `D/report.txt`，该文件存在

### Requirement: CDP 透传
系统 SHALL 在 `page.cdp` 时把给定的 CDP 方法与参数发到该页面的会话并原样返回结果。

#### Scenario: 透传 Runtime.evaluate
- WHEN 脚本打印 `JSON.stringify(await page.cdp("Runtime.evaluate", {expression: "1+1", returnByValue: true}))`
- THEN 输出 JSON 的 `result.value` 为 2

### Requirement: CDP 转发端点
系统 SHALL 在 `cdpPort` 上为带有与 `gateway.json` 一致的 Bearer token 的请求转发 Chromium 的 CDP 发现接口与 WebSocket，供 Playwright `connectOverCDP` 使用。

#### Scenario: Playwright 连接
- WHEN app 当前 Space 打开了本地测试页 `/a`，测试用 `chromium.connectOverCDP("http://127.0.0.1:<cdpPort>", {headers: {Authorization: "Bearer <token>"}})` 连接
- THEN 连接成功，`browser.contexts()` 中某个页面的 `url()` 为 `/a` 的地址

#### Scenario: 缺少 token
- WHEN 不带 `Authorization` 头请求 `http://127.0.0.1:<cdpPort>/json/version`
- THEN 响应状态码为 401

### Requirement: 动作回执日志
系统 SHALL 为每个 `page.*` 调用写一条 JSON 日志，字段包含 `task_space_id`、`label`、`method`、`duration_ms`、`ok`、`error_code`。

#### Scenario: 成功与失败各一条
- WHEN 脚本依次执行一次成功的 `page.goto("<server>/a")` 和一次失败的 `page.click("text=Nope", {timeoutMs: 300})`
- THEN alterd 的 stderr 中有一行 `method` 为 `page.goto`、`ok` 为 `true` 的 JSON，和一行 `method` 为 `page.click`、`ok` 为 `false`、`error_code` 为 `E_NO_MATCH` 的 JSON
