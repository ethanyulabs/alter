---
change: add-spaces-sidebar
---

# 执行日志：浏览器基础 UI：Shell Control 与侧边栏标签

<!-- 记录实际运行过什么。没有亲眼看到的结果不要写。
### T<n> <标题>
- Revision: <当前 revision>
- RED: `<命令>` → FAIL: <能看出预期失败原因的输出摘录>
- GREEN: `<命令>` → PASS
- REFACTOR: <改了什么> → PASS
每个覆盖的场景一对 RED/GREEN（RED 和 GREEN 两行都写上场景 ID）。一写就通过的测试，要先证明它会失败：
"- RED (mutation: <你故意改坏了什么>): `<命令>` → FAIL: …"，恢复代码后再记 GREEN。
（[baseline] 任务不写 RED/GREEN，改写 "- BASELINE: `<命令>` → PASS on unchanged code"。） -->

## 证据

### T1 协议类型与错误码导出；alterd 监听 `shell.sock`，连接即收到含默认 Space 的快照
- Revision: 2
- RED: `cargo test -p alter-core --test export_ts` → FAIL: assertion `left == right` failed，left 为 14 个错误码，right 多出 `E_INVALID_ARGUMENT`、`E_TAB_NOT_FOUND` (R13.S1)
- GREEN: `cargo test -p alter-core --test export_ts` → PASS (1 test) (R13.S1)
- RED: `cargo test -p alter-core --test export_ts r13_s2` → FAIL: 读取 …/ShellCommand.ts 失败：No such file or directory (R13.S2)
- GREEN: `cargo test -p alter-core --test export_ts` → PASS (2 tests) (R13.S2)
- RED: `cargo test -p alterd --test shell` → FAIL: 应当能连接 shell.sock: Os { code: 2, kind: NotFound } (R1.S1)
- GREEN: `cargo test -p alterd --test shell` → PASS (1 test) (R1.S1)
- RED (mutation: `shell::bind` 里的 umask 改为 0o022): `cargo test -p alterd --test shell r1_s2` → FAIL: assertion `left == right` failed: 权限位：755 (R1.S2)
- GREEN: `cargo test -p alterd --test shell r1_s2` → PASS (1 test，变异已恢复) (R1.S2)
- RED (mutation: `ensure_default_space` 的判断改为 `open > 0 && false`，每次启动都插入): `cargo test -p alterd --test shell r4_s1` → FAIL: 快照里出现两个 Personal (R4.S1)
- GREEN: `cargo test -p alterd --test shell` → PASS (3 tests，变异已恢复) (R4.S1)
- VERIFY: `bun run protocol:export` 重新生成 `packages/protocol/src/generated/`（新增 ShellCommand / ShellEvent / ShellState / SpaceView / TabView）；`bun run ci:check` → exit 0（第一次因 fmt 与测试里尚未使用的 send 辅助函数失败，格式化并删掉辅助函数后通过）；`bun run e2e` → 7 passed，本机 macOS 主会话运行

### T2 `openRequested` 新建标签，地址按规则规范化
- Revision: 2
- RED: `cargo test -p alterd --test shell r5_` → FAIL: r5_s1_url_with_scheme_is_kept panicked: 1 秒内没有收到消息（alterd 还不处理事件） (R5.S1)
- GREEN: `cargo test -p alterd --test shell r5_` → PASS (1 test) (R5.S1)
- RED: `cargo test -p alterd --test shell r5_s2` → FAIL: left: "example.com/docs" right: "https://example.com/docs" (R5.S2)
- GREEN: `cargo test -p alterd --test shell r5_` → PASS (2 tests) (R5.S2)
- RED: `cargo test -p alterd --test shell r5_s3` → FAIL: left: "localhost:8080" right: "http://localhost:8080" (R5.S3)
- GREEN: `cargo test -p alterd --test shell r5_` → PASS (3 tests) (R5.S3)
- RED: `cargo test -p alterd --test shell r5_s4` → FAIL: left: "rust 生命周期" right: "https://www.google.com/search?q=rust%20%E7%94%9F%E5%91%BD%E5%91%A8%E6%9C%9F" (R5.S4)
- GREEN: `cargo test -p alterd --test shell r5_` → PASS (4 tests) (R5.S4)
- RED (mutation: `normalize_input` 的空输入判断恒为假，空串照常规范化): `cargo test -p alterd --test shell r5_s5` → FAIL: 收到的是 `shell.stateChanged`（建出了搜索空串的标签），不是 `shell.error` (R5.S5)
- GREEN: `cargo test -p alterd --test shell` → PASS (9 tests，变异已恢复) (R5.S5)
- VERIFY: `bun run ci:check` → exit 0；`bun run e2e` → 7 passed，本机 macOS 主会话运行

### T3 多连接广播、坏行丢弃、错误只回发送方、socket 路径过长报错
- Revision: 2
- RED (mutation: `TabService::run` 处理成功后只把新快照回给发送方，不更新 watch): `cargo test -p alterd --test shell r2_s1` → FAIL: r2_s1_both_connections_receive_change panicked: 1 秒内没有收到消息 (R2.S1)
- GREEN: `cargo test -p alterd --test shell r2_s1` → PASS (1 test，变异已恢复) (R2.S1)
- RED (mutation: 连接遇到无法解析的行后 `return` 断开): `cargo test -p alterd --test shell r2_s2` → FAIL: r2_s2_malformed_line_is_dropped panicked at shell.rs:109（`recv_within` 读到 0 字节：连接被关闭） (R2.S2)
- GREEN: `cargo test -p alterd --test shell r2_s2` → PASS (1 test，变异已恢复) (R2.S2)
- RED (mutation: 事件被拒绝时也调用 `publish` 广播快照): `cargo test -p alterd --test shell r2_s3` → FAIL: r2_s3_rejection_goes_to_sender_only panicked: B 不应收到消息 (R2.S3)
- GREEN: `cargo test -p alterd --test shell r2_s3` → PASS (1 test，变异已恢复) (R2.S3)
- RED (mutation: `shell::bind` 失败时改用 `$TMPDIR/m<pid>.sock` 继续启动): `cargo test -p alterd --test shell r3_s1` → FAIL: r3_s1_socket_path_too_long panicked: 2 秒内应当退出 (R3.S1)
- GREEN: `cargo test -p alterd --test shell r3_s1` → PASS (1 test，变异已恢复，残留的 `m87707.sock` 已删除) (R3.S1)
- VERIFY: `bun run ci:check` → exit 0（shell 契约测试 13 passed）

### T4 Electron 连接 Shell Control，渲染侧边栏骨架
- Revision: 2
- RED: `bun run e2e --grep "R4\\.S2"` → FAIL: expect(locator).toHaveText(expected) failed，Expected: "Personal"，Error: element(s) not found (R4.S2)
- GREEN: `bun run e2e --grep "R4\\.S2"` → PASS (1 test) (R4.S2)
- VERIFY: `bun run ci:check` → exit 0（第一次 typecheck 报 `.css` 无类型声明、lint 报 `_type` 未使用，补 `vite/client` 引用并改为显式构造 state 后通过）；`bun run e2e` → 8 passed（含已存档需求的 R6.S1、R7.S1–S3、R2.S1、R3.S1），本机 macOS 主会话运行

### T5 新建标签：地址框 → alterd → WebContentsView 加载 → 标题回写
- Revision: 2
- RED: `bun run e2e --grep "R6\\.S1|R8\\.S1"` → FAIL: sidebar.spec.ts「R6.S1 从侧边栏新建标签」30.0s 超时（找不到 `new-tab` 按钮） (R6.S1)
- RED: `bun run e2e --grep "R6\\.S1|R8\\.S1"` → FAIL: sidebar.spec.ts「R8.S1 标题显示并落库」30.0s 超时（找不到 `new-tab` 按钮；alterd 也还忽略 `tabUpdated`） (R8.S1)
- GREEN: `bun run e2e --grep "R6\\.S1|R8\\.S1"` → PASS (3 tests，含已存档的「R6.S1 窗口打开」) (R6.S1)
- GREEN: `bun run e2e --grep "R6\\.S1|R8\\.S1"` → PASS (3 tests；`node:sqlite` 只读查到 `[{ title: "Page A" }]`) (R8.S1)
- VERIFY: `bun run ci:check` → exit 0（第一次 lint 报未用的 `waitUntil` 与 `no-unassigned-vars`，改为回调 ref；第二次 fmt，格式化后通过）；`bun run e2e` → 10 passed，本机 macOS 主会话运行；另加 `r8_update_unknown_tab_is_rejected`（`E_TAB_NOT_FOUND`），变异「标签不存在时当作无变化」下 FAIL: 1 秒内没有收到消息，恢复后 PASS

### T6 在当前标签打开：alterd 回 `shell.navigate`，Main 在该视图 `loadURL`
- Revision: 2
- RED: `cargo test -p alterd --test shell r7_` → FAIL: r7_s2_open_in_tab_sends_navigate panicked: 1 秒内没有收到消息 (R7.S2)
- GREEN: `cargo test -p alterd --test shell` → PASS (15 tests) (R7.S2)
- RED: `bun run e2e --grep "R7\\.S1"` → FAIL: sidebar.spec.ts「R7.S1 当前标签导航」expect(locator).toHaveCount(expected) failed，Expected: 1，Received: 2 (R7.S1)
- GREEN: `bun run e2e --grep "R7\\.S1"` → PASS (3 tests，含已存档的「R7.S1 启动拉起 alterd」「R7.S1 迟到就绪」) (R7.S1)
- VERIFY: `bun run ci:check` → exit 0；`bun run e2e` → 11 passed，本机 macOS 主会话运行

### T7 激活与关闭标签
- Revision: 2
- RED: `bun run e2e --grep "R9\\.S1"` → FAIL: sidebar.spec.ts「R9.S1 点击切换」expect(received).toBe(expected)，Expected: true，Received: false（点击后挂载的视图仍是 /b） (R9.S1)
- GREEN: `bun run e2e --grep "R9\\.S1"` → PASS (1 test) (R9.S1)
- RED: `cargo test -p alterd --test shell r10_` → FAIL: r10_s1_close_active_middle_tab_activates_next panicked: 1 秒内没有收到消息（alterd 还不处理 tabClosed） (R10.S1)
- GREEN: `cargo test -p alterd --test shell` → PASS (16 tests) (R10.S1)
- RED: `bun run e2e --grep "R10\\.S2"` → FAIL: sidebar.spec.ts「R10.S2 关闭最后一个标签」30.0s 超时（找不到 `tab-close`） (R10.S2)
- GREEN: `bun run e2e` → PASS (13 tests) (R10.S2)
- VERIFY: `bun run ci:check` → exit 0；`bun run e2e` → 13 passed，本机 macOS 主会话运行。中途关闭按钮的「×」进了行文字，导致按标题断言的 R7.S1 / R8.S1 / R9.S1 失败，改为 CSS 伪元素后全部通过

### T8 后退、前进、刷新
- Revision: 2
- RED: `bun run e2e --grep "R12\\."` → FAIL: sidebar.spec.ts「R12.S1 后退与前进」Error: expect(locator).toBeDisabled() failed（没有 `nav-forward`） (R12.S1)
- RED: `bun run e2e --grep "R12\\."` → FAIL: sidebar.spec.ts「R12.S2 刷新」30.0s 超时（没有 `nav-reload`） (R12.S2)
- GREEN: `bun run e2e --grep "R12\\."` → PASS (2 tests) (R12.S1)
- GREEN: `bun run e2e --grep "R12\\."` → PASS (2 tests) (R12.S2)
- VERIFY: `bun run ci:check` → exit 0；`bun run e2e` → 15 passed，本机 macOS 主会话运行

### T9 重启恢复与懒加载
- Revision: 2
- RED (mutation: `list_open_tabs` 改为按 `t.position DESC` 排序): `bun run e2e --grep "R11\\.S1"` → FAIL: sidebar.spec.ts「R11.S1 顺序与激活标签恢复」Error: expect(locator).toHaveText(expected) failed (R11.S1)
- GREEN: `bun run e2e --grep "R11\\.S1"` → PASS (1 test，变异已恢复) (R11.S1)
- RED (mutation: `TabViews.apply` 为快照里的全部标签建视图): `bun run e2e --grep "R11\\.S2"` → FAIL: expect(received).not.toContain(expected)，Expected value: not "http://127.0.0.1:53402/a"，Received array 含 /c、/b、/a (R11.S2)
- GREEN: `bun run e2e --grep "R11\\.S2"` → PASS (1 test，变异已恢复) (R11.S2)
- VERIFY: `bun run ci:check` → exit 0；`bun run e2e` → 17 passed，本机 macOS 主会话运行

### T10 成功标准的基准测试与文档
- Revision: 2
- VERIFY: `cargo test -p alterd --release --test shell_perf -- --ignored --nocapture` → PASS：`SC1: 100 次 openRequested → 快照（500–600 个标签）p50 1.015542ms，p95 1.162ms`（预算 p95 < 10 ms）
- VERIFY: `bun run e2e --grep SC2` → PASS：`SC2: running → 100 行 59 ms`（预算 < 1000 ms）；全量 `bun run e2e` 时为 14 ms，18 passed，本机 macOS 主会话运行
- VERIFY: `bun run ci:check` → exit 0；AGENTS.md 的「现状」「命令」「目录结构」「验证与测试」已更新（108 行）

### T11 只回写主框架的页面内导航（CR1）
- Revision: 3
- RED: `bun run e2e --grep "CR1"` → FAIL: storedUrls 期望 `["http://127.0.0.1:57567/frame"]`，实际 `["http://127.0.0.1:57567/inner#x"]`（复现 CR1）
- GREEN: `bun run e2e --grep "CR1"` → PASS (1 test)
- VERIFY: `bun run ci:check` → exit 0；`bun run e2e` 全量通过，本机 macOS 主会话运行

### T12 每个改变状态的事件都向每个连接各发一条快照（CR3）
- Revision: 3
- RED: `cargo test -p alterd --test shell r2_consecutive`（连跑 3 次）→ 2 次 FAIL: r2_consecutive_events_each_get_snapshot panicked at tests/common/mod.rs:129（1 秒内没有收到第二份快照，watch 把两份合并了），1 次 PASS——缺陷依赖调度，不稳定但可复现
- GREEN: `cargo test -p alterd --test shell`（连跑 5 次）→ 每次 PASS (17 tests)；`cargo test -p alterd --test shell r2_` 连跑 20 次 → 20 次 PASS
- VERIFY: `cargo test -p alterd --release --test shell_perf -- --ignored --nocapture` → SC1 p50 987.375µs，p95 1.12275ms；`bun run ci:check` → exit 0；`bun run e2e` → 19 passed，本机 macOS 主会话运行

### T13 SC2 从 `alterd.json` 写入时刻计时（CR4）
- Revision: 3
- VERIFY: `bun run e2e --grep SC2` 连跑 3 次 → PASS：`SC2: alterd.json 写出 → 100 行` 77 ms、145 ms、145 ms（预算 < 1000 ms）。起点取 `alterd.json` 的 `mtimeMs`（不晚于 running），终点为测试进程首次看到 100 行的时刻，轮询间隔沿用 `waitUntilAsync` 的 50 ms（tasks.md 写的是 10 ms，更粗的间隔只会让结果偏大）

### T14 R10.S1 用例改为让「最近激活」与「下方相邻」指向不同标签（CR5）
- Revision: 3
- RED (mutation: `close_tab` 里激活相邻标签的条件恒为假): `cargo test -p alterd --test shell r10_` → FAIL: assertion `left == right` failed，快照里 A 为激活标签（R10.S1 期望 C） (R10.S1)
- GREEN: `cargo test -p alterd --test shell r10_` → PASS (1 test，变异已恢复) (R10.S1)
- VERIFY: `cargo test -p alterd --test shell` → 17 passed

### T15 侧边栏显示 favicon（CR6）
- Revision: 3
- RED: `bun run e2e --grep "CR6"` → FAIL: expect(locator).toHaveAttribute(expected) failed，Error: element(s) not found（没有 `tab-favicon`）
- GREEN: `bun run e2e --grep "CR6"` → PASS (1 test)
- VERIFY: `bun run ci:check` → exit 0；`bun run e2e` 全量通过，本机 macOS 主会话运行

### T16 地址框下方显示 alterd 回的错误（CR7）
- Revision: 3
- RED: `bun run e2e --grep "CR7"` → FAIL: expect(locator).toContainText(expected) failed，Error: element(s) not found（没有 `shell-error`）
- GREEN: `bun run e2e --grep "CR7"` → PASS (1 test)
- VERIFY: `bun run ci:check` → exit 0；`bun run e2e` 全量通过，本机 macOS 主会话运行

### T17 IPC 通道与载荷对齐 design.md（CR8）
- Revision: 3
- VERIFY: `bun run typecheck` → 无错误；`bun run e2e` → 21 passed（R9.S1 激活、R10.S2 关闭、R12.S1 / R12.S2 导航都经新的 `tab.*` `{tabId}` 载荷与 `nav.back` / `nav.forward` / `nav.reload` 通道），本机 macOS 主会话运行；`bun run ci:check` → exit 0

### T18 网页弹窗转为当前 Space 的新标签（CR2）
- Revision: 3
- RED: `bun run e2e --grep "R14\\.S1"` → FAIL: expect(locator).toHaveText(expected) failed，Expected 两行，Received 只有 `Popup` 一行（弹窗去了独立窗口） (R14.S1)
- GREEN: `bun run e2e --grep "R14\\.S1"` → PASS (1 test) (R14.S1)
- VERIFY: `bun run ci:check` → exit 0；`bun run e2e` → 22 passed，本机 macOS 主会话运行；`pgrep -fl target/debug/alterd` 无残留

### T19 慢连接积压时仍收到最新快照（CR3 剩余）
- Revision: 4
- RED (mutation: 连接任务在 `RecvError::Lagged` 时 `return` 关闭连接): `cargo test -p alterd --test shell r2_s4` → FAIL: r2_s4_slow_connection_still_gets_latest panicked at tests/common/mod.rs:107（连接被关闭——说明用例确实触发了广播溢出） (R2.S4)
- GREEN: `cargo test -p alterd --test shell r2_s4` → PASS (1 test，变异已恢复) (R2.S4)

### T20 地址规范化保留 `blob:`
- Revision: 4
- RED: `cargo test -p alterd --test shell r5_s6` → FAIL: left: "https://www.google.com/search?q=blob%3Ahttps%3A%2F%2Fexample.com%2F0f6c…" right: "blob:https://example.com/0f6c…" (R5.S6)
- GREEN: `cargo test -p alterd --test shell` → PASS (19 tests) (R5.S6)

### T21 `window.open` 打开 blob 页面变成新标签（CR9）
- Revision: 4
- RED (mutation: 去掉 `normalize_input` 里的 `blob:` 规则): `bun run e2e --grep "R14\\.S2"` → FAIL: expect(locator).toHaveText(expected) failed（第二行不是 `Blob`，blob URL 被当成搜索词） (R14.S2)
- GREEN: `bun run e2e --grep "R14\\.S2"` → PASS (1 test，变异已恢复) (R14.S2)

### T22 favicon 用例断言图片真的加载成功；CR7 用例排除迟到更新的干扰（CR11）
- Revision: 4
- RED (mutation: 去掉 `renderer/index.html` CSP 里的 `img-src`): `bun run e2e --grep "CR6"` → FAIL: expect(received).toBe(expected)，Expected: true，Received: false（图片被 CSP 拦截，naturalWidth 为 0）
- GREEN: `bun run e2e --grep "CR6|CR7"` → PASS (2 tests，变异已恢复)
- VERIFY: `bun run ci:check` → exit 0；`bun run e2e` → 23 passed（SC2 127 ms），本机 macOS 主会话运行；`cargo test -p alterd --release --test shell_perf -- --ignored --nocapture` → SC1 p50 997.5µs，p95 1.103458ms

### T23 主机名只在第一个 `/` 前截取，与 R5 一致（验证缺口 1）
- Revision: 4
- RED: `cargo test -p alterd --test shell r5_query` → FAIL: left: "https://example.com?q=x" right: "https://www.google.com/search?q=example.com%3Fq%3Dx"（复现缺口）
- RED (mutation: `host_of` 返回整串，不再截取主机名): `cargo test -p alterd --test shell r5_s2` → FAIL: r5_s2_bare_domain_gets_https assertion `left == right` failed (R5.S2)
- GREEN: `cargo test -p alterd --test shell` → PASS (21 tests，变异已恢复) (R5.S2)
- VERIFY: design.md「接口契约」的规范化说明补上「不以 `.` 开头或结尾」与「先校验 spaceId」

### T24 带 `tabId` 的 `openRequested` 也先校验 `spaceId`（验证缺口 2）
- Revision: 4
- RED: `cargo test -p alterd --test shell r7_open_in_tab_with` → FAIL: 收到 `{"type":"shell.navigate",…}`，期望 `shell.error`（复现缺口）
- RED (mutation: 带 `tabId` 时不回 `shell.navigate`): `cargo test -p alterd --test shell r7_s2` → FAIL: r7_s2_open_in_tab_sends_navigate panicked: 1 秒内没有收到消息 (R7.S2)
- GREEN: `cargo test -p alterd --test shell` → PASS (21 tests，变异已恢复) (R7.S2)
- VERIFY: `bun run ci:check` → exit 0（Rust 39 passed、0 failed；bun test 1 pass）；`bun run e2e` → 23 passed，本机 macOS 主会话运行

## 决策与偏差
- 2026-10-21：起始状态：分支 `feature/add-spaces-sidebar`，已合入 `origin/main` 31aafee（含 fix-orphan-alterd），工作区干净；改动前 `bun run ci:check` exit 0。
- 2026-10-21：T1 的快照分发用 `tokio::sync::watch` 代替 design.md 写的 `broadcast`：watch 只保留最新值，新连接直接取当前快照，慢连接自然跳过中间快照（全量快照可以跳过），不存在 `Lagged` 处理。对外行为与契约不变，存档时同步 design 描述。
- 2026-10-21：tasks.md 建议在 `crates/alter-core/tests/store.rs` 为仓储加单测；T1 的仓储（`ensure_default_space`、`list_spaces`、`list_open_tabs`）已经由契约测试经真实 alterd 覆盖，没有另加，避免两处测同一行为。
- 2026-10-21：`TabService` 的 `store` / `state` 字段在 T1 只被持有，用 `#[expect(dead_code)]` 标注；T2 用上之后该标注会报错，届时删除。
- 2026-10-21：T2 补了需求 R5 正文里「以 `about:` 开头的原样使用」这条规则（没有对应场景），加了测试 `r5_about_prefix_is_kept`，先观察到它失败（被当成搜索词）再实现。
- 2026-10-21：T2 起 TabService 对尚未实现的事件（`tabUpdated`、`tabActivated`、`tabClosed`、带 `tabId` 的 `openRequested`）只记 WARN 并忽略，T5–T7 逐个实现；T1 的 `#[expect(dead_code)]` 已删除。
- 2026-10-21：T3 的四个场景一写就通过：广播、坏行丢弃、只回发送方在 T2 打通事件链路时已经实现；路径过长时 std 的 `UnixListener::bind` 本身就拒绝（`path must be shorter than SUN_LEN`），经 `main.rs` 的 `code = 1` 日志退出，错误链里带「监听 shell.sock 失败」。因此每个场景都用变异 RED 证明测试有效，没有加 tasks.md 建议的显式长度检查（行为已满足 R3，少一份重复逻辑）。watch 代替 broadcast 后也不存在 `Lagged` 处理。
- 2026-10-21：T4 给 `apps/browser` 加了 workspace 依赖 `@alter/protocol`（只用类型）；preload 的订阅逻辑抽成 `subscribe()` 供 daemon 与 shell 两个通道共用。`data-testid` 只加了本任务用到的 `space-name`、`tab-row`、`empty-content`，其余随各自的任务加。
- 2026-10-21：T5 起 `TabService::handle` 返回「状态是否变化」，只有变化才广播；`tabUpdated` 在 alterd 侧实现（`Store::update_tab`，值不变时不写库、不广播）。T5 里地址框提交一律按新建标签发送（不带 `tabId`），「在当前标签打开」由 T6 实现。
- 2026-10-21：T7 起 `TabService::handle` 对四种事件都已实现，T2 时的「暂不支持」分支已删除。关闭时相邻标签的选择用一条 SQL（`ORDER BY position > ? DESC, ABS(position - ?)`）实现「下方优先、否则上方」；上方回退没有单独的场景，属于 R10 正文的一部分。
- 2026-10-21：T8 的刷新按钮在当前 Space 没有激活标签时也设为不可点（R12 只规定后退 / 前进的不可点条件；没有标签时刷新本来就没有作用对象）。
- 2026-10-21：T9 第一次跑 `bun run e2e --grep "R11\\."` 时，R11.S1 在第一次启动的 `ready()` 处失败：Expected "Personal"，Received ""，整个用例只用了 851 ms（早于 3 秒等待超时，推测 Electron 进程提前退出），同一轮的 R11.S2 通过。之后直接跑 3 次、`--repeat-each 8`（24 次）、每次重新构建再跑 4 次，全部通过，没能复现。失败现场已被后续运行覆盖，根因未查明。处理：`e2e/app.ts` 收集每个 app 主进程的 stdout / stderr 与退出码，`sidebar.spec.ts` 的 `afterEach` 在用例失败时打印，便于再次出现时定位。验证阶段要再观察；如果复现，按 debugging.md 走根因分析，不当成通过。后续：若验证前仍未复现，把它记为已知的待观察问题。
- 2026-10-21：T10 把 `shell.rs` 的 `Daemon` / `Client` 等辅助抽到 `crates/alterd/tests/common/mod.rs`，供 `shell.rs` 与 `shell_perf.rs` 共用；`common` 里加了 `#![allow(dead_code)]`，因为每个测试 crate 只用其中一部分。SC2 的 100 个标签不经 UI 打开，而是直接拉起 alterd、经 socket 发事件准备数据，避免第一次启动时建 100 个视图。
- 2026-10-21：T12 按 design.md 改回 `broadcast`（容量 256）逐份发送快照，`watch` 只保存当前快照给新连接作第一条；新连接先订阅再取当前值，最多重复收到一份。T1 那条「用 watch 代替 broadcast」的偏差就此撤销。
- 2026-10-21：T15 为显示 favicon 把侧边栏页面的 CSP 放宽为 `img-src 'self' http: https: data:`：只允许加载图片，脚本、样式等仍限于 `'self'`。侧边栏页面持有 preload API，但图片资源不能执行代码。
- 2026-10-21：用户接受 T9 那次未复现的偶发失败为已知待观察问题：此后专门重跑 30 余次、全量 e2e 8 轮以上均未再出现；`dumpLogsOnFailure` 已就绪，再出现时按 debugging.md 查根因。后续：观察首次启动时 Electron 提前退出的可能。
- 2026-10-21：第一次验证（codex，HEAD 36cf26a）结论 NOT READY：其沙箱禁止监听 Unix socket，`cargo test` 在 lifecycle 中断（8 failed，`监听 shell.sock 失败: Operation not permitted`），属于环境限制；另报两处规格差异（主机名在 `?` `#` 前截取、带 tabId 时不校验 spaceId），转为收敛任务 T23、T24。后续验证中 `cargo test && bun test` 由主会话在本机运行。
- 2026-10-21：T19–T22 合在一个提交（e5bf0cb）里，没有按任务逐个提交；证据仍按任务分别记录。
- 2026-10-21：评审第 2 轮：CR3 的剩余部分（慢连接溢出）与 CR9（blob 弹窗）由用户选择「改 R2，写明过载时可跳过旧快照」「R5 增加 blob: 原样使用」，选项里写明了具体的规格与测试改动，按此授权修订到 revision 4（R2 正文与新场景 R2.S4、R5 正文与新场景 R5.S6、新场景 R14.S2），新增任务 T19–T22。
- 2026-10-21：后续：favicon 改为经所属 Space 的 session 获取（CR10），与 add-space-management 的多 Space 一起做。
- 2026-10-21：评审第 1 轮的 CR2（网页弹窗）由用户选择「在同一 Space 新建标签」，走 update 到 revision 3（新增需求 R14 与场景 R14.S1、任务 T18）。
- 2026-10-21：执行 T1 前发现 protocol 现行规格写死 14 个错误码，走 update 到 revision 2（MODIFIED「协议类型导出」+ R13.S2），用户已再批准。

## 代码评审
### Round 1
- Revision: 2
- Scope: alter `git diff 31aafee...e89fe7f`（origin/main...HEAD，不含 .dev-flow 与 lock 文件，无未提交改动）；reviewer: codex（`codex exec --sandbox read-only`），主会话逐条复核
- Tasks: T1-T10
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR1 | major | correctness | apps/browser/src/main/tab-views.ts:67 | `did-navigate-in-page` 对 iframe 也会触发，回调没看 `isMainFrame` 就把 URL 回写为标签地址；iframe 改 hash 后数据库存成子框架地址，重启恢复打开错误页面（R11） | T11 |
| CR2 | major | spec | apps/browser/src/main/tab-views.ts:55 | 页面视图没有 `setWindowOpenHandler`，`target=_blank` / `window.open` 走 Electron 默认行为弹出独立 BrowserWindow，不经 TabService、不落库、不恢复，引入了范围外的多窗口 | T18 |
| CR3 | major | spec | crates/alterd/src/tabs/mod.rs:151 | T1 把快照分发从设计的 `broadcast` 改成 `watch`，连续两个事件被快速处理时连接只收到最后一份快照，违反 R2「每个事件改变状态后各发一条快照」（第 2 轮：连续事件已由 T12 修复；慢连接积压超过广播容量时仍会丢旧快照，按用户选择修订 R2 并补测试） | T19 |
| CR4 | major | tests | apps/browser/e2e/sidebar.spec.ts:282 | SC2 在渲染进程观察到 running 之后才开始计时，恢复若在此之前完成会量出接近 0 的耗时，现有 59 ms / 14 ms 不能证明 SC2 | T13 |
| CR5 | major | tests | crates/alterd/tests/shell.rs:271 | R10.S1 的用例里 C 恰好是最近激活的标签，删掉「激活相邻标签」的实现测试仍通过，不能证明「下方相邻优先」 | T14 |
| CR6 | minor | spec | apps/browser/src/renderer/src/Sidebar.tsx:33 | 「变更内容」写了 favicon 回写到侧边栏，但标签行不渲染 `faviconUrl` | T15 |
| CR7 | minor | spec | apps/browser/src/renderer/src/App.tsx:39 | `shell.error` 进入 `ShellView.error` 后没有任何界面显示，与设计时序图「地址框下方提示错误」不符 | T16 |
| CR8 | minor | spec | apps/browser/src/shared/shell.ts:34 | IPC 通道与载荷和 design.md「接口契约」表不一致（`tab.*` 载荷应为 `{tabId}`，导航应为 `nav.back` / `nav.forward` / `nav.reload` 三个通道） | T17 |

Review: CHANGES REQUESTED
<!-- 由评审步骤填写，每次评审一轮：
### Round <n>
- Revision: <当前 revision>
- Scope: <repo> `git diff <base>...HEAD`（含未提交的改动）
- Tasks: T1, T2, …（这一轮读过代码的任务；收敛任务和重新打开的任务需要新的一轮）
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
最后恰好一行 "Review: PASS" 或 "Review: CHANGES REQUESTED"。语法见 references/formats.md -->

### Round 2
- Revision: 3
- Scope: alter `git diff e89fe7f...509fd2c`（第 1 轮之后的改动，不含 .dev-flow 与 lock 文件，无未提交改动）；reviewer: codex（`codex exec --sandbox read-only`），主会话逐条复核
- Tasks: T11-T18
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR9 | major | correctness | apps/browser/src/main/tab-views.ts:77 | 弹窗的已解析 URL 又被当成地址框输入规范化，`window.open(URL.createObjectURL(blob))` 的 `blob:https://…` 被转成 Google 搜索，违反 R14「打开该 URL」 | T21 |
| CR10 | minor | correctness | apps/browser/src/renderer/src/Sidebar.tsx:35 | favicon 由侧边栏页面用默认 session 加载，不带所属 Space partition 的 Cookie，需要凭证的图标会加载失败 | accepted: 绝大多数 favicon 不需要凭证；按 Space 的 session 取图需要 Main 代为请求并转成 data URL，放到引入多 Space 的 add-space-management 一起做（已记为后续） |
| CR11 | minor | tests | apps/browser/e2e/sidebar.spec.ts:337 | CR6 用例只断言 `src`，图片被 CSP 拦截或加载失败时仍通过，撤销 CSP 修改也测不出 | T22 |

Review: CHANGES REQUESTED

### Round 3
- Revision: 4
- Scope: alter `git diff 509fd2c...e5bf0cb`（T19–T22 的改动，不含 .dev-flow 与 lock 文件，无未提交改动），并复核 T11–T18（第 2 轮读过的代码，本轮只有 `url.rs` 因 T20 变化）；reviewer: codex（`codex exec --sandbox read-only`），主会话复核
- Tasks: T11-T22
Findings: none.

Review: PASS

### Round 4
- Revision: 4
- Scope: alter `git diff 36cf26a...0eb926b`（T23、T24 的代码与 design.md 改动，无未提交改动）；reviewer: codex（`codex exec --sandbox read-only`），主会话复核
- Tasks: T23, T24
Findings: none.

Review: PASS

## 验证报告
- Revision: 4
- Review: Round 4
- Verifier: 主会话（本机 macOS，Darwin 25.6.0，HEAD 2319e91）。配置的验证者 codex 第一次验证时沙箱禁止监听 Unix socket，`cargo test` 无法跑完，按 principles 的「调用失败」处理由主会话完成本轮；codex 那次的 lint / typecheck / build 结果与场景对照作为参考

| Repo | Command | Result |
|---|---|---|
| alter | `cargo test && bun test` | PASS （exit 0；Rust 39 passed、0 failed；bun 1 pass、0 fail） |
| alter | `cargo fmt --check && cargo clippy --all-targets -- -D warnings && bun run lint` | PASS （exit 0） |
| alter | `bun run typecheck` | PASS （exit 0） |
| alter | `cargo build && bun run build` | PASS （exit 0） |
| alter | `bun run e2e` | PASS （23 passed） |

| Scenario | Test | Result |
|---|---|---|
| R1.S1 | crates/alterd/tests/shell.rs::r1_s1_new_connection_receives_snapshot | PASS |
| R1.S2 | crates/alterd/tests/shell.rs::r1_s2_socket_mode_is_0600 | PASS |
| R2.S1 | crates/alterd/tests/shell.rs::r2_s1_both_connections_receive_change | PASS |
| R2.S2 | crates/alterd/tests/shell.rs::r2_s2_malformed_line_is_dropped | PASS |
| R2.S3 | crates/alterd/tests/shell.rs::r2_s3_rejection_goes_to_sender_only | PASS |
| R2.S4 | crates/alterd/tests/shell.rs::r2_s4_slow_connection_still_gets_latest | PASS |
| R3.S1 | crates/alterd/tests/shell.rs::r3_s1_socket_path_too_long | PASS |
| R4.S1 | crates/alterd/tests/shell.rs::r4_s1_restart_keeps_single_default_space | PASS |
| R4.S2 | apps/browser/e2e/sidebar.spec.ts::R4.S2 侧边栏显示默认 Space | PASS |
| R5.S1 | crates/alterd/tests/shell.rs::r5_s1_url_with_scheme_is_kept | PASS |
| R5.S2 | crates/alterd/tests/shell.rs::r5_s2_bare_domain_gets_https | PASS |
| R5.S3 | crates/alterd/tests/shell.rs::r5_s3_localhost_and_ip_get_http | PASS |
| R5.S4 | crates/alterd/tests/shell.rs::r5_s4_other_input_becomes_google_search | PASS |
| R5.S5 | crates/alterd/tests/shell.rs::r5_s5_blank_input_is_rejected | PASS |
| R5.S6 | crates/alterd/tests/shell.rs::r5_s6_blob_url_is_kept | PASS |
| R6.S1 | apps/browser/e2e/sidebar.spec.ts::R6.S1 从侧边栏新建标签 | PASS |
| R7.S1 | apps/browser/e2e/sidebar.spec.ts::R7.S1 当前标签导航 | PASS |
| R7.S2 | crates/alterd/tests/shell.rs::r7_s2_open_in_tab_sends_navigate | PASS |
| R8.S1 | apps/browser/e2e/sidebar.spec.ts::R8.S1 标题显示并落库 | PASS |
| R9.S1 | apps/browser/e2e/sidebar.spec.ts::R9.S1 点击切换 | PASS |
| R10.S1 | crates/alterd/tests/shell.rs::r10_s1_close_active_middle_tab_activates_next | PASS |
| R10.S2 | apps/browser/e2e/sidebar.spec.ts::R10.S2 关闭最后一个标签 | PASS |
| R11.S1 | apps/browser/e2e/sidebar.spec.ts::R11.S1 顺序与激活标签恢复 | PASS |
| R11.S2 | apps/browser/e2e/sidebar.spec.ts::R11.S2 未激活的标签不预先加载 | PASS |
| R12.S1 | apps/browser/e2e/sidebar.spec.ts::R12.S1 后退与前进 | PASS |
| R12.S2 | apps/browser/e2e/sidebar.spec.ts::R12.S2 刷新 | PASS |
| R13.S1 | crates/alter-core/tests/export_ts.rs::r13_s1_export_error_code_and_health_file | PASS |
| R13.S2 | crates/alter-core/tests/export_ts.rs::r13_s2_export_shell_messages | PASS |
| R14.S1 | apps/browser/e2e/sidebar.spec.ts::R14.S1 点击新窗口链接 | PASS |
| R14.S2 | apps/browser/e2e/sidebar.spec.ts::R14.S2 window.open 打开 blob 页面 | PASS |

| SC | Check | Result |
|---|---|---|
| SC1 | `cargo test -p alterd --release --test shell_perf -- --ignored --nocapture` → p50 1.050042 ms，p95 1.159584 ms | PASS （< 10 ms） |
| SC2 | `bun run e2e --grep SC2` → alterd.json 写出到 100 行 112 ms（此前 3 轮 94–152 ms） | PASS （< 1000 ms） |

宪章与非功能需求：只有 alterd 写库（TabService 独占 `Store`）；Electron Main 不开数据库、不发 CDP；Renderer 只经 preload 的 `window.alter` 走 IPC；`shell.sock` 由 `umask(0o177)` 创建为 0600（R1.S2）；标签视图 `sandbox: true`、无 preload；被拒绝的事件与无法解析的行记 WARN；关闭标签只置 `archived_at`；没有迁移。新依赖（`ulid`、tokio `net`/`io-util`、`libc` 正式依赖、`@alter/protocol` workspace 依赖）在 design.md 宪章检查里有理由。

Gaps: none。已接受：CR10（favicon 用默认 session，记为后续）；T9 偶发失败由用户接受为已知待观察问题。

Overall: READY

