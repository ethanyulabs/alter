# API

<!-- 模块、仓库和客户端之间的契约。这里的变更需要契约测试（宪章第四条）。 -->

## 约定
- Base URL / 版本：Gateway 为 `ws://127.0.0.1:<port>/rpc`，端口随机，连同 token 写在 `~/Library/Application Support/alter/gateway.json`（`{"port", "token", "cdpPort", "version"}`）。协议为 JSON-RPC 2.0；`hello` 响应里带 `protocolVersion`（整数），SDK 版本不匹配时报 `E_PROTOCOL_VERSION`。
- 认证：WS 握手头 `Authorization: Bearer <token>`；缺失或错误时握手返回 401，连接内不再校验。
- 错误：JSON-RPC `error.data` 固定为 `{"code": "E_…", "message": "<人可读>", "details": {...}}`。错误码：`E_UNAUTHORIZED`、`E_PROTOCOL_VERSION`、`E_SPACE_NOT_FOUND`、`E_TASK_SPACE_NOT_FOUND`、`E_PAGE_NOT_FOUND`、`E_PAGE_BUDGET`、`E_USER_CONTROL`、`E_AMBIGUOUS_SELECTOR`、`E_NO_MATCH`、`E_TIMEOUT`、`E_DIALOG_OPEN`、`E_IMPORT_SOURCE_NOT_FOUND`、`E_KEYCHAIN_DENIED`、`E_INTERNAL`、`E_TAB_NOT_FOUND`、`E_INVALID_ARGUMENT`（后两个目前只用于 Shell Control）。
- 分页：无；列表接口返回全部（TaskSpace、Space 数量都很小）。
- 时间：全部 UTC 毫秒整数；时长字段名以 `Ms` 结尾。
- 标识：Space 和 Tab 用 ULID 字符串；TaskSpace 用自增整数（agent 跨轮次 resume 时好记）；Page 用标签字符串 `p1`、`p2`…。

## 接口
Gateway（JSON-RPC 方法）。`label` 是 Page 标签，`selector` 接受 `@ref` / `text=` / `loc=css:` / `loc=role:` / `xpath=` / 原生 CSS。

| 方法 | 路径 | 用途 | 请求 | 响应 | 错误 |
|---|---|---|---|---|---|
| RPC | `hello` | 握手，交换版本与预算 | `{clientVersion}` | `{protocolVersion, pageBudget, spaces:[{id,name}]}` | `E_PROTOCOL_VERSION` |
| RPC | `space.list` | 列出 Space | `{}` | `{spaces:[{id,name,color,tabCount,archived}]}` | — |
| RPC | `taskSpace.create` | 新建 TaskSpace，附带 `p1` | `{name, spaceId?, profileId?}` | `{id, spaceId, ownership:"agent", pages:["p1"]}` | `E_SPACE_NOT_FOUND` |
| RPC | `taskSpace.resume` | 恢复已有 TaskSpace | `{id}` | `{id, spaceId, ownership, pages:[…]}` | `E_TASK_SPACE_NOT_FOUND` |
| RPC | `taskSpace.list` | 列出 TaskSpace | `{}` | `{taskSpaces:[{id,name,spaceId,ownership,status}]}` | — |
| RPC | `taskSpace.newPage` | 新开空白 Page | `{id, as?}` | `{label}` | `E_PAGE_BUDGET`、`E_USER_CONTROL` |
| RPC | `taskSpace.tabs` | 该 TaskSpace 所在 Space 的全部标签（含未托管） | `{id}` | `{tabs:[{label?,tabId,title,url,active,openedBy}]}` | — |
| RPC | `taskSpace.adopt` / `taskSpace.release` | 托管一个未托管标签 / 归还给用户 | `{id, tabId, as?}` / `{id, label}` | `{label}` / `{}` | `E_PAGE_NOT_FOUND` |
| RPC | `taskSpace.handOff` | ownership → user | `{id}` | `{}` | — |
| RPC | `taskSpace.takeOver` | ownership → agent（用户已交还时） | `{id}` | `{ownership, userPage?}` | `E_USER_CONTROL`（用户尚未交还） |
| RPC | `taskSpace.finish` | 结束，关闭未列入 keep 的 agent 页面 | `{id, keep:[labels]}` | `{closed:[labels], kept:[labels]}` | — |
| RPC | `page.info` / `page.url` / `page.title` | 页面状态 | `{id, label}` | `{url,title,loading,dialog?}` | `E_PAGE_NOT_FOUND` |
| RPC | `page.goto` / `page.reload` | 导航 | `{id,label,url,waitUntil?,timeoutMs?}` | `receipt` | `E_TIMEOUT`、`E_USER_CONTROL` |
| RPC | `page.snapshot` | 无障碍树文本 | `{id,label,scope?:"viewport"\|"full_page"\|"subtree",root?}` | `{text, refs:n}` | — |
| RPC | `page.screenshot` | 截图到绝对路径 | `{id,label,path,fullPage?}` | `{path,width,height}` | — |
| RPC | `page.click` / `dblclick` / `hover` / `fill` / `focus` / `press` / `selectOption` / `dragAndDrop` / `setInputFiles` | 元素动作 | `{id,label,selector,value?,label?:"3-6 词动作描述"}` | `receipt` | `E_NO_MATCH`、`E_AMBIGUOUS_SELECTOR`、`E_DIALOG_OPEN`、`E_USER_CONTROL` |
| RPC | `page.mouse` / `page.keyboard` | 原始指针 / 键盘 | `{id,label,op,args}` | `receipt` | `E_USER_CONTROL` |
| RPC | `page.waitFor` | 等待 URL / selector / 函数 / 事件 / 超时 | `{id,label,kind,arg,timeoutMs?}` | `{matched}` | `E_TIMEOUT` |
| RPC | `page.evaluate` | 页内执行函数 | `{id,label,fn,arg?}` | `{value}` | `E_TIMEOUT` |
| RPC | `page.fetch` | 页内 `window.fetch` | `{id,label,url,options,saveAs?}` | `{ok,status,headers,body\|path}` | — |
| RPC | `page.dialog` | 处理 JS 对话框 | `{id,label,accept,promptText?}` | `{}` | `E_NO_MATCH`（无对话框） |
| RPC | `page.close` | 关闭 agent 创建的 Page | `{id,label}` | `{}` | — |
| RPC | `page.cdp` / `taskSpace.cdp` | 透传 CDP 命令（页面级 / Target、Browser 级） | `{id,label?,method,params}` | `{result}` | — |
| RPC | `import.run` | 导入 Chrome / Arc profile | `{source:"chrome"\|"arc", profilePath?, spaceId, items:[…]}` | `{importId}` + 事件 | `E_IMPORT_SOURCE_NOT_FOUND` |
| RPC | `import.status` | 导入进度 | `{importId}` | `{status, counts, error?}` | — |
| HTTP | `GET http://127.0.0.1:<cdpPort>/json/version` | CDP 转发端点，供 Playwright `connectOverCDP` | — | 标准 CDP 发现 JSON | — |

`receipt` 结构：`{ok, durationMs, popups:[{label,targetId}], dialog?:{type,message}}`。它只描述已派发的动作和立即观察到的弹窗，不代表页面最终状态。

Shell Control Channel（alterd ↔ Electron Main，Unix socket `ALTER_HOME/shell.sock`，权限 0600，alterd 为服务端，每行一个 JSON 对象，`type` 区分消息，字段 camelCase；Rust 类型 `alter_core::protocol::{ShellCommand, ShellEvent}`，TS 类型由 `packages/protocol` 生成）。已实现（add-spaces-sidebar）：

| 方向 | 消息 | 用途 | 字段 |
|---|---|---|---|
| alterd → Electron | `shell.stateChanged` | 全量快照：连接建立时发一份，之后每个改变状态的事件发一份（连接积压超过 256 份时跳过最旧的，最新一份必达） | `{activeSpaceId, spaces:[{id,name,color}], tabs:[{id,spaceId,url,title,faviconUrl,active}]}`（只含未归档的；Space 按 position，标签先按 Space 再按 position；每个 Space 至多一个 `active`） |
| alterd → Electron | `shell.navigate` | 在已有标签打开 URL，只发给请求方 | `{tabId, url}` |
| alterd → Electron | `shell.error` | 事件被拒绝，只发给请求方 | `{code, message}` |
| Electron → alterd | `shell.event.openRequested` | 地址输入或网页弹窗：不带 `tabId` 在该 Space 末尾新建并激活标签；带 `tabId` 回 `shell.navigate`。输入规范化：带 `<scheme>://`、`about:`、`blob:` 原样；`localhost` / IPv4 补 `http://`；「主机名[:端口][/路径]」补 `https://`；其余走 Google 搜索 | `{spaceId, input, tabId?}`；错误 `E_SPACE_NOT_FOUND`、`E_TAB_NOT_FOUND`、`E_INVALID_ARGUMENT`（空输入） |
| Electron → alterd | `shell.event.tabUpdated` | 页面状态回写（只认主框架导航），值有变化才广播 | `{tabId, url?, title?, faviconUrl?}`；错误 `E_TAB_NOT_FOUND` |
| Electron → alterd | `shell.event.tabActivated` | 激活标签（`last_active_at` 取同 Space 最大值 + 1 与当前时间中较大者） | `{tabId}`；错误 `E_TAB_NOT_FOUND` |
| Electron → alterd | `shell.event.tabClosed` | 关闭即归档；关闭的是激活标签时激活下方相邻的，没有则上方 | `{tabId}`；错误 `E_TAB_NOT_FOUND` |

无法解析的行与未知 `type` 记 WARN 后丢弃，连接保持。

规划中（由后续需求实现，届时再定字段）：Space 管理事件（add-space-management）；`shell.setLayout`（分屏）；agent 用的指定标签 / CDP target 对应与 `shell.setOverlay`（add-agent-gateway / add-agent-handoff）；`shell.event.userTookControl` / `ownershipReturned`；`shell.event.cdpReady`。

CLI（`alter`，除 `status` 外的子命令读同一个 `gateway.json`；`ALTER_HOME` 覆盖数据目录以便测试）：

| 子命令 | 用途 | 退出码 |
|---|---|---|
| `alter nodejs [-e <code>]`（stdin heredoc 或 `-e`） | 用打包的 Bun 运行脚本，预置 `taskSpace`、`takeOverTaskSpace`、`claimTaskSpace`、`listTaskSpaces` 全局函数 | 脚本退出码；无法连接 2 |
| `alter spaces list [--json]` | 列出 Space | 0 |
| `alter import --from chrome\|arc [--profile <path>] --space <id> [--items bookmarks,history,cookies]` | 导入并等待完成，打印计数 | 0；部分成功 0 并打印警告；来源不存在 1 |
| `alter status [--json]` | alterd 是否运行与版本（读 `alterd.json`，格式见下文「进程状态」）；端口与最近错误待网关需求补充 | 运行中 0，未运行或残留 1 |
| `alter upgrade [--check]` | 从 GitHub Releases 下载并安装新版 DMG | 0；无更新 0；下载失败 1 |
| `alter --version` | 版本 | 0 |

进程状态（add-workspace-skeleton、fix-orphan-alterd 已实现，三个进程共用 `ALTER_HOME`，未设置时为 `~/Library/Application Support/alter`）：

- 健康文件 `alterd.json`：alterd 写，CLI 与 Electron Main 读；`{"version": "0.1.0", "pid": 12345, "startedAt": 1760918400000, "dbPath": "/abs/path/alter.db"}`，先写临时文件再 rename。Rust 类型 `alter_core::protocol::HealthFile`，TS 类型由 `packages/protocol` 生成。
- `alterd run [--exit-on-stdin-close]`：带上该参数时，stdin 读到 EOF 或读取出错就按 SIGTERM 的流程退出（删除 `alterd.json`，退出码 0，日志 `signal` 为 `stdin-eof` / `stdin-error`）；不带时 stdin 不影响运行。Electron Main 总是带上它，并以自己独占写端的管道作 alterd 的 stdin。
- `alterd run` 退出码：0 正常退出（SIGTERM / SIGINT / stdin 关闭）；3 已有实例持有 `alterd.lock`；1 其他错误。stderr 只输出 tracing 的 JSON 行，退出码 1 与 3 的那条带 `code` 字段。
- `alter status`：运行中输出 `alterd running (pid <n>, version <v>)`；无文件输出 `not running`；文件在但 pid 已死输出 `not running (stale)`。`--json` 为 `{"running": true, "pid": n, "version": "…", "dbPath": "…"}`、`{"running": false}` 或 `{"running": false, "stale": true}`。
- Electron IPC（Main ↔ Renderer，经 preload 的 `window.alter`）：`shell.state`（Main 推送 / Renderer invoke，载荷 `{state: ShellState|null, nav: {canGoBack, canGoForward, url}, error?: {code, message}}`）、`shell.open`（`{input, newTab}`）、`tab.activate` / `tab.close`（`{tabId}`）、`nav.back` / `nav.forward` / `nav.reload`（`{}`），定义在 `apps/browser/src/shared/shell.ts`。通道 `daemon.status`，载荷 `{state: "starting"|"running"|"missing"|"exited", pid?, path?}`；alterd 以退出码 3 退出且 `alterd.json` 里的 pid 存活时，报 `running` 与该外部实例的 pid；Renderer 显示 `alterd 启动中 / 运行中 / 未找到 / 已退出`。

SDK（`packages/sdk`，Bun 内全局可用）：`taskSpace(name|id)`、`takeOverTaskSpace(id)`、`claimTaskSpace(id)`、`listTaskSpaces()`；TaskSpace 对象：`spaceId`、`name`、`ownership`、`page(label)`、`userPage()`、`pages()`、`tabs()`、`newPage()`、`adopt()`、`release()`、`handOff()`、`waitForControl()`、`finish({keep})`、`cdp()`；Page 对象一一对应上表 `page.*` 方法，外加 `mouse.*`、`keyboard.*`、`waitForEvent("popup"|"download")`、`waitForFileChooser()`。与 ego-browser 的 API 形状保持一致，方便 skill 迁移。

## 事件 / 消息
Gateway 用 JSON-RPC 通知（无 id）推送：

| 主题 | 生产者 | 消费者 | 消息体 |
|---|---|---|---|
| `taskSpace.ownershipChanged` | alterd | SDK（`waitForControl`）、Renderer | `{id, ownership, reason:"handOff"\|"userTookControl"\|"returned"}` |
| `page.dialog` | alterd | SDK | `{id, label, type, message}` |
| `page.popup` | alterd | SDK | `{id, label, newLabel, targetId}` |
| `page.download` | alterd | SDK | `{id, label, url, suggestedFilename, path}` |
| `import.progress` / `import.done` | alterd | Renderer、CLI | `{importId, status, counts, warning?}` |
| `app.updateAvailable` | alterd | Renderer、CLI（`[alter:notice]` 行） | `{version, url}` |
