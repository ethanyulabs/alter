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
| RPC | `hello` | 握手，交换版本与预算 | `{clientVersion, protocolVersion}` | `{protocolVersion, pageBudget, spaces:[{id,name}]}` | `E_PROTOCOL_VERSION` |
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
| RPC | `import.run` | 导入 Chrome / Arc profile 的书签、最近 90 天历史与 Cookie，顺序固定为书签、历史、Cookie（省略 `profilePath` 用数据根目录下的 `Default`；省略 `items` 为书签与历史，Cookie 要显式指定，因为它会弹 Keychain 授权框） | `{source:"chrome"\|"arc", profilePath?, spaceId, items?:["bookmarks"\|"history"\|"cookies"]}` | `{importId}`，之后 `import.progress` / `import.done` 只推给发起连接 | `E_IMPORT_SOURCE_NOT_FOUND`、`E_SPACE_NOT_FOUND`、`E_INVALID_ARGUMENT` |
| RPC | `import.status` | 导入的状态与计数（本次运行内存里的为准，`skipped` 重启后报 0；上次退出时未完成的为 `failed` / `interrupted`） | `{importId}` | `{status:"running"\|"done"\|"partial"\|"failed", counts:{bookmarks,history,cookies,skipped}, warning?, error?}`；`partial` 时 `warning` 为 `keychain_denied`（拒绝授权或没有这一项）或 `shell_unavailable`（没有持有 Chromium 的 Shell 连接，或 10 秒内没回报）；`cookies` 为 Main 回报写入的数，`skipped` 含解密失败、主机名前缀不符与 Main 拒绝的，过期的不计 | `E_INVALID_ARGUMENT`（没有这个导入） |
| HTTP / WS | `http://127.0.0.1:<cdpPort>/…` | CDP 转发端点，供 Playwright `connectOverCDP`：读请求头校验 `Authorization: Bearer <token>` 后把字节原样转给 Chromium（发现接口与 WebSocket 都这样）；有 TaskSpace 由用户控制时回 423，接管时断开已有连接；Chromium 未就绪回 503 | — | 标准 CDP 发现 JSON / CDP WebSocket | 401、423、502、503 |

`receipt` 结构：`{ok, durationMs, popups:[{label,targetId}], dialog?:{type,message}}`。它只描述已派发的动作和立即观察到的弹窗，不代表页面最终状态。

Shell Control Channel（alterd ↔ Electron Main，Unix socket `ALTER_HOME/shell.sock`，权限 0600，alterd 为服务端，每行一个 JSON 对象，`type` 区分消息，字段 camelCase；Rust 类型 `alter_core::protocol::{ShellCommand, ShellEvent}`，TS 类型由 `packages/protocol` 生成）。add-spaces-sidebar 与 add-core-features 已实现：

| 方向 | 消息 | 用途 | 字段 |
|---|---|---|---|
| alterd → Electron | `shell.stateChanged` | 全量快照：连接建立时发一份，之后每个改变状态的事件发一份（连接积压超过 256 份时跳过最旧的，最新一份必达） | `{activeSpaceId, spaces:[{id,name,color}], tabs:[{id,spaceId,url,title,faviconUrl?,active,pinned,discarded,taskSpaceId?,label?,splitGroup?}], taskSpaces:[{id,spaceId,name,ownership,status}], archived:[{id,spaceId,url,title,archivedAt}]}`（每个 Space 最近 50 个归档） |
| alterd → Electron | `shell.navigate` | 在已有标签打开 URL，只发给请求方 | `{tabId, url}` |
| alterd → Electron | `shell.error` | 事件被拒绝，只发给请求方 | `{code, message}` |
| alterd → Electron | `shell.discard` | 关闭固定标签：只卸载页面，只发给请求方 | `{tabId}` |
| alterd → Electron | `shell.searchResults` | 命令栏搜索结果，只发给请求方 | `{requestId, items:[{kind:"tab"\|"space"\|"bookmark"\|"history"\|"open", title, url?, tabId?, spaceId?, input?}]}`（标签、Space、当前 Space 的书签、历史，合计至多 8 项，最后附「打开或搜索」；空输入为空数组） |
| alterd → Electron | `shell.importSources` | 可导入的 profile，只发给请求方 | `{requestId, chrome:[name], arc:[name]}` |
| alterd → Electron | `shell.importCookies` | 把解密后的 Cookie 写进 Space 的 partition：只发给最近一次发 `cdpReady` 的连接，每批至多 500 项，等回报后再发下一批，每批前确认 Space 还在 | `{requestId, spaceId, cookies:[{url, name, value, domain, path, secure, httpOnly, sameSite:"unspecified"\|"no_restriction"\|"lax"\|"strict", expirationDate?}]}`（字段与 Electron `CookiesSetDetails` 一致；`domain` 不以 `.` 开头的 host-only Cookie，Main 不传 `domain`；会话 Cookie 没有 `expirationDate`） |
| alterd → Electron | `shell.importProgress` | 导入进度，只发给发起连接；最后一条的 `status` 为 done / partial / failed；没能开始时只有一条 failed（`importId` 0，`error` 为 `<错误码>: <说明>`） | `{requestId, importId, status, item?, counts, warning?, error?}` |
| Electron → alterd | `shell.event.openRequested` | 地址输入或网页弹窗：不带 `tabId` 在该 Space 末尾新建并激活标签；带 `tabId` 回 `shell.navigate`；来源是 agent 页面的弹窗记为同一 TaskSpace 的下一个 Page。输入规范化：带 `<scheme>://`、`about:`、`blob:` 原样；`localhost` / IPv4 补 `http://`；「主机名[:端口][/路径]」补 `https://`；其余走 Google 搜索 | `{spaceId, input, tabId?, openerTabId?}`；错误 `E_SPACE_NOT_FOUND`、`E_TAB_NOT_FOUND`、`E_INVALID_ARGUMENT`（空输入） |
| Electron → alterd | `shell.event.tabUpdated` | 页面状态回写（只认主框架导航，主框架导航写一条历史），值有变化才广播；视图建好后上报一次 CDP targetId（只记在内存） | `{tabId, url?, title?, faviconUrl?, targetId?}`；错误 `E_TAB_NOT_FOUND` |
| Electron → alterd | `shell.event.tabActivated` | 激活标签（`last_active_at` 取同 Space 最大值 + 1 与当前时间中较大者） | `{tabId}`；错误 `E_TAB_NOT_FOUND` |
| Electron → alterd | `shell.event.tabClosed` | 关闭即归档（固定标签只卸载）；关闭的是激活标签时激活下方相邻的，没有则上方；解散它所在的分屏 | `{tabId}`；错误 `E_TAB_NOT_FOUND` |
| Electron → alterd | `shell.event.spaceCreated` / `spaceUpdated` / `spaceActivated` / `spaceDeleted` | Space 管理：新建（名称 1–32 字符，颜色为预设之一）并设为当前；只改给出的字段；切换；硬删（不能删最后一个，partition 目录在下次启动时删除） | `{name, color}` / `{spaceId, name?, color?}` / `{spaceId}` / `{spaceId}` |
| Electron → alterd | `shell.event.tabPinned` / `tabMoved` | 固定 / 取消固定；移到目标分区第 `index` 位 | `{tabId, pinned}` / `{tabId, pinned, index}` |
| Electron → alterd | `shell.event.cdpReady` | Chromium remote-debugging 端口就绪，alterd 据此连接 CdpMux | `{port}` |
| Electron → alterd | `shell.event.ownershipReturned` / `userTookControl` | 用户点「交还给 Agent」/「停止 Agent」 | `{taskSpaceId}` |
| Electron → alterd | `shell.event.searchRequested` | 命令栏输入变化 | `{requestId, query}` |
| Electron → alterd | `shell.event.splitRequested` / `splitDissolved` | 同一 Space 的两个标签组成分屏（左边是 `tabIds[0]`）/ 解散 | `{tabIds:[a,b]}` / `{tabId}` |
| Electron → alterd | `shell.event.tabRestored` | 恢复已归档标签到普通分区末尾并激活 | `{tabId}` |
| Electron → alterd | `shell.event.importSourcesRequested` / `importRequested` | 导入向导：列出 profile / 开始导入（`profile` 是列表里的子目录名） | `{requestId}` / `{requestId, source, profile, spaceId, items}` |
| Electron → alterd | `shell.event.cookiesImported` | 回报一批 Cookie 的写入结果；只接受来自收到该批的连接、且 `imported + rejected` 等于批大小的，其余记 WARN 丢弃 | `{requestId, imported, rejected}` |

快照 `shell.stateChanged` 另有可选字段 `updateAvailable: {version, url}`（alterd 发现新版本后出现，没有更新时没有这个字段），侧边栏据此显示更新提示条。

无法解析的行与未知 `type` 记 WARN 后丢弃，连接保持。

CLI（`alter`，除 `status` 外的子命令读同一个 `gateway.json`；`ALTER_HOME` 覆盖数据目录以便测试）：

| 子命令 | 用途 | 退出码 |
|---|---|---|
| `alter nodejs [-e <code>]`（stdin heredoc 或 `-e`） | 用打包的 Bun 运行脚本，预置 `taskSpace`、`takeOverTaskSpace`、`claimTaskSpace`、`listTaskSpaces` 全局函数；收到 `app.updateAvailable` 时 stderr 打一行 `[alter:notice] alter <v> is available, run: alter upgrade` | 脚本退出码；无法连接 2 |
| `alter spaces list [--json]` | 列出 Space | 0 |
| `alter import --from chrome\|arc [--profile <path>] --space <id> [--items bookmarks,history,cookies]` | 导入并等待 `import.done`，stdout 打印 `imported bookmarks=<n> history=<n> cookies=<n> skipped=<n>`；`partial` 时 stderr 打出 warning，退出码仍为 0；省略 `--items` 为书签与历史；`--profile` 的相对路径按当前目录转成绝对路径 | 0；来源不存在或其他业务错误 1；连不上 Gateway 2 |
| `alter status [--json]` | alterd 是否运行与版本（读 `alterd.json`，格式见下文「进程状态」）；Gateway 端口与最近错误尚未加入 | 运行中 0，未运行或残留 1 |
| `alter upgrade [--check [--json]] [--version v<x.y.z>]` | `--check` 打印 `alter <latest> is available (current <current>)` 或 `alter <current> is up to date`（`--json`：`{current, latest, updateAvailable}`）；升级：下载 latest（或 `--version`）的 install.sh 并带 `--version --prefix <包所在目录> --bin-dir <链接所在目录>` 运行，成功后刷新带 `.alter-skill` 的 skill，最后一行 `upgraded to <v>`；只升不降，除非显式 `--version`；不是从 app 包运行时拒绝 | 0 成功或已是最新；1 取不到版本、不是安装包、install.sh 失败（原安装不动）；`--json` 不带 `--check` 为 2 |
| `alter skill install [--target <dir>]…` / `alter skill uninstall [--target <dir>]…` | 把包内（开发时仓库里）的 `skills/alter-browser` 复制到 `~/.claude/skills`、`~/.codex/skills` 中已存在的目录或 `--target`，经暂存目录原子替换，写 `.alter-skill`（CLI 版本），每个目标一行 `installed alter-browser -> <path>`；uninstall 只删带标记的，输出 `removed <path>` | install：0；没有目标、找不到源、源与目标相同或复制失败 1。uninstall：0 |
| `alter --version` | 版本 | 0 |

进程状态（add-workspace-skeleton、fix-orphan-alterd 已实现，三个进程共用 `ALTER_HOME`，未设置时为 `~/Library/Application Support/alter`）：

- 健康文件 `alterd.json`：alterd 写，CLI 与 Electron Main 读；`{"version": "0.1.0", "pid": 12345, "startedAt": 1760918400000, "dbPath": "/abs/path/alter.db"}`，先写临时文件再 rename。Rust 类型 `alter_core::protocol::HealthFile`，TS 类型由 `packages/protocol` 生成。
- `alterd run [--exit-on-stdin-close]`：带上该参数时，stdin 读到 EOF 或读取出错就按 SIGTERM 的流程退出（删除 `alterd.json`，退出码 0，日志 `signal` 为 `stdin-eof` / `stdin-error`）；不带时 stdin 不影响运行。Electron Main 总是带上它，并以自己独占写端的管道作 alterd 的 stdin。
- `alterd run` 退出码：0 正常退出（SIGTERM / SIGINT / stdin 关闭）；3 已有实例持有 `alterd.lock`；1 其他错误。stderr 只输出 tracing 的 JSON 行，退出码 1 与 3 的那条带 `code` 字段。
- `alter status`：运行中输出 `alterd running (pid <n>, version <v>)`；无文件输出 `not running`；文件在但 pid 已死输出 `not running (stale)`。`--json` 为 `{"running": true, "pid": n, "version": "…", "dbPath": "…"}`、`{"running": false}` 或 `{"running": false, "stale": true}`。
- Electron IPC（Main ↔ Renderer，经 preload 的 `window.alter`）：`shell.state`（Main 推送 / Renderer invoke，载荷 `{state: ShellState|null, nav: {canGoBack, canGoForward, url}, error?: {code, message}}`）、`shell.open`（`{input, newTab}`）、`tab.activate` / `tab.close`（`{tabId}`）、`nav.back` / `nav.forward` / `nav.reload`（`{}`），定义在 `apps/browser/src/shared/shell.ts`。add-core-features 另加：`space.create` / `space.update` / `space.activate` / `space.delete`；`tab.pin` / `tab.move` / `tab.split` / `tab.unsplit` / `tab.restore`；`taskSpace.return` / `taskSpace.stop`（`{taskSpaceId}`）；`commandBar.open` / `commandBar.close` / `commandBar.search {requestId, query}` / `commandBar.results {requestId, items}`（Main → Renderer）/ `commandBar.execute {item}`；`peek.state {open, bounds?}`（Main → Renderer）/ `peek.close` / `peek.promote`；`address.focus {newTab}`（Main → Renderer）；`import.sources`（invoke）/ `import.start` / `import.progress`（Main → Renderer）。通道 `daemon.status`，载荷 `{state: "starting"|"running"|"missing"|"exited", pid?, path?}`；alterd 以退出码 3 退出且 `alterd.json` 里的 pid 存活时，报 `running` 与该外部实例的 pid；Renderer 显示 `alterd 启动中 / 运行中 / 未找到 / 已退出`。

SDK（`packages/sdk`，Bun 内全局可用）：`taskSpace(name|id)`、`takeOverTaskSpace(id)`、`claimTaskSpace(id)`、`listTaskSpaces()`；TaskSpace 对象：`spaceId`、`name`、`ownership`、`page(label)`、`userPage()`、`pages()`、`tabs()`、`newPage()`、`adopt()`、`release()`、`handOff()`、`waitForControl()`、`finish({keep})`、`cdp()`；Page 对象一一对应上表 `page.*` 方法，外加 `mouse.*`、`keyboard.*`、`waitForEvent("popup"|"download")`（`waitForFileChooser()` 推迟到后续 change）。与 ego-browser 的 API 形状保持一致，方便 skill 迁移。

## 事件 / 消息
Gateway 用 JSON-RPC 通知（无 id）推送：

| 主题 | 生产者 | 消费者 | 消息体 |
|---|---|---|---|
| `taskSpace.ownershipChanged` | alterd | SDK（`waitForControl`）、Renderer | `{id, ownership, reason:"handOff"\|"userTookControl"\|"returned"}` |
| `page.dialog` | alterd | SDK | `{id, label, type, message}` |
| `page.popup` | alterd | SDK | `{id, label, newLabel, targetId}` |
| `page.download` | alterd | SDK | `{id, label, url, suggestedFilename, path}` |
| `import.progress` / `import.done` | alterd | 发起导入的连接（CLI 等）；向导经 Shell 的 `shell.importProgress` | `import.progress {importId, item, counts}`；`import.done {importId, status, counts, warning?}` |
| `app.updateAvailable` | alterd | SDK（`alter nodejs` 打 `[alter:notice]`）；侧边栏经 Shell 快照的 `updateAvailable` | `{version, url}`（`url` 为 Release 页，必须是 https）；当时已连接的客户端与之后每个新连接在鉴权后各收到一次，同一版本一次运行只推一次 |

安装脚本 `scripts/install.sh`（Release 资产之一，安装命令 `curl -fsSL https://github.com/ethanyulabs/alter/releases/latest/download/install.sh | bash`）：参数 `--version v<x.y.z>`、`--prefix <dir>`（默认 `/Applications`，不可写时 `~/Applications`）、`--bin-dir <dir>`（默认 `~/.local/bin`）、`--with-skill`、`--dry-run`（每步打印 `[dry-run] <步骤>`）、`-h`；环境变量 `ALTER_RELEASE_API`、`ALTER_RELEASE_BASE`。只支持 macOS arm64；SHA256 不符、下载失败、运行中的 alter 10 秒内不退出时退出码 1 且不改动安装；未知参数 2；stdout 最后一行 `alter <v> installed to <prefix>/alter.app`。Release 资产命名：`alter-<v>-arm64.dmg`、`alter-<v>-arm64.dmg.sha256`、`install.sh`。
