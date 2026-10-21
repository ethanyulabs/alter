---
change: add-spaces-sidebar
---

# 执行文档：浏览器基础 UI：Shell Control 与侧边栏标签

<!-- 执行文档。每个任务是一个薄的垂直切片，能由一个测试单独证明。
行语法：- [ ] T<n> [covers:R1.S1] [after:T<k>] [P] <做什么> — seam: <测试驱动的公开入口> — verify: <命令>
有变更的场景用 RED/GREEN，已有行为用 BASELINE，没有 covers 的任务用 GREEN/VERIFY。
子条目写涉及的文件、跨任务必须遵守的约束、风险与检查项、并行时各自占用的文件和资源。
[P] 在执行前要先做独立性检查；共享的证据和任务勾选只由协调者记录。
对应的证据写进 log.md 之后才能勾选 [x]。完整语法见 references/formats.md -->

## Repo: alter

- [x] T1 [covers:R13.S1,R13.S2,R1.S1,R1.S2,R4.S1] 协议类型与错误码导出；alterd 监听 `shell.sock`，连接即收到含默认 Space 的快照 — seam: `export-ts` 二进制（R13.*）；`alterd` 二进制 + 假客户端（`std::os::unix::net::UnixStream` 按行读写，临时 `ALTER_HOME`） — verify: `cargo test -p alter-core --test export_ts && cargo test -p alterd --test shell`
  - 开工前：fix-orphan-alterd（PR #2）合并后把 `main` 合进本分支，它改了 `lifecycle.rs`、`daemon.ts` 并给 tokio 加了 `sync` feature；没合并就先停下来告诉用户。
  - 依赖：根 `Cargo.toml` 的 tokio 加 `net`、`io-util` feature；`[workspace.dependencies]` 加 `ulid = "3.0.0"`；alterd 的 `[dependencies]` 加 `ulid`、`serde_json`、`libc`（libc 从 dev 依赖提上来，只用于 umask）。
  - 协议：新文件 `crates/alter-core/src/protocol/shell.rs`，定义 `ShellCommand`（`#[serde(tag = "type")]`，变体 rename 成 `shell.stateChanged` / `shell.navigate` / `shell.error`，`rename_all_fields = "camelCase"`）、`ShellEvent`（四个 `shell.event.*`）、`ShellState`、`SpaceView`、`TabView`；在 `protocol/mod.rs` 的 `export_ts` 里导出，并扩展 `crates/alter-core/tests/export_ts.rs`。`ErrorCode` 加 `E_TAB_NOT_FOUND`、`E_INVALID_ARGUMENT`。跑 `bun run protocol:export` 更新 `packages/protocol/src/generated/`，不要手改生成文件。
  - R13.S1 / R13.S2 先做：改 `crates/alter-core/tests/export_ts.rs`（`ERROR_CODES` 改为 16 个，新增 R13.S2 的断言：从 `ShellCommand.ts` / `ShellEvent.ts` 里抽出 `type: "…"` 的字面量集合比较），verify 额外跑 `cargo test -p alter-core --test export_ts`。
  - 仓储：`crates/alter-core/src/store/spaces.rs`（`ensure_default`、`list`）、`store/tabs.rs`（`list_open`、后续任务再加写操作），写法参考 `store/mod.rs`；单测放 `crates/alter-core/tests/store.rs`（内存库或临时目录）。
  - alterd：新目录 `crates/alterd/src/shell/`（`UnixListener`；bind 前删除残留的 `shell.sock`，bind 前后用 `libc::umask(0o177)` 再恢复）、`crates/alterd/src/tabs/`（TabService 任务，独占 `Store`，经 `mpsc` 收事件、经 `broadcast` 发快照）。`lifecycle.rs` 在写健康文件之前启动它们（外部看到 `alterd.json` 时 socket 已可连），退出时删除 `shell.sock`。
  - 测试：新文件 `crates/alterd/tests/shell.rs`，`Daemon` 守卫与 `wait_until` 抄 `crates/alterd/tests/lifecycle.rs` 的写法（或抽到 `tests/common/mod.rs` 共用）。R1.S2 用 `std::fs::metadata(..).permissions().mode() & 0o777 == 0o600`。R4.S1 两次启动之间用 SIGTERM 正常结束第一次。
  - 坑：current_thread 运行时里 `Store`（rusqlite `Connection` 是 `Send` 不是 `Sync`）要整个 move 进 TabService 任务，不要跨任务共享引用。
- [x] T2 [covers:R5.S1,R5.S2,R5.S3,R5.S4,R5.S5] [after:T1] `openRequested` 新建标签，地址按规则规范化 — seam: `alterd` 二进制 + 假客户端 — verify: `cargo test -p alterd --test shell r5_`
  - 文件：`crates/alterd/src/tabs/url.rs`（`normalize_input`）、`crates/alterd/src/tabs/mod.rs`（处理 `openRequested` 无 `tabId` 的分支）、`crates/alter-core/src/store/tabs.rs`（`insert`：末尾 position，`last_active_at = max(now, 同 Space 最大值 + 1)`）。
  - 期望值直接用需求里的字面量（例如 R5.S4 的 `https://www.google.com/search?q=rust%20%E7%94%9F%E5%91%BD%E5%91%A8%E6%9C%9F`），不要在测试里调用实现去算。URL 编码按 RFC 3986 的非保留字符（`A-Z a-z 0-9 - . _ ~`）原样保留，其余按 UTF-8 字节 `%XX`；空格编成 `%20`。
  - 搜索模板先写成常量 `https://www.google.com/search?q=`；`config.toml` 可配置不在本需求范围。
  - 每个场景一个测试，按 S1 → S5 的顺序 RED / GREEN：S2、S3 的规则要等 S1 之后才写得出失败的测试。
- [x] T3 [covers:R2.S1,R2.S2,R2.S3,R3.S1] [after:T2] 多连接广播、坏行丢弃、错误只回发送方、socket 路径过长报错 — seam: `alterd` 二进制 + 两个假客户端 — verify: `cargo test -p alterd --test shell`
  - 文件：`crates/alterd/src/shell/`（每个连接一个读任务 + 一个写任务；写任务同时订阅 broadcast 与本连接的私有 mpsc，用 `tokio::select!`）。
  - R2.S3 断言 B「1 秒内没有收到任何消息」：B 的 socket 设 `set_read_timeout(1s)`，读到 `WouldBlock` / `TimedOut` 即通过。
  - R3.S1：用 `tempfile::Builder::new().prefix(&"x".repeat(n))` 构造总长 120 字节的目录；在 bind 之前检查路径长度（`> 103` 字节）并返回带 `shell.sock` 字样的错误，走 `main.rs` 现有的 `code = 1` 日志路径；健康文件此时还没写。
  - 坑：broadcast 的 `Lagged` 错误要处理成「直接读最新快照继续」，不能让连接任务退出。
- [x] T4 [covers:R4.S2] [after:T3] Electron 连接 Shell Control，渲染侧边栏骨架 — seam: Playwright `_electron.launch`（临时 `ALTER_HOME`） — verify: `bun run e2e --grep "R4\\.S2"`
  - 文件：`apps/browser/src/shared/shell.ts`（IPC 通道与载荷类型，写法参考 `shared/daemon.ts`；协议类型从 `@alter/protocol` 引入）、`src/main/shell-link.ts`（`net.createConnection`，按 `\n` 切行，`JSON.parse` 失败记日志丢弃）、`src/main/index.ts`（在 `app.whenReady()` 之前 `app.setPath("userData", join(alterHome(), "chromium"))`；`DaemonSupervisor` 推 running 后连接 ShellLink）、`src/preload/index.ts`、`src/renderer/src/{App,Sidebar}.tsx`。
  - 布局：侧边栏固定 240 px（常量放 `shared/shell.ts`，Main 计算视图边界也用它）；底部显示 Space 名称与原有的 alterd 状态文字（R7.* 的 e2e 依赖「alterd 运行中」这几个字，不要改文案）。
  - 新的 e2e 文件 `apps/browser/e2e/sidebar.spec.ts`，复用 `e2e/app.ts` 的 `launchApp`、`tempHome`、`waitUntil`；给关键元素加 `data-testid`（`space-name`、`tab-row`、`address-input`、`new-tab`、`nav-back`、`nav-forward`、`nav-reload`、`empty-content`）。
  - 跑完整个 e2e，确认已有的 `window.spec.ts`、`daemon.spec.ts`、`orphan.spec.ts` 仍通过。注意：已存档需求的用例标题用的是它们自己的场景 ID（`R6.S1 窗口打开`、`R7.S1 启动拉起 alterd` 等），本需求的 `--grep "R6\.S1"`、`--grep "R7\.S1"` 会把它们一起选中；它们也必须通过，不要改旧用例的标题。
- [x] T5 [covers:R6.S1,R8.S1] [after:T4] 新建标签：地址框 → alterd → WebContentsView 加载 → 标题回写 — seam: Playwright `_electron.launch` + 本地 HTTP 测试服务器 — verify: `bun run e2e --grep "R6\\.S1|R8\\.S1"`
  - 测试服务器：新文件 `apps/browser/e2e/fixtures/server.ts`，`http.createServer` 监听 `127.0.0.1:0`，提供 `/a`、`/b`、`/c`（`<title>Page A</title>` 等）与 `/count`（记录请求次数），在每个用例里启动 / 关闭。不访问外网。
  - 文件：`src/main/tab-views.ts`（规则见 design.md「核心流程」下的 TabViews 规则 1–5）、`src/renderer/src/AddressBar.tsx`（新标签模式）、`Sidebar.tsx`（标签行显示标题，没有标题时显示 URL）。
  - 断言内容区 URL：`app.evaluate(({ webContents }) => webContents.getAllWebContents().map((w) => w.getURL()))`，不要依赖 `app.windows()` 是否包含 WebContentsView。
  - R8.S1 落库断言：Playwright 跑在 Node 下（本机 v24.21.0），用 `node:sqlite` 的 `DatabaseSync` 以 `{ readOnly: true }` 打开 `alter.db` 查 `tabs.title`；alterd 用 WAL，可以并发读。
- [x] T6 [covers:R7.S1,R7.S2] [after:T5] 在当前标签打开：alterd 回 `shell.navigate`，Main 在该视图 `loadURL` — seam: 假客户端（R7.S2）+ Playwright（R7.S1） — verify: `cargo test -p alterd --test shell r7_ && bun run e2e --grep "R7\\.S1"`
  - 文件：`crates/alterd/src/tabs/mod.rs`（`openRequested` 带 `tabId` 的分支：标签不存在回 `E_TAB_NOT_FOUND`）、`src/main/shell-link.ts`（处理 `shell.navigate`）、`AddressBar.tsx`（非新标签模式带上当前激活标签的 `tabId`；没有激活标签时不带）。
- [x] T7 [covers:R9.S1,R10.S1,R10.S2] [after:T6] 激活与关闭标签 — seam: 假客户端（R10.S1）+ Playwright（R9.S1、R10.S2） — verify: `cargo test -p alterd --test shell r10_ && bun run e2e --grep "R9\\.S1|R10\\.S2"`
  - 文件：`crates/alterd/src/tabs/mod.rs`（`tabActivated`、`tabClosed`：激活相邻标签时下方优先）、`crates/alter-core/src/store/tabs.rs`、`src/main/tab-views.ts`（切换挂载的视图；关闭时 `webContents.close()`）、`Sidebar.tsx`（行点击、关闭按钮 `data-testid="tab-close"`）。
  - R10.S1 的 `archived_at` 断言在 Rust 测试里用 `rusqlite` 只读打开 `alter.db`（WAL 模式下可以并发读）。
- [x] T8 [covers:R12.S1,R12.S2] [after:T7] 后退、前进、刷新 — seam: Playwright `_electron.launch` + 本地测试服务器 — verify: `bun run e2e --grep "R12\\."`
  - 文件：`src/main/tab-views.ts`（`webContents.navigationHistory.goBack()` / `goForward()`、`reload()`；在 `did-navigate*` 后推送 `nav` 状态）、`src/renderer/src/NavButtons.tsx`（`disabled` 绑定 `canGoBack` / `canGoForward`）。
  - 「不可点击」用 Playwright 的 `toBeDisabled()` 断言。
- [x] T9 [covers:R11.S1,R11.S2] [after:T8] 重启恢复与懒加载 — seam: Playwright（同一 `ALTER_HOME` 启动两次） — verify: `bun run e2e --grep "R11\\."`
  - 文件：`src/main/tab-views.ts`（首次快照只为激活标签建视图——规则 2 已保证；本任务主要是验证并修正恢复路径上的问题）。
  - 第一次启动后用 `app.close()` 正常退出（走 `before-quit`），再用同一个 `ALTER_HOME` 调 `launchApp`。
- [x] T10 [after:T9] 成功标准的基准测试与文档 — verify: `cargo test -p alterd --release --test shell_perf -- --ignored --nocapture && bun run e2e --grep SC2`
  - SC1：新文件 `crates/alterd/tests/shell_perf.rs`（`#[ignore]`，只在显式要求时跑），先经 `openRequested` 建 500 个标签，再计 100 次 `openRequested` → 收到快照的耗时，打印 p95 并断言 < 10 ms。
  - SC2：`apps/browser/e2e/sidebar.spec.ts` 里加用例 `SC2 重启恢复 100 个标签`：第一次启动用测试服务器的 `/n/<i>` 开 100 个标签（可以直接经 ShellLink 发事件，不必点 UI），重启后测从 `daemon.status` 为 running 到侧边栏出现 100 个 `tab-row` 的时间 < 1000 ms。
  - 文档：AGENTS.md 的「目录结构」「验证与测试」补上 `shell/`、`tabs/`、`tab-views.ts` 与新的 e2e 文件；基础文档（api.md 的 Shell Control 表、components.md、core-flows.md 打开页面流程）留到存档步骤更新。

## 收敛任务

- [x] T11 [after:T10] 只回写主框架的页面内导航（CR1） — verify: `bun run e2e --grep "CR1"`
  - 先写失败的 e2e：`apps/browser/e2e/fixtures/server.ts` 加 `/frame`（标题 `Frame`，内嵌 `<iframe src="/inner">`）与 `/inner`（加载后执行 `location.hash = "x"`）；用例「CR1 iframe 页内导航不改标签地址」新建标签打开 `/frame`，等 1 秒后用 `node:sqlite` 只读查 `tabs.url`，应为 `/frame` 的地址，修复前会变成 `/inner#x`。
  - 修复：`src/main/tab-views.ts` 的 `did-navigate-in-page` 回调用第三个参数 `isMainFrame`，只有主框架才回写（`did-navigate` 本身只对主框架触发）。
- [x] T12 [after:T10] 每个改变状态的事件都向每个连接各发一条快照（CR3） — verify: `cargo test -p alterd --test shell`
  - 先写失败的契约测试 `r2_consecutive_events_each_get_snapshot`：同一连接连发两条 `openRequested`（不先读），之后应依次读到标签数为 1 和 2 的两份快照；另一个连接也一样。
  - 修复：回到 design.md 的 `tokio::sync::broadcast`（容量 256）：TabService 每次 `publish` 都 `send` 一份；连接任务先 `subscribe` 再取当前快照发出（新连接可能多收到一份重复快照，全量快照重复无害）。`Lagged` 时记 WARN 并继续 `recv`（缓冲里最新的快照仍会送达）。当前快照仍用 `watch` 保存，只用于新连接的第一条。
  - 同步：log.md 里 T1 关于 watch 的偏差说明保留为历史，本任务的决策写进「决策与偏差」。
- [x] T13 [after:T10] SC2 从 `alterd.json` 写入时刻计时（CR4） — verify: `bun run e2e --grep SC2`
  - `sidebar.spec.ts` 的 SC2：起点取 `alterd.json` 的 `mtimeMs`（alterd 写完健康文件后，Main 至多 100 ms 才判定 running，所以这个起点不晚于 running）；终点是测试进程首次观察到 100 行的时刻（每 10 ms 轮询）。两个时刻都用墙钟毫秒，结果只会高估。
- [x] T14 [covers:R10.S1] [after:T10] R10.S1 用例改为让「最近激活」与「下方相邻」指向不同标签（CR5） — seam: `alterd` 二进制 + 假客户端 — verify: `cargo test -p alterd --test shell r10_`
  - 依次建 A、B、C 后先激活 A、再激活 B，关闭 B：期望 C 激活（下方相邻），而剩余标签里最近激活的是 A。
  - 用变异证明：删掉 `close_tab` 里激活相邻标签的分支后，测试应失败（A 成为激活）。
- [x] T15 [after:T10] 侧边栏显示 favicon（CR6） — verify: `bun run e2e --grep "CR6"`
  - `fixtures/server.ts` 给测试页加 `<link rel="icon" href="/favicon.png">` 并返回一个 1×1 PNG；`Sidebar.tsx` 在标题前渲染 `<img data-testid="tab-favicon" src={tab.faviconUrl}>`（没有时不渲染）；`renderer/index.html` 的 CSP 加 `img-src 'self' http: https: data:`。用例「CR6 侧边栏显示 favicon」断言 `tab-favicon` 的 `src` 为 `/favicon.png` 的地址。
- [x] T16 [after:T10] 地址框下方显示 alterd 回的错误（CR7） — verify: `bun run e2e --grep "CR7"`
  - `AddressBar.tsx` 下方渲染 `<p data-testid="shell-error">{error.message}</p>`；Main 收到下一份快照时清掉 `error`，提示随之消失。
  - 用例「CR7 错误提示」：新建标签打开 `/a` 后，测试进程用 `node:sqlite` 以读写方式把该标签的 `tabs.archived_at` 置为非空（模拟它已在别处被关闭；alterd 用 WAL，并发写可行，且不会触发新快照），再在地址框输入 `/b` 的地址回车：Main 仍带着这个 `tabId` 发 `openRequested`，alterd 回 `E_TAB_NOT_FOUND`，断言 `shell-error` 显示包含「标签不存在」。
- [x] T17 [after:T10] IPC 通道与载荷对齐 design.md（CR8） — verify: `bun run typecheck && bun run e2e`
  - `shared/shell.ts`：`tab.activate` / `tab.close` 的载荷改为 `{ tabId }`；导航拆为 `nav.back`、`nav.forward`、`nav.reload` 三个通道，载荷 `{}`；preload 与 `main/index.ts` 同步。行为不变，跑完整个 e2e。
- [x] T18 [covers:R14.S1] [after:T10] 网页弹窗转为当前 Space 的新标签（CR2） — seam: Playwright `_electron.launch` + 本地测试服务器 — verify: `bun run e2e --grep "R14\\.S1"`
  - `fixtures/server.ts` 加 `/popup`（标题 `Popup`，内容 `<a id="pop" target="_blank" href="/b">pop</a>`）。用例在主进程里用 `webContents.getAllWebContents()` 找到 URL 为 `/popup` 的那个，`executeJavaScript("document.getElementById('pop').click()")` 触发（点击发生在页面视图里，渲染进程 DOM 够不到）；断言侧边栏两行、`Page B` 激活、`BrowserWindow.getAllWindows().length === 1`。
  - 实现：`src/main/tab-views.ts` 的 `create()` 里 `wc.setWindowOpenHandler(({ url }) => { this.send({ type: "shell.event.openRequested", spaceId: tab.spaceId, input: url }); return { action: "deny" }; })`。
- [x] T19 [covers:R2.S4] [after:T18] 慢连接积压时仍收到最新快照（CR3 剩余） — seam: `alterd` 二进制 + 两个假客户端 — verify: `cargo test -p alterd --test shell r2_s4`
  - 契约测试 `r2_s4_slow_connection_still_gets_latest`：B 连上读完初始快照后不读；A 连发 300 条并读完自己的 300 份；然后 B 循环 `recv_within(2s)` 直到读到 300 个标签的快照或超时。macOS Unix socket 的发送缓冲很小，B 的连接任务很快阻塞在写上，广播会溢出 256 的容量，覆盖 `Lagged` 分支。
  - 实现预计已满足（`Lagged` 时继续 `recv`），用变异证明：`Lagged` 时 `return` 关闭连接，测试应失败。
- [x] T20 [covers:R5.S6] [after:T18] 地址规范化保留 `blob:` — seam: `alterd` 二进制 + 假客户端 — verify: `cargo test -p alterd --test shell r5_`
  - `crates/alterd/src/tabs/url.rs`：`has_scheme(s) || s.starts_with("about:")` 处加 `|| s.starts_with("blob:")`。期望值用场景里的字面量。
- [x] T21 [covers:R14.S2] [after:T20] `window.open` 打开 blob 页面变成新标签（CR9） — seam: Playwright `_electron.launch` + 本地测试服务器 — verify: `bun run e2e --grep "R14\\.S2"`
  - `fixtures/server.ts` 加 `/blob`（标题 `Blob Opener`）；用例在主进程里对 `/blob` 页面 `executeJavaScript('window.open(URL.createObjectURL(new Blob(["<title>Blob</title>"], {type: "text/html"})))', true)`（`userGesture` 为 true，避免弹窗拦截）；断言两行与 `Blob` 激活。依赖 T20，Main 侧无需改动。
- [x] T22 [after:T18] favicon 用例断言图片真的加载成功；CR7 用例排除迟到更新的干扰（CR11） — verify: `bun run e2e --grep "CR6|CR7"`
  - CR6：再断言 `tab-favicon` 的 `complete && naturalWidth > 0`（`expect.poll` 2 秒）。用变异证明：去掉 `index.html` CSP 里的 `img-src`，用例应失败。
  - CR7：归档标签后先等 1 秒并断言 `shell-error` 不可见（排除页面迟到的 `tabUpdated` 引起的同样错误），再回车，再断言出现。
- [x] T23 [covers:R5.S2] [after:T22] 主机名只在第一个 `/` 前截取，与 R5 一致（验证缺口 1） — seam: `alterd` 二进制 + 假客户端 — verify: `cargo test -p alterd --test shell r5_`
  - 先写失败的契约测试 `r5_query_without_path_is_search`：`input` 为 `example.com?q=x`，期望 `https://www.google.com/search?q=example.com%3Fq%3Dx`（R5：只有「主机名[:端口][/路径]」形式补 https，其余走搜索）。
  - 修复：`crates/alterd/src/tabs/url.rs` 的 `host_of` 只按 `/` 切分；`r5_s2_bare_domain_gets_https` 等已有用例保持通过（R5.S2 的证据在本任务重新记一对 RED/GREEN：用变异「`host_of` 返回整串」）。
  - 同步 design.md「接口契约」里的规范化说明：补上「主机名不能以点号开头或结尾」这条现有限制。
- [x] T24 [covers:R7.S2] [after:T22] 带 `tabId` 的 `openRequested` 也先校验 `spaceId`（验证缺口 2） — seam: `alterd` 二进制 + 假客户端 — verify: `cargo test -p alterd --test shell r7_`
  - 先写失败的契约测试 `r7_open_in_tab_with_unknown_space_is_rejected`：带已存在标签的 `tabId`、`spaceId` 为 `01JZZZZZZZZZZZZZZZZZZZZZZZ`，期望 `shell.error` 的 `code` 为 `E_SPACE_NOT_FOUND`。
  - 修复：`crates/alterd/src/tabs/mod.rs` 把 `space_exists` 校验移到 `tabId` 分支之前。R7.S2 的证据在本任务重新记一对 RED/GREEN（变异：带 `tabId` 时不回 `shell.navigate`）。
