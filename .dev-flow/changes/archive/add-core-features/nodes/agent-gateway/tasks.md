---
change: add-core-features
---

# 执行文档：agent 网关、CDP 与脚本运行时

<!-- 行语法：- [ ] T<n> [covers:R1.S1] [after:T<k>] [P] <做什么> — seam: <测试驱动的公开入口> — verify: <命令>
对应的证据写进 log.md 之后才能勾选 [x]。完整语法见 references/formats.md -->

## Repo: alter

- [x] T1 spike：在 Electron 44 上确定标签 ↔ CDP target 的对应方案 — verify: `bun run e2e --grep "N2 spike"`
  - 写一个临时 e2e `apps/browser/e2e/spike-targets.spec.ts`：开 remote-debugging 端口，建 100 个标签，分别试 design.md「风险」里的方案 A 和 B，统计对应正确率；再验证页面重载后、`webContents.forcefullyCrashRenderer()` 后对应是否仍然正确。
  - 产出：在 design.md「风险与权衡」写明选哪个方案、实测数据；选 B 时同时在「决策」表写明对「只有 alterd 发 CDP」的例外及其边界（只调用 `Target.getTargetInfo`，用完立即 detach）。证据用 VERIFY，spike 测试文件在 T9 合入正式实现后删除。
- [x] T2 [covers:R23.S1,R23.S2,R23.S3] [after:T1] 协议：Gateway 类型、`cdpReady`、快照新字段 — seam: `export-ts`（`crates/alter-core/tests/export_ts.rs`） — verify: `cargo test -p alter-core --test export_ts`
  - 新文件 `crates/alter-core/src/protocol/gateway.rs`：`GatewayFile`、`HelloParams`、`HelloResult`、`RpcErrorData`、`Receipt`、`Popup`，以及各方法的参数 / 结果结构（字段照 design.md 与 infra/api.md）。
  - `shell.rs`：`ShellEvent::CdpReady { port: u16 }`；`TabView` 加 `task_space_id: Option<i64>`、`label: Option<String>`；`ShellState` 加 `task_spaces`。
  - 跑 `bun run protocol:export` 并提交生成文件。
- [x] T3 [covers:R1.S1,R1.S2,R2.S1,R2.S2,R2.S3,R3.S1,R3.S2,R3.S3] [after:T2] Gateway：监听、`gateway.json`、鉴权、hello、错误格式 — seam: 真 alterd + WS 客户端（新文件 `crates/alterd/tests/gateway.rs`） — verify: `cargo test -p alterd --test gateway n2_`
  - 依赖：workspace 加 `tokio-tungstenite`（`connect` 与 `handshake` feature）、`getrandom`；alterd dev-dependencies 也加 tokio-tungstenite 作测试客户端。
  - token：`getrandom` 取 32 字节转十六进制；`gateway.json` 与 `alterd.json` 一样先写临时文件再 rename，再 `chmod 0600`；退出路径（SIGTERM、stdin EOF）都要删掉它，复用 `lifecycle.rs` 的清理函数。
  - R1.S1 的断言时机：读到 `alterd.json` 时 `gateway.json` 必须已在，所以在写 `alterd.json` 之前写 `gateway.json`。
  - 测试辅助放进 `crates/alterd/tests/common/mod.rs`：`gateway_client(home) -> (ws, token)`、`rpc(ws, method, params) -> Value`。
- [x] T4 [covers:R4.S1,R22.S1] [after:T3] `space.list` 与 `alter spaces list` — seam: Gateway WS；`alter` 二进制（`crates/alterd/tests/gateway.rs`、`crates/alter-cli/tests/cli.rs`） — verify: `cargo test -p alterd --test gateway n2_r4_ && cargo test -p alter-cli --test cli n2_r22_`
  - CLI 侧新文件 `crates/alter-cli/src/gateway.rs`：读 `gateway.json`，用同步 WS（tungstenite，不带 tokio）发一个请求；不要把 tokio 拉进 CLI，以免拖慢冷启动（SC1）。
  - CLI 测试要起一个真 alterd：在 `crates/alter-cli/tests/` 复用 alterd 测试里的 `Daemon` 辅助（复制到 `crates/alter-cli/tests/common/mod.rs`，用 `assert_cmd::cargo::cargo_bin("alterd")` 找不到时先 `cargo build -p alterd`）。
- [x] T5 [covers:R5.S1,R5.S2,R6.S1,R6.S2,R6.S3,R7.S1,R7.S2,R7.S3,R8.S1,R8.S2,R8.S3,R8.S4,R9.S1,R10.S1] [after:T3] TaskSpaceManager 的账目：创建、恢复、列出、新页、预算、关闭、托管、归还、结束、用户关标签 — seam: Gateway WS + Shell 假客户端（`crates/alterd/tests/gateway.rs`） — verify: `cargo test -p alterd --test gateway n2_r5_ n2_r6_ n2_r7_ n2_r8_ n2_r9_ n2_r10_`
  - 这些场景都不需要 Chromium：没有 CdpMux 连接时，账目照常更新；只有真正发 CDP 的方法才等待 target。
  - 仓储：`crates/alter-core/src/store/task_spaces.rs`。写操作走 TabService 的循环（design.md「架构」），TaskSpaceManager 经内部 `mpsc` 请求并等待 `oneshot` 回复。
  - 快照里 agent 标签的 `taskSpaceId` / `label` 来自 `task_pages` 中 `closed_at IS NULL` 的行；R5.S1 断言 A 仍为激活标签：插入 agent 标签时 `last_active_at` 取同 Space 最小值 − 1（不是最大值 + 1）。
  - R10.S1：`tabClosed` 归档标签时同步置 `task_pages.closed_at`；`page.info` 先查账目，`closed_at` 不为 NULL 就回 `E_PAGE_NOT_FOUND`，不碰 CDP。
- [x] T6 [covers:R20.S1,R20.S2,R20.S3] [after:T2] [P] `@alter/sdk`：客户端、TaskSpace / Page 对象、错误映射 — seam: SDK 公开 API 对假 Gateway（`packages/sdk/tests/sdk.test.ts`） — verify: `bun test packages/sdk/tests/sdk.test.ts`
  - 假 Gateway 用 `Bun.serve({ websocket })`，记录收到的每个请求，按测试设定回结果或错误。
  - `packages/sdk/src/`：`client.ts`（读 `ALTER_GATEWAY_FILE` 或 `ALTER_HOME/gateway.json`，`new WebSocket(url, { headers })`）、`errors.ts`（`AlterError`）、`task-space.ts`、`page.ts`、`runtime.ts`（把 `taskSpace` 等挂到 `globalThis`）、`index.ts`。
  - 方法清单照 infra/api.md 末尾「SDK」一段；`handOff` / `waitForControl` / `takeOverTaskSpace` 在本节点只发请求、不实现等待逻辑（N3 补）。
  - 独立性：只写 `packages/sdk/`，与 T3–T5 的 Rust 文件不重叠。
- [x] T7 [covers:R21.S1,R21.S2,R21.S3,R21.S4] [after:T5,T6] `alter nodejs` — seam: `alter` 二进制（新文件 `crates/alter-cli/tests/nodejs.rs`） — verify: `cargo test -p alter-cli --test nodejs`
  - `crates/alter-cli/src/nodejs.rs`：Bun 与 runtime 的查找顺序见 design.md「架构」；脚本写到临时文件（`tempfile`），`bun --preload <runtime> <file>`，转发退出码；没有 `gateway.json` 时不启动 Bun，直接以 2 退出并提示「请先打开 alter.app」。
  - 测试环境：`ALTER_BUN_PATH` 指向 PATH 里的 bun（`which bun`）；R21.S1 / S2 只需要 alterd，不需要 Electron。
  - 冷启动：`--version` 路径不得加载 nodejs 模块里的任何重活（SC1 在 T14 测）。
- [x] T8 [covers:R5.S3] [after:T1,T5] Electron：remote-debugging 端口、`cdpReady`、agent 离屏视图与侧边栏标记；CdpMux 连接与 target 认领 — seam: Electron e2e（新文件 `apps/browser/e2e/agent.spec.ts`，经 `alter nodejs` 驱动） — verify: `bun run e2e --grep "N2 R5\."`
  - e2e 辅助 `apps/browser/e2e/agent.ts`：`runScript(home, code)` 用 `target/debug/alter nodejs -e` 运行脚本（`ALTER_HOME` 同 app，`ALTER_BUN_PATH` 取 PATH 里的 bun），返回 `{stdout, stderr, code}`；构建时 `bun run e2e` 先 `cargo build -q -p alterd -p alter-cli`。
  - `crates/alterd/src/cdp/conn.rs` 与 `targets.rs` 按 T1 的结论实现。
  - Sidebar 的 agent 标记：`data-testid="agent-badge"`，显示 `label`。
- [x] T9 [covers:R11.S1,R11.S2] [after:T8] `page.goto` / `reload` / `info` / `url` / `title`、回执与回执日志 — seam: Electron e2e（`apps/browser/e2e/agent.spec.ts`） — verify: `bun run e2e --grep "N2 R(11|19)\."`
  - 测试服务器加 `/delay`（5 秒后响应）。
  - 回执日志用 `tracing::info!(target: "receipt", task_space_id, label, method, duration_ms, ok, error_code)`，JSON 层会输出这些字段；e2e 从 `launchApp` 收集到的输出里找。
  - 删除 T1 的 spike 测试文件。
- [x] T10 [covers:R12.S1,R13.S1,R13.S2,R13.S3,R13.S4,R19.S1] [after:T9] snapshot 与元素动作 — seam: Electron e2e（`apps/browser/e2e/agent.spec.ts`） — verify: `bun run e2e --grep "N2 R1[23]\."`
  - 测试服务器加 `/form`、`/two-buttons`（见 requirement.md「术语」）。
  - snapshot：`Accessibility.getFullAXTree`，过滤 `ignored`；`@e<n>` 记 `backendDOMNodeId`，存在该 Page 的内存状态里。
  - 选择器：`text=` / `loc=role:` 用 `Accessibility.queryAXTree`；`loc=css:` / 原生 CSS / `xpath=` 在隔离 world 里 `Runtime.evaluate` 求匹配数与对象；匹配数在 `timeoutMs` 内轮询（每 100 ms）直到恰好 1 个，0 个超时报 `E_NO_MATCH`，大于 1 立即报 `E_AMBIGUOUS_SELECTOR` 并带 `details.count`。
  - 点击：`DOM.scrollIntoViewIfNeeded` → `DOM.getContentQuads` 取中心点 → `Input.dispatchMouseEvent`（pressed / released）；`fill`：聚焦、全选、`Input.insertText`。
- [x] T11 [covers:R14.S1,R15.S1,R15.S2,R15.S3,R15.S4] [after:T10] 对话框、截图、evaluate、waitFor — seam: Electron e2e（`apps/browser/e2e/agent.spec.ts`） — verify: `bun run e2e --grep "N2 R1[45]\."`
  - 测试服务器加 `/alert`。对话框状态来自 `Page.javascriptDialogOpening` / `Closed` 事件；有对话框时元素动作立即回 `E_DIALOG_OPEN`，`details.message` 为对话框文本。
  - 截图：`Page.captureScreenshot`，路径必须是绝对路径，否则 `E_INVALID_ARGUMENT`；写文件由 alterd 完成。
- [x] T12 [covers:R16.S1,R16.S2,R17.S1] [after:T11] 弹窗、下载与 CDP 透传 — seam: Electron e2e（`apps/browser/e2e/agent.spec.ts`） — verify: `bun run e2e --grep "N2 R1[67]\."`
  - 测试服务器加 `/links`、`/download`。
  - 弹窗：Electron 的 `setWindowOpenHandler` 已把新窗口转成 `openRequested`；alterd 识别来源标签属于某个 TaskSpace 时，把新标签记为下一个 Page（`opened_by = agent`），并在触发它的那个动作的回执里返回。归属判断用 `openRequested` 新增的可选字段 `openerTabId`（Electron 填写），这是 Shell 协议的增量，同步写进 design.md「接口契约」。
  - 下载：`Browser.setDownloadBehavior {behavior: "allowAndName", downloadPath}` 加 `Browser.downloadProgress` 事件；完成后把文件改名为建议文件名；目录取 `ALTER_DOWNLOAD_DIR`，然后是 config.toml 的 `download_dir`，最后 `~/Downloads`。
- [x] T13 [covers:R18.S1,R18.S2] [after:T8] CDP 转发端点 — seam: Electron e2e，Playwright `chromium.connectOverCDP`（`apps/browser/e2e/cdp-forward.spec.ts`） — verify: `bun run e2e --grep "N2 R18\."`
  - `crates/alterd/src/gateway/forward.rs`：HTTP 请求先校验 token；`/json/version` 改写 `webSocketDebuggerUrl` 的主机端口；WS 升级后双向拷贝帧。Chromium 端口还没就绪（没收到 `cdpReady`）时返回 503。
- [x] T14 [after:T7,T10] 性能基准：CLI 冷启动、网关开销、snapshot（父级 SC1、SC2、SC3） — verify: `cargo test -p alter-cli --release --test perf -- --ignored --nocapture && cargo test -p alterd --release --test gateway_perf -- --ignored --nocapture && bun run e2e --grep SC3`
  - SC1：`crates/alter-cli/tests/perf.rs`，`#[ignore]`，跑 100 次 `alter --version` 取中位数，> 20 ms 断言失败。
  - SC2：`crates/alterd/tests/gateway_perf.rs`，`#[ignore]`；用一个假 CDP 端（本地 WS 服务）代替 Chromium，记录「收到 `page.click` 请求」到「假 CDP 端收到 `Input.dispatchMouseEvent`」的时间（alterd 在 debug 日志里打两个时间点），1000 次的 p95 < 5 ms。
  - SC3：测试服务器加 `/big`（2000 个节点），`apps/browser/e2e/perf.spec.ts` 里 10 次 `page.snapshot` 的 p95 < 150 ms。

## Convergence

- [x] T15 [after:T14] 补齐 `taskSpace.cdp`（浏览器级透传）、`page.mouse`、`page.keyboard`、`page.fetch`，SDK 加 `page.mouse` / `page.keyboard` / `page.fetch`（CR1） — seam: Gateway WS + 假 CDP 端（`crates/alterd/tests/gateway.rs`，假 CDP 端抽到 `tests/common/fake_cdp.rs`）；SDK 对假 Gateway（`packages/sdk/tests/sdk.test.ts`） — verify: `cargo test -p alterd --test gateway n2_cr1_ && bun test packages/sdk`
  - 假 CDP 端记录收到的每条命令；断言 `page.mouse {op:"click"}` 发出 mousePressed / mouseReleased、`page.keyboard {op:"type"}` 发出 `Input.insertText`、`taskSpace.cdp` 以浏览器会话（无 sessionId）发出、`page.fetch` 在页面里 `Runtime.evaluate`（`awaitPromise`）并按 `saveAs` 写文件（相对路径回 `E_INVALID_ARGUMENT`）。
- [x] T16 [after:T14] `taskSpace.newPage` / `adopt` / `release` / `finish` 与 `page.close` 进入该 TaskSpace 的串行队列（CR2） — seam: Gateway WS + 假 CDP 端（`crates/alterd/tests/gateway.rs`） — verify: `cargo test -p alterd --test gateway n2_cr2_`
  - 假 CDP 端对 `Page.navigate` 返回 loaderId 但不发生命周期事件；连接 A `page.goto {timeoutMs:500}`，100 ms 后连接 B `taskSpace.finish`：B 的响应晚于 A 的 `E_TIMEOUT`。
- [x] T17 [after:T14] `waitFor({fn})` 先等函数结果（含 Promise）再判真假（CR3） — seam: Electron e2e（`apps/browser/e2e/agent.spec.ts`） — verify: `bun run e2e --grep "N2 CR3 "`
- [x] T18 [after:T14] 连接结束时移除它的 listener，写队列随之关闭（CR4） — seam: `gateway` 模块单测（tokio，回环 TCP 上的真 WebSocket 握手） — verify: `cargo test -p alterd --bin alterd gateway::`
- [x] T19 [after:T16] `page.close` 经回执日志入口；成功的回执行带 `error_code: ""`（CR5） — seam: Electron e2e（`apps/browser/e2e/agent.spec.ts`） — verify: `bun run e2e --grep "N2 (CR5 |R19\.)"`
- [x] T20 [after:T17] `waitFor` 的每次 CDP 求值都受剩余期限约束，到期回 `E_TIMEOUT`；CdpMux 的调用方放弃等待时移除待响应请求（CR6） — seam: Electron e2e（`apps/browser/e2e/agent.spec.ts`） — verify: `bun run e2e --grep "N2 (CR3 |CR6 |R15\.S4)"`
- [x] T21 [after:T16] CR2 的测试等假 CDP 端收到 `Page.navigate` 后再发 `finish`，从该时点判断顺序（CR7） — seam: Gateway WS + 假 CDP 端（`crates/alterd/tests/gateway.rs`） — verify: `cargo test -p alterd --test gateway n2_cr2_`
- [x] T22 [after:T21] R5.S3 的 2 秒从创建动作开始计（验证缺口） — seam: Electron e2e（`apps/browser/e2e/agent.spec.ts`） — verify: `bun run e2e --grep "N2 R5\.S3"`
- [x] T23 [after:T21] `page.snapshot` 支持 `scope: "subtree"` + `root`（选择器）；`full_page` 为默认；`viewport` 回 `E_INVALID_ARGUMENT`，SDK 类型同步（验证缺口） — seam: Electron e2e（`apps/browser/e2e/agent.spec.ts`） — verify: `bun run e2e --grep "N2 (V2 |R12\.)"`
- [x] T24 [after:T21] 元素动作接受 `actionLabel`，写进回执日志的 `action_label` 字段（验证缺口） — seam: Electron e2e（`apps/browser/e2e/agent.spec.ts`） — verify: `bun run e2e --grep "N2 (V3 |R19\.)"`
- [x] T25 [after:T21] design.md 收窄 `page.waitFor` 的条件为 url / selector / fn / ms，事件经 SDK 的 `waitForEvent`（验证缺口，文档修正） — seam: 无代码改动 — verify: `python3 -I ~/.claude/skills/dev-flow/scripts/dflow.py validate add-core-features`
- [x] T26 [after:T21] `page.reload` 按 `waitUntil` 等 `load` 或 `DOMContentLoaded`（验证缺口） — seam: Electron e2e（`apps/browser/e2e/agent.spec.ts`，`/slow-image` 测试页） — verify: `bun run e2e --grep "N2 V5 "`
- [x] T27 [after:T21] SDK：`page.mouse.*` / `page.keyboard.*` 对象方法、`TaskSpace.name`（`TaskSpaceInfo` 加 `name`）；`waitForFileChooser` 推迟并记录（验证缺口） — seam: SDK 对假 Gateway（`packages/sdk/tests/sdk.test.ts`）；Gateway WS（`crates/alterd/tests/gateway.rs`） — verify: `bun test packages/sdk && cargo test -p alterd --test gateway n2_v6_`
- [x] T28 [after:T21] `alter nodejs` 在 `gateway.json` 指向的端口连不上时以 2 退出（验证缺口） — seam: CLI 真二进制（`crates/alter-cli/tests/nodejs.rs`） — verify: `cargo test -p alter-cli --test nodejs n2_v7_`
- [x] T29 [after:T21] 补 R20.S1、R20.S3 的行为 RED：用变异（不带 Authorization 头、连接失败时不给出提示）确认测试失败（验证缺口，只补证据） — seam: SDK 对假 Gateway（`packages/sdk/tests/sdk.test.ts`） — verify: `bun test packages/sdk`
- [x] T30 [after:T22] R5.S3 用绝对截止时间：脚本与界面检查同时进行，全部满足的时刻不晚于截止时间（CR8） — seam: Electron e2e（`apps/browser/e2e/agent.spec.ts`） — verify: `bun run e2e --grep "N2 R5\.S3"`
- [x] T31 [after:T30] R5.S3 在脚本结束后也断言截止时间，计时改用单调时钟（CR9、CR10） — seam: Electron e2e（`apps/browser/e2e/agent.spec.ts`） — verify: `bun run e2e --grep "N2 R5\.S3"`
