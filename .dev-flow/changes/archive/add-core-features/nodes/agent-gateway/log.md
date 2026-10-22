---
change: add-core-features
---

# 执行日志：agent 网关、CDP 与脚本运行时

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

### T1 spike：在 Electron 44 上确定标签 ↔ CDP target 的对应方案
- Revision: 1
- VERIFY: `ALTER_E2E_ARGS="--remote-debugging-port=0 --remote-debugging-address=127.0.0.1" npx playwright test -c e2e/playwright.config.ts --grep "N2 spike"` → 1 passed。100 个离屏视图：初始、重载 10 个、崩溃 5 个并重新加载后，A / B / C 都是 100/100；2 个导航到相同 URL 后 A、B（按 URL 判定）98/100，C 100/100；C 读到的 id 与 B 完全一致
- 结论：选方案 C（`webContents.getOrCreateDevToolsTargetId()`，spike 中发现，tasks.md 里原本只列了 A、B），不需要对「只有 alterd 发 CDP」开例外；design.md「风险与权衡」「决策」「接口契约」与时序图已写入

### T2 协议：Gateway 类型、`cdpReady`、快照新字段
- Revision: 1
- RED: `cargo test -q -p alter-core --test export_ts` → FAIL: `n2_r23_s2_export_shell_messages` ShellEvent 字面量缺 `shell.event.cdpReady` (R23.S2)；`n2_r23_s3_export_gateway_types` panicked at export_ts.rs:48（读取 GatewayFile.ts 失败）(R23.S3)
- GREEN: `cargo test -q -p alter-core --test export_ts` → PASS (4 tests)。新文件 `protocol/gateway.rs`（`GatewayFile`、`HelloParams` / `HelloResult`、`RpcErrorData`、`SpaceListResult`、`Ownership`、`TaskSpaceStatus`、`TaskSpaceInfo` / `Summary` / `Tab`、`Popup`、`DialogInfo`、`Receipt`，常量 `PROTOCOL_VERSION`、`DEFAULT_PAGE_BUDGET`）；`ShellEvent::CdpReady`、`tabUpdated.targetId`、`TabView.taskSpaceId` / `label`、`ShellState.taskSpaces`、`TaskSpaceView` (R23.S2, R23.S3)
- RED (mutation: 注释掉 `HealthFile::export_all`；N1 的 `n1_r16_s1_*` 改名为 `n2_r23_s1_*`，内容未变): `cargo test -q -p alter-core --test export_ts n2_r23_s1` → FAIL: panicked at export_ts.rs:48 (R23.S1)
- GREEN: 恢复后 `cargo test -q -p alter-core --test export_ts` → PASS (4 tests) (R23.S1)
- VERIFY: `bun run protocol:export` 生成 16 个新类型文件并在 `packages/protocol/src/index.ts` 再导出；alterd 最小改动使其编译：`tabUpdated.targetId` 记入 TabService 内存表，`cdpReady` 记端口，快照新字段先为空（T5、T8 实现）；Main 拼快照时带上 `taskSpaces`
- VERIFY (Widen): `cargo test --no-fail-fast` → cli 4、alter-core 1+4+7、lifecycle 8、shell 52 全部 PASS；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过

### T3 Gateway：监听、`gateway.json`、鉴权、hello、错误格式
- Revision: 1
- RED: `cargo test -q -p alterd --test gateway n2_` → 8 条全部 FAIL: panicked at tests/common/mod.rs:248 「应当有 gateway.json: NotFound」（还没有 Gateway）(R1.S1, R1.S2, R2.S1, R2.S2, R2.S3, R3.S1, R3.S2, R3.S3)
- GREEN: `cargo test -q -p alterd --test gateway` → PASS (8 tests)。`alter_core::gateway_file`（0600 原子写、读、删）；`alterd::gateway`（tokio-tungstenite，握手回调校验 `/rpc` 路径与 `Authorization: Bearer <token>`，不通过回 401；每条请求一个任务，响应经连接的写出队列）；`gateway::rpc`（JSON-RPC 2.0，-32700 / -32600 / -32601 / -32602 / -32000，`error.data` 为 `RpcErrorData`；`hello` 校验协议版本，空间列表经 TabService 取）；TabService 新增 `Request::Gateway(GatewayJob)`（在单写者循环里执行闭包，返回是否需要发布快照）；lifecycle 在写 `alterd.json` 之前绑定 Gateway 与 CDP 转发端口（后者 T13 实现，先只接受连接）、写 `gateway.json`，退出时删除；token 由 `getrandom` 取 32 字节 (R1.S1, R1.S2, R2.S1, R2.S2, R2.S3, R3.S1, R3.S2, R3.S3)
- RED (mutation: 握手条件 `path == "/rpc" && token_ok` 改为 `||`): `cargo test -q -p alterd --test gateway n2_r2_` → FAIL: left: None right: Some(401) (R2.S1, R2.S2)
- RED (mutation: 退出时不删 gateway.json): `cargo test -q -p alterd --test gateway n2_r1_s2` → FAIL: panicked at tests/gateway.rs:44 (R1.S2)
- GREEN: 恢复后 `cargo test -q -p alterd --test gateway` → PASS (8 tests) (R1.S2, R2.S1, R2.S2)
- VERIFY (Widen): `cargo test --no-fail-fast` → cli 4、alter-core 1+4+7、gateway 8、lifecycle 8、shell 52 全部 PASS；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`（握手回调签名由 tungstenite 规定，`result_large_err` 就地 allow）、`bun test`、`bun run lint`、`bun run typecheck` 通过

### T4 `space.list` 与 `alter spaces list`
- Revision: 1
- RED: `cargo test -q -p alterd --test gateway n2_r4_` → FAIL: 响应 error -32601「没有这个方法：space.list」(R4.S1)
- RED: `cargo test -q -p alter-cli --test cli n2_r22_` → FAIL: code=2，`error: unrecognized subcommand 'spaces'` (R22.S1)
- GREEN: `cargo test -q -p alterd --test gateway n2_r4_` → PASS，`space.list` 返回 `SpaceListResult`（TabService 的 `space_summaries`）(R4.S1)
- GREEN: `cargo test -q -p alter-cli --test cli` → PASS (5 tests)，新文件 `alter-cli/src/gateway.rs`（同步 tungstenite，读 `gateway.json`，连不上为退出码 2「无法连接 alterd，请先打开 alter.app」）与 `spaces.rs`；测试辅助 `alter-cli/tests/common/mod.rs` 起真实 alterd（与 `alter` 同目录，缺时先 `cargo build -p alterd`）(R22.S1)
- RED (mutation: `tab_count` 恒为 0): `cargo test -q -p alterd --test gateway n2_r4_` → FAIL: left: [("Personal", 0), ("Work", 0)] right: [("Personal", 2), ("Work", 0)]；恢复后 PASS (R4.S1)
- VERIFY (Widen): `cargo test --no-fail-fast` → cli 5、alter-core 1+4+7、gateway 9、lifecycle 8、shell 52 全部 PASS；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 通过

### T5 TaskSpaceManager 的账目
- Revision: 1
- RED: `cargo test -q -p alterd --test gateway n2_r` → FAIL（14 条新用例全部失败：`taskSpace.create` 回「尚未实现」，其余方法 -32601 没有这个方法）(R5.S1)
- RED: `cargo test -q -p alterd --test gateway n2_r` → FAIL（14 条新用例全部失败：`taskSpace.create` 回「尚未实现」，其余方法 -32601 没有这个方法）(R5.S2)
- RED: `cargo test -q -p alterd --test gateway n2_r` → FAIL（14 条新用例全部失败：`taskSpace.create` 回「尚未实现」，其余方法 -32601 没有这个方法）(R6.S1)
- RED: `cargo test -q -p alterd --test gateway n2_r` → FAIL（14 条新用例全部失败：`taskSpace.create` 回「尚未实现」，其余方法 -32601 没有这个方法）(R6.S2)
- RED: `cargo test -q -p alterd --test gateway n2_r` → FAIL（14 条新用例全部失败：`taskSpace.create` 回「尚未实现」，其余方法 -32601 没有这个方法）(R6.S3)
- RED: `cargo test -q -p alterd --test gateway n2_r` → FAIL（14 条新用例全部失败：`taskSpace.create` 回「尚未实现」，其余方法 -32601 没有这个方法）(R7.S1)
- RED: `cargo test -q -p alterd --test gateway n2_r` → FAIL（14 条新用例全部失败：`taskSpace.create` 回「尚未实现」，其余方法 -32601 没有这个方法）(R7.S2)
- RED: `cargo test -q -p alterd --test gateway n2_r` → FAIL（14 条新用例全部失败：`taskSpace.create` 回「尚未实现」，其余方法 -32601 没有这个方法）(R7.S3)
- RED: `cargo test -q -p alterd --test gateway n2_r` → FAIL（14 条新用例全部失败：`taskSpace.create` 回「尚未实现」，其余方法 -32601 没有这个方法）(R8.S1)
- RED: `cargo test -q -p alterd --test gateway n2_r` → FAIL（14 条新用例全部失败：`taskSpace.create` 回「尚未实现」，其余方法 -32601 没有这个方法）(R8.S2)
- RED: `cargo test -q -p alterd --test gateway n2_r` → FAIL（14 条新用例全部失败：`taskSpace.create` 回「尚未实现」，其余方法 -32601 没有这个方法）(R8.S3)
- RED: `cargo test -q -p alterd --test gateway n2_r` → FAIL（14 条新用例全部失败：`taskSpace.create` 回「尚未实现」，其余方法 -32601 没有这个方法）(R8.S4)
- RED: `cargo test -q -p alterd --test gateway n2_r` → FAIL（14 条新用例全部失败：`taskSpace.create` 回「尚未实现」，其余方法 -32601 没有这个方法）(R9.S1)
- RED: `cargo test -q -p alterd --test gateway n2_r` → FAIL（14 条新用例全部失败：`taskSpace.create` 回「尚未实现」，其余方法 -32601 没有这个方法）(R10.S1)
- 实现：仓储 `alter-core/src/store/task_spaces.rs`（TaskSpace 增查与结束、Page 增删查、`next_page_label` 取已用最大编号 + 1、`insert_agent_tab` 插到 Space 末尾且 `last_active_at` 取同 Space 最小值 − 1、`close_pages_of_tab`）；TabService 子模块 `tabs/task_space.rs`（create / resume / list / newPage / tabs / adopt / release / finish / page.close / page.info，错误为 `OpError`，在单写者循环里执行）；`tabClosed` 归档标签时关闭托管它的 Page；快照填 `taskSpaceId` / `label` / `taskSpaces`，并把每个 Space 激活标签的计算由 O(n²) 改为一次遍历；Gateway 分发对应方法（`Gateway::op`）
- GREEN: `cargo test -q -p alterd --test gateway` → PASS (23 tests) (R5.S1)
- GREEN: `cargo test -q -p alterd --test gateway` → PASS (23 tests) (R5.S2)
- GREEN: `cargo test -q -p alterd --test gateway` → PASS (23 tests) (R6.S1)
- GREEN: `cargo test -q -p alterd --test gateway` → PASS (23 tests) (R6.S2)
- GREEN: `cargo test -q -p alterd --test gateway` → PASS (23 tests) (R6.S3)
- GREEN: `cargo test -q -p alterd --test gateway` → PASS (23 tests) (R7.S1)
- GREEN: `cargo test -q -p alterd --test gateway` → PASS (23 tests) (R7.S2)
- GREEN: `cargo test -q -p alterd --test gateway` → PASS (23 tests) (R7.S3)
- GREEN: `cargo test -q -p alterd --test gateway` → PASS (23 tests) (R8.S1)
- GREEN: `cargo test -q -p alterd --test gateway` → PASS (23 tests) (R8.S2)
- GREEN: `cargo test -q -p alterd --test gateway` → PASS (23 tests) (R8.S3)
- GREEN: `cargo test -q -p alterd --test gateway` → PASS (23 tests) (R8.S4)
- GREEN: `cargo test -q -p alterd --test gateway` → PASS (23 tests) (R9.S1)
- GREEN: `cargo test -q -p alterd --test gateway` → PASS (23 tests) (R10.S1)
- REFACTOR: `n2_r9_s1_*` 的等待条件改为「快照里出现 p3」（`newPage` 之后也有一份 3 个标签的快照，测试先前取错了）；`n2_r10_s1_*` 另断言 `taskSpace.resume` 的 `pages` 不再含 p1 → PASS
- RED (mutation: agent 标签的 `last_active_at` 取最大值 + 1): `cargo test -q -p alterd --test gateway n2_r5_s1` → FAIL: A 应当仍是激活标签 (R5.S1)
- RED (mutation: 下一个标签按未关闭页数 + 1): `cargo test -q -p alterd --test gateway n2_r7_s3` → FAIL (R7.S3)
- RED (mutation: `tabClosed` 不关闭 Page；先前版本的用例只断言 `page.info` 抓不到，加断言后): `cargo test -q -p alterd --test gateway n2_r10` → FAIL: left: ["p1"] right: [] (R10.S1)
- GREEN: 恢复后 `cargo test -q -p alterd --test gateway` → PASS (23 tests) (R5.S1, R7.S3, R10.S1)
- VERIFY (Widen): `cargo test --no-fail-fast` → cli 5、alter-core 1+4+7、gateway 23、lifecycle 8、shell 52 全部 PASS；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun test`、`bun run lint`、`bun run typecheck` 通过；`bun run e2e` → 52 passed（SC4 p95 13.6 ms，SC2 109 ms）。运行环境：本机 macOS，主会话

### T6 `@alter/sdk`：客户端、TaskSpace / Page 对象、错误映射
- Revision: 1
- RED: `bun test packages/sdk/tests/sdk.test.ts` → FAIL: SyntaxError: Export named 'createAlter' not found in module packages/sdk/src/index.ts（三条用例所在文件无法加载）(R20.S1, R20.S2, R20.S3)
- GREEN: `bun test packages/sdk` → 4 pass。`src/errors.ts`（`AlterError`，code / message / details）、`client.ts`（读 `ALTER_GATEWAY_FILE` 或 `<ALTER_HOME>/gateway.json`，带 Bearer 头连 `/rpc`，先 `hello` 核对协议版本；请求按 id 等待；通知分发给订阅者；没有未完成请求与订阅时空闲 20 ms 断开，脚本结束后进程能退出；连不上抛 `E_INTERNAL`「无法连接 alterd，请先打开 alter.app」）、`page.ts`、`task-space.ts`、`alter.ts`（`createAlter({gatewayFile})`）、`runtime.ts`（挂全局函数）、`index.ts` (R20.S1, R20.S2, R20.S3)
- RED (mutation: Page 的调用出错后自动重试一次): `bun test packages/sdk/tests/sdk.test.ts` → FAIL: Expected length: 1 Received length: 2 (R20.S2)
- GREEN: 恢复后 `bun test packages/sdk` → 4 pass (R20.S2)
- VERIFY: `bun run typecheck` 先报 `TaskSpaceInfo.id` 为 bigint：ts-rs 把 `i64` 映射为 bigint，而 JSON 里是数字；协议的 TaskSpace id 与 `TabView.taskSpaceId` 标注 `#[ts(type = "number")]` 后重新导出，typecheck 通过；`packages/sdk/package.json` 声明依赖 `@alter/protocol`
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS（gateway 23、shell 52）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun test`、`bun run lint`、`bun run typecheck` 通过

### T7 `alter nodejs`
- Revision: 1
- RED: `cargo test -q -p alter-cli --test nodejs` → 4 条 FAIL: code=Some(2) stderr「error: unrecognized subcommand 'nodejs'」（R21.S4 退出码碰巧也是 2，但 stderr 不含 alter.app）(R21.S1, R21.S2, R21.S3, R21.S4)
- GREEN: `cargo test -q -p alter-cli --test nodejs` → PASS (4 tests)。`alter-cli/src/nodejs.rs`：先查 `gateway.json`（没有则退出码 2 并提示 alter.app）；Bun 按 `ALTER_BUN_PATH` → 同目录 `bun` → PATH，runtime 按 `ALTER_SDK_RUNTIME` → 打包后的 `../sdk/runtime.js` → 仓库内 `packages/sdk/src/runtime.ts`；`-e` 或 stdin 的脚本写到临时目录的 `script.ts`，`bun --preload <runtime> <script>`，转发退出码，结束后删临时目录 (R21.S1, R21.S2, R21.S3, R21.S4)
- RED (mutation: 不论脚本结果都以 0 退出): `cargo test -q -p alter-cli --test nodejs n2_r21_s3` → FAIL: left: Some(0) right: Some(1) (R21.S3)
- GREEN: 恢复后 `cargo test -q -p alter-cli --test nodejs` → PASS (4 tests) (R21.S3)
- VERIFY (Widen): `cargo test --no-fail-fast` → cli 5、nodejs 4、alter-core 1+4+7、gateway 23、lifecycle 8、shell 52 全部 PASS；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 通过；`target/debug/alter --version` 约 5 ms（正式基准在 T14）

### T8 Electron：remote-debugging 端口、`cdpReady`、agent 离屏视图与侧边栏标记
- Revision: 1
- RED: `bun run e2e --grep "N2 R5\."` → FAIL: expect(locator).toHaveText(expected) Expected: "p1"，element(s) not found，waiting for getByTestId('agent-badge') (R5.S3)。第一次运行另有一次 `ready()` 3 秒内没等到 Space 名称的失败，之后两次都稳定落在 agent-badge 处，未再出现
- GREEN: `bun run e2e --grep "N2 R5\."` → 1 passed。Main：`remote-debugging-port=0`、`remote-debugging-address=127.0.0.1`，轮询 `userData/DevToolsActivePort` 取端口后经 `shell.event.cdpReady` 告诉 alterd（Shell 连上时再发一次）；`TabViews.apply` 为带 `taskSpaceId` 的标签总建视图（1280×800、关后台节流），每个新视图经 `tabUpdated.targetId` 上报 `getOrCreateDevToolsTargetId()`；Sidebar 的 agent 标签行带 `agent-badge`；`bun run e2e` 先构建 alterd 与 alter-cli；e2e 辅助 `e2e/agent.ts` 的 `runScript` (R5.S3)
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS（gateway 23、shell 52、nodejs 4）；`bun test` 4 pass；`bun run lint`、`bun run typecheck` 通过；`bun run e2e` → 53 passed（SC4 p95 15.1 ms，SC2 151 ms）。运行环境：本机 macOS，主会话

### T9 CdpMux、`page.goto` / `reload` / `info` / `url` / `title`、回执与回执日志
- Revision: 1
- RED: `bun run e2e --grep "N2 R11\."` → `N2 R11.S2` FAIL: Expected: "E_TIMEOUT" Received: "E_INVALID_ARGUMENT"（还没有 page.goto）(R11.S2)；`N2 R11.S1` 这一次失败在 `ready()`（构建后第一次启动 app 偏慢，3 秒内没等到 Space 名称），未到场景断言
- 处理：`e2e/ui.ts` 的 `ready()` 是准备步骤，等待放宽到 10 秒（场景断言的时限不变）
- GREEN: `bun run e2e --grep "N2 R(5|11)\."` → 3 passed。新模块 `alterd::cdp`（CdpMux：读 `DevToolsActivePort` 第二行取浏览器端点，连 WebSocket；响应按 id 分发、事件带 sessionId 广播；`session(targetId)` 用 `Target.attachToTarget {flatten}` 并开 Page / Runtime 域与生命周期事件，按 targetId 缓存，detach 时清掉）；TabService 收到 `cdpReady` 后让它连接；`gateway::page`（每个 TaskSpace 一把锁串行；`goto` 等与 `Page.navigate` 返回的 loaderId 相同的 `load` / `DOMContentLoaded` 生命周期事件，整体超过 `timeoutMs` 回 `E_TIMEOUT`；`reload`；`info` / `url` / `title` 连着 CDP 时在页面里读；每次调用写 `target: "receipt"` 日志）；测试服务器加 `/delay` (R11.S1, R11.S2)
- RED (mutation: goto 拿到 loaderId 后不等 load 直接返回): `bun run e2e --grep "N2 R11\.S1"` → FAIL: Expected: "Page A" Received: undefined (R11.S1)
- RED (mutation: 忽略 `timeoutMs`，至少等 30 秒): `bun run e2e --grep "N2 R11\.S2"` → FAIL: Expected: "E_TIMEOUT" Received: ""（6.7 s 后导航成功）(R11.S2)
- GREEN: 恢复后通过 (R11.S1, R11.S2)
- VERIFY: 删除 T1 的 spike 测试 `apps/browser/e2e/spike-targets.spec.ts`
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS（gateway 23、shell 52、nodejs 4）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun test`、`bun run lint`、`bun run typecheck` 通过；`bun run e2e` → 54 passed（SC4 p95 15.9 ms，SC2 111 ms）。运行环境：本机 macOS，主会话

### T10 snapshot、元素动作与回执日志
- Revision: 1
- RED: `bun run e2e --grep "N2 R(1[23]|19)\."` → 6 条 FAIL：R12.S1、R13.S1、R13.S2 脚本退出码 1（page.snapshot / page.fill / page.evaluate 还没有，-32601）；R13.S3 期望 {code: E_AMBIGUOUS_SELECTOR, details: {count: 2}} 得到 1 行不同；R13.S4 Expected "E_NO_MATCH" Received "E_INVALID_ARGUMENT"；R19.S1 没有 page.click 失败的回执行 (R12.S1, R13.S1, R13.S2, R13.S3, R13.S4, R19.S1)
- 实现：新模块 `gateway/actions.rs`：`snapshot`（`Accessibility.getFullAXTree`，跳过 ignored 与排版节点，可交互角色带 `@e<n>`，引用存进 `Gateway.refs`，名称规范化空白）；选择器（`@e` 走快照引用、`loc=role:` 用 `Accessibility.queryAXTree`、`text=` / CSS / `xpath=` 在页面里求匹配元素；每 100 ms 轮询，多于 1 个立即 `E_AMBIGUOUS_SELECTOR` 带 `count`，超时 `E_NO_MATCH`）；动作 click / dblclick / hover / focus / fill / press / selectOption / dragAndDrop / setInputFiles；`evaluate`；测试服务器加 `/form`、`/two-buttons`；`e2e/app.ts` 导出 `appLogs()`
- GREEN: `bun run e2e --grep "N2 R(1[23]|19)\."` → 第一次 R12.S1 失败（快照里名称是 "Email "，带尾随空白），名称规范化后 R12.S1 通过；R13.S2 失败在测试脚本自己的正则转义（反斜杠在 TS 模板字符串里被吃掉），改为按行查找后 6 条全部 PASS (R12.S1, R13.S1, R13.S2, R13.S3, R13.S4, R19.S1)
- RED (mutation: 匹配多个元素时取第一个): `bun run e2e --grep "N2 R13\.S3"` → FAIL (R13.S3)
- RED (mutation: 忽略快照引用): `bun run e2e --grep "N2 R13\.S2"` → FAIL（6.7 s，E_NO_MATCH）(R13.S2)
- RED (mutation: 失败调用的回执日志降为 debug): `bun run e2e --grep "N2 R19\.S1"` → FAIL (R19.S1)
- GREEN: 恢复后通过 (R13.S2, R13.S3, R19.S1)
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun test`、`bun run lint`、`bun run typecheck` 通过；`bun run e2e` → 60 passed（SC4 p95 15.4 ms，SC2 91 ms）。运行环境：本机 macOS，主会话

### T11 对话框、截图、evaluate、waitFor
- Revision: 1
- RED: `bun run e2e --grep "N2 R1[45]\."` → R14.S1、R15.S1、R15.S3、R15.S4 FAIL，R15.S2 PASS（还没有 page.screenshot，-32601 时 SDK 也给出 `E_INVALID_ARGUMENT`；实现后由 `path` 校验给出）。R14.S1 / R15.S1 脚本退出码 1（page.dialog / page.screenshot 未实现）；R15.S3 Expected "42" Received "42"（数字被 Bun 着色，见下）；R15.S4 Expected "E_TIMEOUT" Received "E_INVALID_ARGUMENT" (R14.S1, R15.S1, R15.S3, R15.S4)
- 测试修正：Playwright 的环境带 `FORCE_COLOR`，Bun 给 `console.log` 的数字加 ANSI 颜色；`e2e/agent.ts` 的 `runScript` 设 `FORCE_COLOR=0`、`NO_COLOR=1`
- 实现：`gateway/mod.rs` 跟踪 `Page.javascriptDialogOpening` / `Closed`（按会话）；`page.*` 除 `dialog` / `info` / `url` / `title` 外在对话框打开时立即回 `E_DIALOG_OPEN`，`details` 带 `type`、`message`（对话框开着时页面 JS 停住，`Runtime.evaluate` 会一直等）；`page.info` 带回 `dialog`；`goto` 在加载中弹出对话框时返回、回执带 `dialog`；`page.dialog`（`Page.handleJavaScriptDialog`）；`page.screenshot`（绝对路径校验，`Page.captureScreenshot`，从 PNG 头读宽高）；`page.waitFor`（url / selector / fn / ms，每 100 ms 轮询，超时 `E_TIMEOUT`）；页面会话在 app 刚启动、还没连上 Chromium 时也最多等 5 秒；测试服务器加 `/alert`
- 排查：R14.S1 第一次 GREEN 尝试失败于 `page.dialog` → `No dialog is showing`。原因是 Playwright 的 `_electron.launch` 附着了所有页面（包括各 Space partition 的 agent 视图），没人监听 `dialog` 时自动关掉对话框；`app.context().on("dialog")` 管不到其他 partition。`e2e/app.ts` 新增 `spawnApp`（不经 Playwright 启动，等 `gateway.json` 写出），R14.S1 改用它
- GREEN: `bun run e2e --grep "N2 R1[45]\."` → 5 passed (R14.S1, R15.S1, R15.S2, R15.S3, R15.S4)
- RED (mutation: 去掉对话框守卫): `bun run e2e --grep "N2 R14\.S1"` → FAIL（click 卡在 evaluate，30 s 超时）(R14.S1)
- RED (mutation: 去掉截图路径必须是绝对路径的校验): `bun run e2e --grep "N2 R15\.S2"` → FAIL: Expected: "E_INVALID_ARGUMENT" Received: ""（截图写进了 app 的工作目录）(R15.S2)
- RED (mutation: PNG 宽度读成 0): `bun run e2e --grep "N2 R15\.S1"` → FAIL (R15.S1)
- RED (mutation: evaluate 不传参数): `bun run e2e --grep "N2 R15\.S3"` → FAIL (R15.S3)
- RED (mutation: waitFor 超时当成满足): `bun run e2e --grep "N2 R15\.S4"` → FAIL (R15.S4)
- GREEN: 恢复后通过 (R14.S1, R15.S1, R15.S2, R15.S3, R15.S4)
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS（含 actions 的 base64 单测）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun test`、`bun run lint`、`bun run typecheck` 通过；`bun run e2e` → 65 passed（SC4 p95 14.7 ms，SC2 125 ms）。运行环境：本机 macOS，主会话

### T12 弹窗、下载与 CDP 透传
- Revision: 1
- RED: `bun run e2e --grep "N2 R1[67]\."` → 3 条 FAIL，脚本退出码 1：R16.S1（回执没有 popups）、R16.S2（没有下载事件，`goto` 下载地址时 E_INTERNAL）、R17.S1（page.cdp -32601）(R16.S1, R16.S2, R17.S1)
- 实现：Shell 协议 `openRequested` 增加可选 `openerTabId`（Main 填写，`bun run protocol:export` 重新生成）；TabService 识别来源是 agent 页面的弹窗，记为下一个 Page，targetId 到达时推 `page.popup` 并放进待领取列表；元素动作统计动作期间该会话的 `Page.windowOpen`，领取对应弹窗放进回执；`gateway/events.rs`：连接登记关注的 TaskSpace，通知按 TaskSpace 分发，对话框跟踪（迁入并推 `page.dialog`），下载跟踪（完成后改名、推 `page.download`）；页面会话建立时按浏览器上下文设一次下载行为；`page.cdp`；`goto` 遇 `net::ERR_ABORTED` 按成功返回；测试服务器加 `/links`、`/download`
- 排查：第一次 GREEN 尝试三条都失败于 `Browser.setDownloadBehavior` → `Failed to find browser context`（Electron 的 partition 不在 DevTools 的上下文表里）；改用页面会话的 `Page.setDownloadBehavior`（作用于页面所在的上下文），下载事件按 `Page.` 与 `Browser.` 两个域都接受
- GREEN: `bun run e2e --grep "N2 R1[67]\."` → 3 passed (R16.S1, R16.S2, R17.S1)
- RED (mutation: 不统计 `Page.windowOpen`): `bun run e2e --grep "N2 R16\.S1"` → FAIL (R16.S1)
- RED (mutation: Main 不带 `openerTabId`): `bun run e2e --grep "N2 R16\.S1"` → FAIL（弹窗成了用户标签，没有 p2）(R16.S1)
- RED (mutation: 下载完成后不改名): `bun run e2e --grep "N2 R16\.S2"` → FAIL (R16.S2)
- RED (mutation: page.cdp 不透传参数): `bun run e2e --grep "N2 R17\.S1"` → FAIL (R17.S1)
- GREEN: 恢复后通过 (R16.S1, R16.S2, R17.S1)
- REFACTOR: 回执领取弹窗后曾再等弹窗页面离开 about:blank；去掉这段等待连跑 3 次 R16.S1 都通过，场景不依赖它，删除
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS（含 `finish_download` 单测）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun test`、`bun run lint`、`bun run typecheck` 通过；`bun run e2e` → 68 passed（SC4 p95 14.8 ms，SC2 115 ms）。运行环境：本机 macOS，主会话

### T13 CDP 转发端点
- Revision: 1
- RED: `bun run e2e --grep "N2 R18\."` → R18.S1 FAIL: `browserType.connectOverCDP: read ECONNRESET`；R18.S2 FAIL: `fetch failed`（`read ECONNRESET`，T4 的占位端点只接受连接）(R18.S1, R18.S2)
- 实现：`gateway/forward.rs`：读请求头、校验 token（头名不分大小写、值完全相等，请求行不算），通过后原样转给 Chromium 端口并 `copy_bidirectional`；401 / 503 / 502；CdpMux 记下最近一次连接的 Chromium 端口；新测试文件 `apps/browser/e2e/cdp-forward.spec.ts`
- GREEN: `bun run e2e --grep "N2 R18\."` → 2 passed (R18.S1, R18.S2)
- 加强：R18.S1 另断言带 token 的 `/json/version` 里 `webSocketDebuggerUrl` 的端口是 cdpPort（否则 Playwright 可能绕过转发直连 Chromium，场景照样通过）
- RED (mutation: 不校验 token): `bun run e2e --grep "N2 R18\.S2"` → FAIL (R18.S2)
- RED (mutation: 转发时把 Host 头改成 Chromium 的端口): `bun run e2e --grep "N2 R18\.S1"` → FAIL: Expected: "49487" Received: "49484" (R18.S1)
- GREEN: 恢复后通过 (R18.S1, R18.S2)
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS（含 `authorized` 单测）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun test`、`bun run lint`、`bun run typecheck` 通过；`bun run e2e` → 70 passed（SC4 p95 14.5 ms，SC2 117 ms）。运行环境：本机 macOS，主会话

### T14 性能基准：CLI 冷启动、网关开销、snapshot
- Revision: 1
- 实现：`crates/alter-cli/tests/perf.rs`（SC1，`#[ignore]`，100 次 `alter --version` 含 fork / exec / 等退出）；`crates/alterd/tests/gateway_perf.rs`（SC2，`#[ignore]`，测试进程内的假 CDP 端经 `DevToolsActivePort` 与 `cdpReady` 交给 alterd，从发出 `page.click` 计到假 CDP 端收到第一个 `Input.dispatchMouseEvent`，两个时间点都在测试进程里取，含 WebSocket 收发与途中的 CDP 往返，只会高估；预热 20 次不计）；`apps/browser/e2e/perf.spec.ts` 加 SC3（`/big`：1000 个 li + 1000 个 button；脚本里围住整个 SDK 调用计时，只会高估；断言快照里恰有 1000 个引用，证明页面就是那 2000 个元素）
- 排查：SC2 第一次运行没有响应——`connected()` 起的 alterd 的 stderr 是管道，1000 次动作的回执日志写满没人读的管道后 alterd 卡在写日志上；改为自己起 alterd 并丢弃 stderr（`SC2_DEBUG=1` 时接到终端）
- 排查：SC3 用 debug 版 alterd 时 p50 185 ms、p95 192 ms，FAIL。计时拆分：`Accessibility.getFullAXTree`（6004 个节点）136 ms、遍历约 24 ms；同一用例换 release 版 alterd 为 p95 89 ms。试过 dev profile 给依赖开 `opt-level = 3`，只降到 158 ms（`from_str::<Value>` 在调用方 crate 实例化，仍是 debug），已撤回。决定 SC3 跑 release 版 alterd（见决策）
- GREEN: `cargo test -p alter-cli --release --test perf -- --ignored --nocapture` → `SC1: alter --version 100 次 中位数 3.39 ms，p95 4.30 ms，最大 250.02 ms`，PASS（< 20 ms）
- GREEN: `cargo test -p alterd --release --test gateway_perf -- --ignored --nocapture` → `SC2: page.click 1000 次 p50 0.413 ms，p95 0.493 ms，最大 0.777 ms`，PASS（< 5 ms）
- GREEN: `bun run e2e --grep SC3` → `SC3: 10 次 snapshot p50 82.6 ms，p95 90.0 ms`，PASS（< 150 ms）
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun test`、`bun run lint`、`bun run typecheck` 通过；`bun run e2e` → 71 passed（SC4 p95 17.1 ms，SC3 p95 90.5 ms，SC2 115 ms）。运行环境：本机 macOS，主会话

### T15 补齐 `taskSpace.cdp`、`page.mouse`、`page.keyboard`、`page.fetch`（CR1）
- Revision: 1
- RED: `cargo test -p alterd --test gateway n2_cr1_` → 4 条 FAIL：`没有这个方法：taskSpace.cdp` / `page.mouse` / `page.keyboard` / `page.fetch`（-32601）；`bun test packages/sdk` → `N2 CR1 page.mouse / keyboard / fetch 与 TaskSpace.cdp 的请求体` FAIL（`p.mouse is not a function`）
- 实现：假 CDP 端抽到 `crates/alterd/tests/common/fake_cdp.rs`（记录每条命令，结果可按方法覆盖），SC2 基准改用它；`taskSpace.cdp`（进该 TaskSpace 的队列，校验存在后不带会话发出）；`page.mouse`（move / down / up / click / wheel）、`page.keyboard`（down / up / press / type）回回执；`page.fetch` 在页面里 `fetch`（`awaitPromise`），`saveAs` 必须是绝对路径，正文以 base64 取回后写文件；SDK `Page.mouse` / `keyboard` / `fetch`
- GREEN: `cargo test -p alterd --test gateway n2_cr1_` → 4 passed；`bun test packages/sdk` → 5 pass
- RED (mutation: click 不发 mousePressed): `cargo test -p alterd --test gateway n2_cr1_page_mouse_click` → FAILED
- RED (mutation: taskSpace.cdp 带页面会话发出): `cargo test -p alterd --test gateway n2_cr1_task_space_cdp` → FAILED
- RED (mutation: fetch 不等 Promise): `cargo test -p alterd --test gateway n2_cr1_page_fetch` → FAILED
- GREEN: 恢复后通过
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS（gateway 27）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun test`（5 pass）、`bun run lint`、`bun run typecheck` 通过；`cargo test -p alterd --release --test gateway_perf -- --ignored --nocapture` → SC2 p95 0.490 ms。运行环境：本机 macOS，主会话

### T16 生命周期命令进入 TaskSpace 的串行队列（CR2）
- Revision: 1
- RED: `cargo test -p alterd --test gateway n2_cr2_` → FAIL: `finish 只等了 115.158125ms，没有排在 goto 之后`（假 CDP 端不发生命周期事件，连接 A 的 `page.goto {timeoutMs:500}` 还在等时，连接 B 的 `taskSpace.finish` 立即执行）
- 实现：`taskSpace.newPage` / `adopt` / `release` / `finish`、`page.close`、`taskSpace.cdp` 先取该 TaskSpace 的队列锁，与 `page.*` 动作排在同一队列
- GREEN: `cargo test -p alterd --test gateway` → 28 passed（`finish` 在 goto 的 `E_TIMEOUT` 之后才返回）
- VERIFY (Widen): `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 通过。运行环境：本机 macOS，主会话

### T17 `waitFor({fn})` 先等函数结果再判真假（CR3）
- Revision: 1
- RED: `bun run e2e --grep "N2 CR3 "` → FAIL：输出第一行 Expected "E_TIMEOUT" Received "matched"（`async () => false` 返回的 Promise 被 `!!` 判为真）
- 实现：`fn` 条件的表达式改为 `(async () => !!(await (<fn>)()))()`，`Runtime.evaluate` 已带 `awaitPromise`
- GREEN: `bun run e2e --grep "N2 (CR3 |R15\.S4)"` → 2 passed
- VERIFY (Widen): `bun run lint`、`bun run typecheck`、`cargo clippy --all-targets -- -D warnings` 通过。运行环境：本机 macOS，主会话

### T18 连接结束时移除它的 listener（CR4）
- Revision: 1
- RED: `cargo test -p alterd --bin alterd gateway::` → `connection_ends_and_unregisters_after_client_closes` FAIL：`客户端断开后连接任务应当结束: Elapsed(())`（tokio 单测：回环 TCP 上用真 token 握手，确认已登记 listener 后客户端关闭）
- 实现：`connection` 的读循环结束后按 `interest` 的指针移除自己的 listener，再关写队列、等写出任务
- GREEN: `cargo test -p alterd --bin alterd gateway::` → 4 passed；`cargo test -p alterd --test gateway` → 28 passed
- VERIFY (Widen): `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 通过。运行环境：本机 macOS，主会话

### T19 `page.close` 写回执日志，成功行带 `error_code`（CR5）
- Revision: 1
- RED: `bun run e2e --grep "N2 CR5 "` → FAIL：`page.goto` 的回执行缺 `"error_code": ""`（`page.close` 也没有回执行）
- 实现：`page.close` 改经 `page::call`（排队、回执日志；对话框打开时放行）；成功的回执行写 `error_code = ""`；design.md「接口契约」补上回执日志字段、串行范围与 T15 的四个方法
- GREEN: `bun run e2e --grep "N2 (CR5 |R19\.)"` → 2 passed；`cargo test -p alterd --test gateway` → 28 passed
- VERIFY (Widen，T15–T19): `cargo test --no-fail-fast` 全部 PASS（gateway 28、alterd 单测 4、shell 52）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun test`（5 pass）、`bun run lint`、`bun run typecheck` 通过；`bun run e2e` → 73 passed（SC4 p95 15.4 ms，SC3 p95 89.1 ms，SC2 116 ms）。运行环境：本机 macOS，主会话

### T20 `waitFor` 的每次求值受剩余期限约束，放弃等待的 CDP 请求被移除（CR6）
- Revision: 1
- RED: `bun run e2e --grep "N2 CR6 "` → FAIL（30.0 s 用例超时：`waitFor({fn: () => new Promise(() => {})}, {timeoutMs: 300})` 一直等）
- 实现：`wait_for` 每次 `Runtime.evaluate` 套 `tokio::time::timeout(剩余期限)`，到期回 `E_TIMEOUT`；`CdpMux::send` 加 `ForgetOnDrop` 守卫，调用方丢弃等待时从 pending 移除该 id；cdp 模块加 tokio 单测（只收不回的假端，外层超时后 pending 为空）
- GREEN: `bun run e2e --grep "N2 (CR3 |CR6 |R15\.S4)"` → 3 passed（CR6 用例 973 ms，之后的 `p.title()` 正常返回）；`cargo test -p alterd --bin alterd cdp::` → 1 passed
- RED (mutation: 守卫不移除): `cargo test -p alterd --bin alterd cdp::` → FAILED
- GREEN: 恢复后通过

### T21 CR2 的测试从假 CDP 端收到 `Page.navigate` 起判断顺序（CR7）
- Revision: 1
- 实现：去掉固定 100 ms 睡眠，等假 CDP 端收到 `Page.navigate`（此时 goto 已持有队列）再发 `finish`，断言 `finish` 在该时点之后至少 300 ms 才返回
- GREEN: `cargo test -p alterd --test gateway n2_cr2_` 连跑 3 次 → 均 1 passed
- RED (mutation: finish 不排队): `cargo test -p alterd --test gateway n2_cr2_` → FAIL：`finish 在 Page.navigate 之后只等了 941.958µs，没有排在 goto 之后`
- GREEN: 恢复后通过
- VERIFY (Widen，T20–T21): `cargo test --no-fail-fast` 全部 PASS（gateway 28、alterd 单测 5）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun test`、`bun run lint`、`bun run typecheck` 通过；`bun run e2e` → 74 passed（SC4 p95 16.3 ms，SC3 p95 91.0 ms，SC2 115 ms）。运行环境：本机 macOS，主会话

### T22 R5.S3 的 2 秒从创建动作开始计（验证缺口）
- Revision: 1
- 实现：用例在 `runScript` 之前记起点，之后每个等待的时限取 2 秒的剩余部分（含 Bun 启动，只会高估）
- VERIFY: `bun run e2e --grep "N2 R5\.S3"` → 1 passed（2.1 s，含 app 启动）；这是测试本身的修正，产品代码不变，实现早已满足场景

### T23 `page.snapshot` 的 `scope` / `root`（验证缺口）
- Revision: 1
- RED: `bun run e2e --grep "N2 V2 "` → FAIL：Expected `"root": false` Received `"root": true`（`scope: "subtree"` 被忽略，返回整棵树）
- 实现：`scope` 为 `full_page`（默认）或 `subtree`；`subtree` 用选择器定位 `root`，`DOM.describeNode` 取 backendNodeId，从无障碍树里对应的节点开始输出；其他范围回 `E_INVALID_ARGUMENT`；SDK 类型去掉 `viewport`
- GREEN: `bun run e2e --grep "N2 (V2 |R12\.)"` → 2 passed

### T24 元素动作的 `actionLabel` 写进回执日志（验证缺口）
- Revision: 1
- RED: `bun run e2e --grep "N2 V3 "` → FAIL：`page.click` 的回执行缺 `"action_label": "提交表单"`
- 实现：`page::call` 读请求的 `actionLabel`，成功与失败的回执行都写 `action_label`（没有时为空字符串）
- GREEN: `bun run e2e --grep "N2 (V3 |R19\.)"` → 2 passed

### T25 `page.waitFor` 的条件收窄（验证缺口，文档修正）
- Revision: 1
- VERIFY: design.md「接口契约」写明条件为 url / selector / fn / ms，事件经 SDK 的 `waitForEvent`；决策记录在「决策与偏差」；`dflow.py validate add-core-features` → ok

### T26 `page.reload` 按 `waitUntil` 等待（验证缺口）
- Revision: 1
- RED: `bun run e2e --grep "N2 V5 "` → FAIL：脚本退出码 1（`reload({waitUntil: "domcontentloaded", timeoutMs: 2000})` 仍等 `load`，被 5 秒的图片拖到超时）
- 实现：`waitUntil` 为 `domcontentloaded` 时等 `Page.domContentEventFired`，否则 `Page.loadEventFired`；测试服务器加 `/slow-image`
- GREEN: `bun run e2e --grep "N2 V5 "` → 1 passed（输出 `true`、`E_TIMEOUT`）

### T27 SDK 形状与 `TaskSpace.name`（验证缺口）
- Revision: 1
- RED: `cargo test -p alterd --test gateway n2_v6_` → FAILED（create 结果没有 `name`）；`bun run typecheck` → `Property 'name' does not exist on type 'TaskSpace'`、`Property 'click' does not exist on type '(op: ...) => Promise<Receipt>'`；`bun test packages/sdk` → 1 fail
- 实现：`TaskSpaceInfo` 加 `name`（`bun run protocol:export`）；SDK `TaskSpace.name`；`page.mouse` / `page.keyboard` 改为对象方法（move / down / up / click / wheel，down / up / press / type）；`waitForFileChooser` 推迟（见决策）
- GREEN: `cargo test -p alterd --test gateway` → 29 passed；`bun test packages/sdk` → 5 pass；`bun run typecheck` 通过

### T28 Gateway 连不上时 `alter nodejs` 以 2 退出（验证缺口）
- Revision: 1
- RED: `cargo test -p alter-cli --test nodejs n2_v7_` → FAIL：`code=Some(0) stdout="ran\n"`（gateway.json 指向没人监听的端口时照样运行脚本）
- 实现：读到 gateway.json 后先 `TcpStream::connect_timeout` 到它的端口（1 秒），连不上提示打开 alter.app 并以 2 退出
- GREEN: `cargo test -p alter-cli --test nodejs` → 5 passed

### T29 R20.S1、R20.S3 的行为 RED（验证缺口，只补证据）
- Revision: 1
- RED (mutation: 握手不带 Authorization 头): `bun test packages/sdk` → `(fail) N2 R20.S1 taskSpace 按名称创建、按数字恢复，握手带 Bearer token` (R20.S1)
- RED (mutation: 连接失败时的提示不含「请先打开 alter.app」): `bun test packages/sdk` → `(fail) N2 R20.S3 连接失败：AlterError E_INTERNAL，提示打开 alter.app` (R20.S3)
- GREEN: 恢复后 `bun test packages/sdk` → 5 pass (R20.S1, R20.S3)
- VERIFY (Widen，T22–T29): `cargo test --no-fail-fast` 全部 PASS（gateway 29、nodejs 5、shell 52）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun test`（5 pass）、`bun run lint`、`bun run typecheck` 通过；`bun run e2e` → 77 passed（SC4 p95 15.3 ms，SC3 p95 83.8 ms，SC2 125 ms）；SC1 中位数 3.69 ms。运行环境：本机 macOS，主会话

### T30 R5.S3 用绝对截止时间判断 2 秒（CR8）
- Revision: 1
- 实现：脚本与界面检查同时进行，各项等待的时限取截止时间的剩余部分，全部满足后断言当前时间不晚于截止时间，最后再检查脚本退出码
- GREEN: `bun run e2e --grep "N2 R5\.S3"` → 1 passed；`--repeat-each 5` → 5 passed
- RED (mutation: 脚本先等 2.5 秒再创建，模拟创建慢): `bun run e2e --grep "N2 R5\.S3"` → FAIL：`expect(locator).toHaveCount(expected) failed` (R5.S3)
- GREEN: 恢复后通过 (R5.S3)
- VERIFY (Widen): `bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T31 R5.S3 在脚本结束后也断言截止时间，改用单调时钟（CR9、CR10）
- Revision: 1
- 实现：计时改用 `ui.ts` 的 `now()`（`performance.now()`）；界面全部就绪后、脚本结束后各断言一次不晚于截止时间
- GREEN: `bun run e2e --grep "N2 R5\.S3" --repeat-each 5` → 5 passed
- RED (mutation: 创建后脚本再等 2.5 秒才结束，界面先就绪): `bun run e2e --grep "N2 R5\.S3"` → FAIL：`创建动作没有在 2 秒内完成` (R5.S3)
- GREEN: 恢复后通过 (R5.S3)
- VERIFY (Widen): `bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

## 决策与偏差
- 起点（2026-10-21）：N1 完成后的 HEAD `532a56d`，分支 `feature/add-core-features`，工作区干净；全量检查在 N1 最终验证时为绿。
- T2：N1 的协议导出用例 `n1_r16_s1/s2_*` 随 N2 对同一需求的 MODIFIED（R23）改名为 `n2_r23_s1/s2_*` 并更新期望；N1 的 R16 场景由 R23 取代（存档时按节点顺序合并）。测试辅助 `object_fields` 从类型定义的 `= {` 取字段（生成文件开头可能有 `import type { … }`）。
- 验证缺口（T22–T29）：`page.snapshot` 的 `viewport` 范围不做（判断节点是否在视口内要逐个取盒模型，代价大且没有场景要求），回 `E_INVALID_ARGUMENT`；`page.waitFor` 的「事件」条件收窄到 SDK 的 `waitForEvent`；SDK 的 `waitForFileChooser()` 推迟（需要 `Page.setInterceptFileChooserDialog` 与新的 RPC，上传已可用 `setInputFiles`），列为后续。design.md「接口契约」已同步，存档时写回 api.md。
- T14：SC3 跑 release 版 alterd：`bun run e2e` 先多构建一次 release 版 alterd（增量），SC3 用 `ALTER_ALTERD_PATH` 指向它。理由：性能预算针对发布出去的二进制（SC1、SC2 也都是 `--release`）；debug 版解析几 MB 的无障碍树要多约 100 ms，不代表用户看到的耗时。其他 e2e 仍用 debug 版。SC2 没有按任务原文在 alterd 的 debug 日志里打两个时间点：测试进程同时握着请求方和假 CDP 端，直接在测试里取两个时间点更简单，且只会高估。
- T13：CDP 转发不改写 `/json/version`，改为原样转发请求头（Chromium 按 `Host` 头生成 WebSocket 地址），design.md「接口契约」已同步；对外的契约（地址指向 cdpPort、401）不变。同一 TCP 连接上 token 只在第一个请求校验（keep-alive 的后续请求来自同一个已认证的客户端）。
- T12：下载目录的 config.toml `download_dir` 没有实现：项目里还没有读 config.toml 的代码，引入 TOML 解析放到第一个必须用它的节点（N3 的 inactive 阈值），届时补 `download_dir`（后续）。弹窗不受 page budget 限制（页面自己打开的窗口不能拒绝，拒绝会丢失 agent 需要的页面）。`page.cdp` 在对话框打开时放行。快照的可交互角色比 design.md 原列表多 searchbox、switch、slider、spinbutton，design.md 已同步。以上写进 design.md「接口契约」。
- T11：agent 视图里的 JS 对话框，Electron 默认还会弹一个原生对话框（没有父窗口）。CDP `Page.handleJavaScriptDialog` 会一并关掉它，场景成立；曾试过在 Main 里换掉 Electron 的私有事件 `-run-dialog` 来不弹原生对话框，变异显示场景不依赖它，又依赖私有 API，已删除。原生对话框会打扰用户，是否隐藏留给 N3（接管与 AgentOverlay）决定（后续）。R15.S2 在实现前因方法不存在已经是 PASS（SDK 把 -32601 也映射为 `E_INVALID_ARGUMENT`），RED 不成立，改用下面的变异取得 RED。
- T10：`text=` / CSS / `xpath=` 的查找在页面主 world 里执行（`Runtime.evaluate`），没有用 design.md 写的隔离 world：隔离 world 每次导航都要重建且要先取 frameId，主 world 已满足场景；页面脚本可以干扰查找，留作后续（隔离 world）。tasks.md 子项里写的是隔离 world，design.md 未涉及。`page.evaluate` 随 R13.S1 一起在 T10 实现（R15.S3 在 T11 用变异取得 RED）。
- T9：R19.S1（回执日志）要用一次失败的 `page.click` 驱动，`click` 在 T10 才实现，`covers` 移到 T10；回执日志本身在 T9 实现。`DevToolsActivePort` 第二行就是浏览器级 WebSocket 路径，CdpMux 直接读它，不需要再请求 `/json/version`（design.md 留给 T4 决定的 HTTP 客户端问题因此不存在，T13 的转发端点另行处理）。
- T8：任务里列的「CdpMux 连接与 target 认领」（`cdp/conn.rs`、`targets.rs`）移到 T9 与 `page.goto` 一起做：R5.S3 不经过 CDP，没有能驱动它的场景，放进 T9 才能测试先行。
- T6：SDK 的公开入口是 `createAlter({gatewayFile})`（返回一组共享连接的函数），`runtime.ts` 用默认实例挂全局；测试经它指向假 Gateway，没有专为测试开的接口。空闲断开（20 ms）是为了让 `alter nodejs` 的脚本结束后进程能退出（打开的 WebSocket 会让 Bun 一直等待）。
- T3：新依赖 tokio-tungstenite 0.28（Gateway、CDP 转发、CDP 客户端共用）、getrandom 0.4（token）、futures-util（拆分 WS 读写）；测试用同步的 tungstenite 作客户端（dev-dependency）。Cargo.lock 的源地址为 crates.io。`taskSpace.create` 在 T3 只做参数校验（R3.S3），之后回「尚未实现」，T5 实现。
- T1：spike 发现第三个方案 C（Electron 44 的 `getOrCreateDevToolsTargetId()`），比 A、B 都好且不违反宪章，采用它；Shell 增量 `tabUpdated.targetId` 记入 design.md「接口契约」。

## 代码评审
<!-- 由评审步骤填写，每次评审一轮：
### Round <n>
- Revision: <当前 revision>
- Scope: <repo> `git diff <base>...HEAD`（含未提交的改动）
- Tasks: T1, T2, …（这一轮读过代码的任务；收敛任务和重新打开的任务需要新的一轮）
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
最后恰好一行 "Review: PASS" 或 "Review: CHANGES REQUESTED"。语法见 references/formats.md -->

### Round 1
- Revision: 1
- Scope: alter `git diff 532a56d...HEAD`（N2 的 T1–T14），无未提交改动；reviewer: codex (gpt-6-sol)
- Tasks: T1-T14
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR1 | major | spec | crates/alterd/src/gateway/rpc.rs:255 | design.md 的目标是 api.md 中除 N3、N5 以外的全部方法，但 `taskSpace.cdp`、`page.mouse`、`page.keyboard`、`page.fetch` 没有分发；SDK 公开的 `TaskSpace.cdp()` 必然得到 `E_INVALID_ARGUMENT` | T15 |
| CR2 | major | correctness | crates/alterd/src/gateway/rpc.rs:219 | 串行队列只包住部分 `page.*`；`taskSpace.newPage` / `adopt` / `release` / `finish` 与 `page.close` 绕过它，`page.goto` 等待加载时另一连接的 `finish` 先关掉视图，导航以非预期的错误结束 | T16 |
| CR3 | major | correctness | crates/alterd/src/gateway/actions.rs:533 | `waitFor({fn})` 对函数返回值先取 `!!`，异步函数返回的 Promise 被判为真，`async () => false` 立即满足（R15） | T17 |
| CR4 | major | correctness | crates/alterd/src/gateway/mod.rs:151 | 连接结束后 `listeners` 仍持有写队列的发送端，`writer.await` 不结束；没有新通知触发清理时，每次 SDK 断连都留下一个连接任务 | T18 |
| CR5 | minor | spec | crates/alterd/src/gateway/rpc.rs:250 | `page.close` 不写回执日志，成功的回执行没有 `error_code` 字段；R19 要求每个 `page.*` 调用一行、字段包含 `error_code` | T19 |

Review: CHANGES REQUESTED

### Round 2
- Revision: 1
- Scope: alter `git diff 603cb94...HEAD`（收敛任务 T15–T19），无未提交改动；reviewer: codex (gpt-6-sol)。CR1、CR2、CR4、CR5 确认已修好
- Tasks: T15, T16, T17, T18, T19
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR6 | major | correctness | crates/alterd/src/gateway/actions.rs:685 | `waitFor({fn})` 的函数返回永不完成的 Promise 时，`Runtime.evaluate` 的 `awaitPromise` 一直等，期限检查在 CDP 返回之后，`timeoutMs` 不生效且一直占着该 TaskSpace 的队列（R15） | T20 |
| CR7 | minor | tests | crates/alterd/tests/gateway.rs:669 | CR2 的测试用固定 100 ms 睡眠推断 goto 已取得队列锁，机器繁忙时正确实现也可能误报 | T21 |

Review: CHANGES REQUESTED

### Round 3
- Revision: 1
- Scope: alter `git diff 020d8c0...HEAD`（收敛任务 T20、T21），无未提交改动；reviewer: codex (gpt-6-sol)。CR6、CR7 确认已修好，未发现新的问题
- Tasks: T20, T21
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|

Review: PASS

### Round 4
- Revision: 1
- Scope: alter `git diff 3ab3159...HEAD`（验证缺口的收敛任务 T22–T29），无未提交改动；reviewer: codex (gpt-6-sol)。T23–T29 未发现问题
- Tasks: T22, T23, T24, T25, T26, T27, T28, T29
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR8 | major | tests | apps/browser/e2e/agent.spec.ts:34 | T22 的 `left()` 超过 2 秒后仍返回 1 ms，且没有断言截止时间：创建耗时 3 秒、界面随后已更新时，R5.S3 仍判通过 | T30 |

Review: CHANGES REQUESTED

### Round 5
- Revision: 1
- Scope: alter `git diff b9dc720...HEAD`（收敛任务 T30），无未提交改动；reviewer: codex (gpt-6-sol)
- Tasks: T30
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR9 | major | tests | apps/browser/e2e/agent.spec.ts:42 | 截止时间只在 `await script` 之前断言：界面及时出现、但 `taskSpace("demo")` 2 秒后才完成时 R5.S3 仍判通过 | T31 |
| CR10 | minor | tests | apps/browser/e2e/agent.spec.ts:34 | 用 `Date.now()` 计时，系统校时后拨时会少计耗时，「只会高估」不严格成立 | T31 |

Review: CHANGES REQUESTED

### Round 6
- Revision: 1
- Scope: alter `git diff d4f5115...HEAD`（收敛任务 T31），无未提交改动；reviewer: codex (gpt-6-sol)。CR9、CR10 确认已修好
- Tasks: T31
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR11 | minor | tests | apps/browser/e2e/agent.spec.ts:41 | R5.S3 里调用的 `waitUntilAsync`（`e2e/ui.ts`）仍用 `Date.now()` 轮询，系统时间在等待期间向前跳时可能误报失败 | accepted: 只会让正确实现偶发误报、不会放过错误实现；需要恰在 2 秒内发生系统时间前跳，概率极低 |

Review: PASS

### Round 7
- Revision: 1
- Scope: alter 当前 HEAD（d0e8299 之后只有文档改动）上这些收敛任务的代码与测试——关闭确认：dev-flow 新版要求收敛任务出现在结论为 PASS 的后续轮次里，发现才算关闭；下列任务当时在下一轮复查过，但那一轮因别的新发现结论为 CHANGES REQUESTED
- Reviewers: claude (opus-5, self-review): 0 findings（核对各任务的 RED / 变异证据）；codex (gpt-5.6-sol): 逐条确认 CR1–CR5、CR8 已修复、没有回退，0 个新发现
- Tasks: T15, T16, T17, T18, T19, T30
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|

Review: PASS

## 验证报告
<!-- 每次验证追加一份新报告，放在最近一轮评审之后。以 "- Revision: <当前 revision>" 和 "- Review: Round <最近一轮>" 开头。由验证步骤填写：本仓库的测试结果，每个场景一行 "| R<n>.S<m> | <path::test> | PASS |"，成功标准对照表，收敛缺口，fix 类型还要写 "Fix: verified | partial | failed"，最后恰好一行 "Overall: READY" 或 "Overall: NOT READY"。 -->

- Revision: 1
- Review: Round 3
- Verifier: codex (gpt-6-sol)

| Repo | Command | Result |
|---|---|---|
| alter | `cargo test --no-fail-fast` | PASS（gateway 28、shell 52、lifecycle 8、alterd 单测 5、cli 5、nodejs 4、export_ts 4、store 7 等；本机 macOS，提交 e76c4c3） |
| alter | `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` | PASS |
| alter | `bun test` | PASS（5） |
| alter | `bun run lint` / `bun run typecheck` | PASS |
| alter | `bun run e2e` | PASS（74 passed） |

| Scenario | Test | Result |
|---|---|---|
| R1.S1 | crates/alterd/tests/gateway.rs::n2_r1_s1_writes_gateway_file | PASS |
| R1.S2 | crates/alterd/tests/gateway.rs::n2_r1_s2_removes_gateway_file_on_exit | PASS |
| R2.S1 | crates/alterd/tests/gateway.rs::n2_r2_s1_missing_token_is_401 | PASS |
| R2.S2 | crates/alterd/tests/gateway.rs::n2_r2_s2_wrong_token_is_401 | PASS |
| R2.S3 | crates/alterd/tests/gateway.rs::n2_r2_s3_hello_with_valid_token | PASS |
| R3.S1 | crates/alterd/tests/gateway.rs::n2_r3_s1_protocol_version_mismatch | PASS |
| R3.S2 | crates/alterd/tests/gateway.rs::n2_r3_s2_unknown_method | PASS |
| R3.S3 | crates/alterd/tests/gateway.rs::n2_r3_s3_missing_param | PASS |
| R4.S1 | crates/alterd/tests/gateway.rs::n2_r4_s1_space_list_with_tab_counts | PASS |
| R5.S1 | crates/alterd/tests/gateway.rs::n2_r5_s1_create_in_active_space | PASS |
| R5.S2 | crates/alterd/tests/gateway.rs::n2_r5_s2_create_in_unknown_space | PASS |
| R5.S3 | apps/browser/e2e/agent.spec.ts::N2 R5.S3 侧边栏显示 agent 页面：多出一行带 p1 标记，Page A 仍激活，主进程有 about:blank 的页面 | PASS |
| R6.S1 | crates/alterd/tests/gateway.rs::n2_r6_s1_resume_from_another_connection | PASS |
| R6.S2 | crates/alterd/tests/gateway.rs::n2_r6_s2_resume_unknown | PASS |
| R6.S3 | crates/alterd/tests/gateway.rs::n2_r6_s3_list | PASS |
| R7.S1 | crates/alterd/tests/gateway.rs::n2_r7_s1_new_page_numbering | PASS |
| R7.S2 | crates/alterd/tests/gateway.rs::n2_r7_s2_page_budget | PASS |
| R7.S3 | crates/alterd/tests/gateway.rs::n2_r7_s3_labels_are_not_reused | PASS |
| R8.S1 | crates/alterd/tests/gateway.rs::n2_r8_s1_adopt_and_tabs | PASS |
| R8.S2 | crates/alterd/tests/gateway.rs::n2_r8_s2_adopt_already_adopted | PASS |
| R8.S3 | crates/alterd/tests/gateway.rs::n2_r8_s3_adopt_across_spaces | PASS |
| R8.S4 | crates/alterd/tests/gateway.rs::n2_r8_s4_release | PASS |
| R9.S1 | crates/alterd/tests/gateway.rs::n2_r9_s1_finish_keeps_listed_pages | PASS |
| R10.S1 | crates/alterd/tests/gateway.rs::n2_r10_s1_page_closed_by_user | PASS |
| R11.S1 | apps/browser/e2e/agent.spec.ts::N2 R11.S1 打开本地测试页：回执 ok、durationMs 为非负整数，标题为 Page A，退出码 0 | PASS |
| R11.S2 | apps/browser/e2e/agent.spec.ts::N2 R11.S2 导航超时：异常 code 为 E_TIMEOUT，脚本以退出码 1 结束 | PASS |
| R12.S1 | apps/browser/e2e/agent.spec.ts::N2 R12.S1 表单页快照：有 textbox Email 与 button Submit 两行，各带 @e 引用 | PASS |
| R13.S1 | apps/browser/e2e/agent.spec.ts::N2 R13.S1 填表并提交：正文含 submitted: a@b.c | PASS |
| R13.S2 | apps/browser/e2e/agent.spec.ts::N2 R13.S2 用快照引用点击：正文含 submitted: x@y.z | PASS |
| R13.S3 | apps/browser/e2e/agent.spec.ts::N2 R13.S3 选择器匹配多个元素：E_AMBIGUOUS_SELECTOR，details.count 为 2 | PASS |
| R13.S4 | apps/browser/e2e/agent.spec.ts::N2 R13.S4 选择器无匹配：E_NO_MATCH | PASS |
| R14.S1 | apps/browser/e2e/agent.spec.ts::N2 R14.S1 先处理对话框再操作：E_DIALOG_OPEN，details.message 为 hi；accept 后点击成功 | PASS |
| R15.S1 | apps/browser/e2e/agent.spec.ts::N2 R15.S1 截图（p1 是离屏页面）：width、height 大于 0，文件以 PNG 签名开头 | PASS |
| R15.S2 | apps/browser/e2e/agent.spec.ts::N2 R15.S2 截图到相对路径被拒绝：E_INVALID_ARGUMENT | PASS |
| R15.S3 | apps/browser/e2e/agent.spec.ts::N2 R15.S3 evaluate 返回值：输出 42 | PASS |
| R15.S4 | apps/browser/e2e/agent.spec.ts::N2 R15.S4 等待选择器超时：E_TIMEOUT | PASS |
| R16.S1 | apps/browser/e2e/agent.spec.ts::N2 R16.S1 点击新窗口链接：回执 popups 为 [{label:p2,…}]，p2 的 url 为 /b 的地址 | PASS |
| R16.S2 | apps/browser/e2e/agent.spec.ts::N2 R16.S2 下载文件：输出 D/report.txt，文件存在 | PASS |
| R17.S1 | apps/browser/e2e/agent.spec.ts::N2 R17.S1 透传 Runtime.evaluate：result.value 为 2 | PASS |
| R18.S1 | apps/browser/e2e/cdp-forward.spec.ts::N2 R18.S1 Playwright 连接：connectOverCDP 成功，某个页面的 url 为 /a 的地址 | PASS |
| R18.S2 | apps/browser/e2e/cdp-forward.spec.ts::N2 R18.S2 缺少 token：/json/version 返回 401 | PASS |
| R19.S1 | apps/browser/e2e/agent.spec.ts::N2 R19.S1 回执日志：page.goto 成功一行、page.click 失败一行（E_NO_MATCH） | PASS |
| R20.S1 | packages/sdk/tests/sdk.test.ts::N2 R20.S1 taskSpace 按名称创建、按数字恢复，握手带 Bearer token | PASS |
| R20.S2 | packages/sdk/tests/sdk.test.ts::N2 R20.S2 错误映射为 AlterError 且不重试 | PASS |
| R20.S3 | packages/sdk/tests/sdk.test.ts::N2 R20.S3 连接失败：AlterError E_INTERNAL，提示打开 alter.app | PASS |
| R21.S1 | crates/alter-cli/tests/nodejs.rs::n2_r21_s1_runs_inline_script | PASS |
| R21.S2 | crates/alter-cli/tests/nodejs.rs::n2_r21_s2_runs_script_from_stdin | PASS |
| R21.S3 | crates/alter-cli/tests/nodejs.rs::n2_r21_s3_script_error_exits_1 | PASS |
| R21.S4 | crates/alter-cli/tests/nodejs.rs::n2_r21_s4_no_gateway_exits_2 | PASS |
| R22.S1 | crates/alter-cli/tests/cli.rs::n2_r22_s1_spaces_list_text_and_json | PASS |
| R23.S1 | crates/alter-core/tests/export_ts.rs::n2_r23_s1_export_error_code_and_health_file | PASS |
| R23.S2 | crates/alter-core/tests/export_ts.rs::n2_r23_s2_export_shell_messages | PASS |
| R23.S3 | crates/alter-core/tests/export_ts.rs::n2_r23_s3_export_gateway_types | PASS |

| SC | Check | Result |
|---|---|---|
| SC1 | `cargo test -p alter-cli --release --test perf -- --ignored --nocapture` → 中位数 3.39 ms | PASS（< 20 ms） |
| SC2 | `cargo test -p alterd --release --test gateway_perf -- --ignored --nocapture` → p95 0.490 ms | PASS（< 5 ms） |
| SC3 | `bun run e2e --grep SC3` → p95 91.0 ms | PASS（< 150 ms） |

Gaps:
- partial：R5.S3 的 2 秒时限从 `runScript` 返回后才开始计 → N2/T22
- partial：`page.snapshot` 的 `scope` / `root` 不被解析 → N2/T23
- partial：元素动作的 `actionLabel` 没有写进回执日志（design.md 要求）→ N2/T24
- partial：api.md 的 `page.waitFor` 含「事件」条件，实现只接受 url / selector / fn / ms → N2/T25
- partial：`page.reload` 忽略 `waitUntil` → N2/T26
- missing / contradicts：SDK 缺 `TaskSpace.name`、`waitForFileChooser()`，`mouse` / `keyboard` 的形状与 api.md 的 `mouse.*` / `keyboard.*` 不一致 → N2/T27
- contradicts：`gateway.json` 在但 Gateway 连不上时 `alter nodejs` 不以 2 退出 → N2/T28
- partial：R20.S1、R20.S3 的 RED 是导入错误，没有行为上的 RED → N2/T29

Overall: NOT READY

- Revision: 1
- Review: Round 6
- Verifier: codex (gpt-6-sol)

| Repo | Command | Result |
|---|---|---|
| alter | `cargo test --no-fail-fast` | PASS（gateway 29、shell 52、lifecycle 8、alterd 单测 5、cli 5、nodejs 5、export_ts 4、store 7 等；本机 macOS，HEAD f0b63e5） |
| alter | `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` | PASS |
| alter | `bun test` | PASS（5） |
| alter | `bun run lint` / `bun run typecheck` | PASS |
| alter | `bun run e2e` | PASS（77 passed） |

| Scenario | Test | Result |
|---|---|---|
| R1.S1 | crates/alterd/tests/gateway.rs::n2_r1_s1_writes_gateway_file | PASS |
| R1.S2 | crates/alterd/tests/gateway.rs::n2_r1_s2_removes_gateway_file_on_exit | PASS |
| R2.S1 | crates/alterd/tests/gateway.rs::n2_r2_s1_missing_token_is_401 | PASS |
| R2.S2 | crates/alterd/tests/gateway.rs::n2_r2_s2_wrong_token_is_401 | PASS |
| R2.S3 | crates/alterd/tests/gateway.rs::n2_r2_s3_hello_with_valid_token | PASS |
| R3.S1 | crates/alterd/tests/gateway.rs::n2_r3_s1_protocol_version_mismatch | PASS |
| R3.S2 | crates/alterd/tests/gateway.rs::n2_r3_s2_unknown_method | PASS |
| R3.S3 | crates/alterd/tests/gateway.rs::n2_r3_s3_missing_param | PASS |
| R4.S1 | crates/alterd/tests/gateway.rs::n2_r4_s1_space_list_with_tab_counts | PASS |
| R5.S1 | crates/alterd/tests/gateway.rs::n2_r5_s1_create_in_active_space | PASS |
| R5.S2 | crates/alterd/tests/gateway.rs::n2_r5_s2_create_in_unknown_space | PASS |
| R5.S3 | apps/browser/e2e/agent.spec.ts::N2 R5.S3 侧边栏显示 agent 页面：多出一行带 p1 标记，Page A 仍激活，主进程有 about:blank 的页面 | PASS |
| R6.S1 | crates/alterd/tests/gateway.rs::n2_r6_s1_resume_from_another_connection | PASS |
| R6.S2 | crates/alterd/tests/gateway.rs::n2_r6_s2_resume_unknown | PASS |
| R6.S3 | crates/alterd/tests/gateway.rs::n2_r6_s3_list | PASS |
| R7.S1 | crates/alterd/tests/gateway.rs::n2_r7_s1_new_page_numbering | PASS |
| R7.S2 | crates/alterd/tests/gateway.rs::n2_r7_s2_page_budget | PASS |
| R7.S3 | crates/alterd/tests/gateway.rs::n2_r7_s3_labels_are_not_reused | PASS |
| R8.S1 | crates/alterd/tests/gateway.rs::n2_r8_s1_adopt_and_tabs | PASS |
| R8.S2 | crates/alterd/tests/gateway.rs::n2_r8_s2_adopt_already_adopted | PASS |
| R8.S3 | crates/alterd/tests/gateway.rs::n2_r8_s3_adopt_across_spaces | PASS |
| R8.S4 | crates/alterd/tests/gateway.rs::n2_r8_s4_release | PASS |
| R9.S1 | crates/alterd/tests/gateway.rs::n2_r9_s1_finish_keeps_listed_pages | PASS |
| R10.S1 | crates/alterd/tests/gateway.rs::n2_r10_s1_page_closed_by_user | PASS |
| R11.S1 | apps/browser/e2e/agent.spec.ts::N2 R11.S1 打开本地测试页：回执 ok、durationMs 为非负整数，标题为 Page A，退出码 0 | PASS |
| R11.S2 | apps/browser/e2e/agent.spec.ts::N2 R11.S2 导航超时：异常 code 为 E_TIMEOUT，脚本以退出码 1 结束 | PASS |
| R12.S1 | apps/browser/e2e/agent.spec.ts::N2 R12.S1 表单页快照：有 textbox Email 与 button Submit 两行，各带 @e 引用 | PASS |
| R13.S1 | apps/browser/e2e/agent.spec.ts::N2 R13.S1 填表并提交：正文含 submitted: a@b.c | PASS |
| R13.S2 | apps/browser/e2e/agent.spec.ts::N2 R13.S2 用快照引用点击：正文含 submitted: x@y.z | PASS |
| R13.S3 | apps/browser/e2e/agent.spec.ts::N2 R13.S3 选择器匹配多个元素：E_AMBIGUOUS_SELECTOR，details.count 为 2 | PASS |
| R13.S4 | apps/browser/e2e/agent.spec.ts::N2 R13.S4 选择器无匹配：E_NO_MATCH | PASS |
| R14.S1 | apps/browser/e2e/agent.spec.ts::N2 R14.S1 先处理对话框再操作：E_DIALOG_OPEN，details.message 为 hi；accept 后点击成功 | PASS |
| R15.S1 | apps/browser/e2e/agent.spec.ts::N2 R15.S1 截图（p1 是离屏页面）：width、height 大于 0，文件以 PNG 签名开头 | PASS |
| R15.S2 | apps/browser/e2e/agent.spec.ts::N2 R15.S2 截图到相对路径被拒绝：E_INVALID_ARGUMENT | PASS |
| R15.S3 | apps/browser/e2e/agent.spec.ts::N2 R15.S3 evaluate 返回值：输出 42 | PASS |
| R15.S4 | apps/browser/e2e/agent.spec.ts::N2 R15.S4 等待选择器超时：E_TIMEOUT | PASS |
| R16.S1 | apps/browser/e2e/agent.spec.ts::N2 R16.S1 点击新窗口链接：回执 popups 为 [{label:p2,…}]，p2 的 url 为 /b 的地址 | PASS |
| R16.S2 | apps/browser/e2e/agent.spec.ts::N2 R16.S2 下载文件：输出 D/report.txt，文件存在 | PASS |
| R17.S1 | apps/browser/e2e/agent.spec.ts::N2 R17.S1 透传 Runtime.evaluate：result.value 为 2 | PASS |
| R18.S1 | apps/browser/e2e/cdp-forward.spec.ts::N2 R18.S1 Playwright 连接：connectOverCDP 成功，某个页面的 url 为 /a 的地址 | PASS |
| R18.S2 | apps/browser/e2e/cdp-forward.spec.ts::N2 R18.S2 缺少 token：/json/version 返回 401 | PASS |
| R19.S1 | apps/browser/e2e/agent.spec.ts::N2 R19.S1 回执日志：page.goto 成功一行、page.click 失败一行（E_NO_MATCH） | PASS |
| R20.S1 | packages/sdk/tests/sdk.test.ts::N2 R20.S1 taskSpace 按名称创建、按数字恢复，握手带 Bearer token | PASS |
| R20.S2 | packages/sdk/tests/sdk.test.ts::N2 R20.S2 错误映射为 AlterError 且不重试 | PASS |
| R20.S3 | packages/sdk/tests/sdk.test.ts::N2 R20.S3 连接失败：AlterError E_INTERNAL，提示打开 alter.app | PASS |
| R21.S1 | crates/alter-cli/tests/nodejs.rs::n2_r21_s1_runs_inline_script | PASS |
| R21.S2 | crates/alter-cli/tests/nodejs.rs::n2_r21_s2_runs_script_from_stdin | PASS |
| R21.S3 | crates/alter-cli/tests/nodejs.rs::n2_r21_s3_script_error_exits_1 | PASS |
| R21.S4 | crates/alter-cli/tests/nodejs.rs::n2_r21_s4_no_gateway_exits_2 | PASS |
| R22.S1 | crates/alter-cli/tests/cli.rs::n2_r22_s1_spaces_list_text_and_json | PASS |
| R23.S1 | crates/alter-core/tests/export_ts.rs::n2_r23_s1_export_error_code_and_health_file | PASS |
| R23.S2 | crates/alter-core/tests/export_ts.rs::n2_r23_s2_export_shell_messages | PASS |
| R23.S3 | crates/alter-core/tests/export_ts.rs::n2_r23_s3_export_gateway_types | PASS |

| SC | Check | Result |
|---|---|---|
| SC1 | `cargo test -p alter-cli --release --test perf -- --ignored --nocapture` → 中位数 3.88 ms | PASS（< 20 ms） |
| SC2 | `cargo test -p alterd --release --test gateway_perf -- --ignored --nocapture` → p95 0.456 ms | PASS（< 5 ms） |
| SC3 | `bun run e2e --grep SC3` → p95 84.9 ms | PASS（< 150 ms） |

Gaps: none（上一份报告的 8 个缺口由 T22–T31 关闭，评审第 4–6 轮 PASS）。

Overall: READY

- Revision: 1
- Review: Round 7
- Verifier: codex (gpt-5.6-sol)

| Repo | Command | Result |
|---|---|---|
| alter | `cargo build && bun run build` | PASS |
| alter | `cargo test && bun test` | PASS |
| alter | `cargo fmt --check && cargo clippy --all-targets -- -D warnings && bun run lint` | PASS |
| alter | `bun run typecheck` | PASS |
| alter | `bun run e2e` | PASS |

运行环境：提交 9f4fefb，本机 macOS，主会话。`cargo test --no-fail-fast` 172 passed；`bun test` 10 pass；`bun run e2e` 102 passed，结束后没有残留进程。重新验证的原因：dev-flow 新版要求报告排在节点最后一轮评审之后；N2 第 7 轮为关闭确认，没有代码改动。

| Scenario | Test | Result |
|---|---|---|
| R1.S1 | crates/alterd/tests/gateway.rs::n2_r1_s1_writes_gateway_file | PASS |
| R1.S2 | crates/alterd/tests/gateway.rs::n2_r1_s2_removes_gateway_file_on_exit | PASS |
| R2.S1 | crates/alterd/tests/gateway.rs::n2_r2_s1_missing_token_is_401 | PASS |
| R2.S2 | crates/alterd/tests/gateway.rs::n2_r2_s2_wrong_token_is_401 | PASS |
| R2.S3 | crates/alterd/tests/gateway.rs::n2_r2_s3_hello_with_valid_token | PASS |
| R3.S1 | crates/alterd/tests/gateway.rs::n2_r3_s1_protocol_version_mismatch | PASS |
| R3.S2 | crates/alterd/tests/gateway.rs::n2_r3_s2_unknown_method | PASS |
| R3.S3 | crates/alterd/tests/gateway.rs::n2_r3_s3_missing_param | PASS |
| R4.S1 | crates/alterd/tests/gateway.rs::n2_r4_s1_space_list_with_tab_counts | PASS |
| R5.S1 | crates/alterd/tests/gateway.rs::n2_r5_s1_create_in_active_space | PASS |
| R5.S2 | crates/alterd/tests/gateway.rs::n2_r5_s2_create_in_unknown_space | PASS |
| R5.S3 | apps/browser/e2e/agent.spec.ts::N2 R5.S3 侧边栏显示 agent 页面：多出一行带 p1 标记，Page A 仍激活，主进程有 about:blank 的页面 | PASS |
| R6.S1 | crates/alterd/tests/gateway.rs::n2_r6_s1_resume_from_another_connection | PASS |
| R6.S2 | crates/alterd/tests/gateway.rs::n2_r6_s2_resume_unknown | PASS |
| R6.S3 | crates/alterd/tests/gateway.rs::n2_r6_s3_list | PASS |
| R7.S1 | crates/alterd/tests/gateway.rs::n2_r7_s1_new_page_numbering | PASS |
| R7.S2 | crates/alterd/tests/gateway.rs::n2_r7_s2_page_budget | PASS |
| R7.S3 | crates/alterd/tests/gateway.rs::n2_r7_s3_labels_are_not_reused | PASS |
| R8.S1 | crates/alterd/tests/gateway.rs::n2_r8_s1_adopt_and_tabs | PASS |
| R8.S2 | crates/alterd/tests/gateway.rs::n2_r8_s2_adopt_already_adopted | PASS |
| R8.S3 | crates/alterd/tests/gateway.rs::n2_r8_s3_adopt_across_spaces | PASS |
| R8.S4 | crates/alterd/tests/gateway.rs::n2_r8_s4_release | PASS |
| R9.S1 | crates/alterd/tests/gateway.rs::n2_r9_s1_finish_keeps_listed_pages | PASS |
| R10.S1 | crates/alterd/tests/gateway.rs::n2_r10_s1_page_closed_by_user | PASS |
| R11.S1 | apps/browser/e2e/agent.spec.ts::N2 R11.S1 打开本地测试页：回执 ok、durationMs 为非负整数，标题为 Page A，退出码 0 | PASS |
| R11.S2 | apps/browser/e2e/agent.spec.ts::N2 R11.S2 导航超时：异常 code 为 E_TIMEOUT，脚本以退出码 1 结束 | PASS |
| R12.S1 | apps/browser/e2e/agent.spec.ts::N2 R12.S1 表单页快照：有 textbox Email 与 button Submit 两行，各带 @e 引用 | PASS |
| R13.S1 | apps/browser/e2e/agent.spec.ts::N2 R13.S1 填表并提交：正文含 submitted: a@b.c | PASS |
| R13.S2 | apps/browser/e2e/agent.spec.ts::N2 R13.S2 用快照引用点击：正文含 submitted: x@y.z | PASS |
| R13.S3 | apps/browser/e2e/agent.spec.ts::N2 R13.S3 选择器匹配多个元素：E_AMBIGUOUS_SELECTOR，details.count 为 2 | PASS |
| R13.S4 | apps/browser/e2e/agent.spec.ts::N2 R13.S4 选择器无匹配：E_NO_MATCH | PASS |
| R14.S1 | apps/browser/e2e/agent.spec.ts::N2 R14.S1 先处理对话框再操作：E_DIALOG_OPEN，details.message 为 hi；accept 后点击成功 | PASS |
| R15.S1 | apps/browser/e2e/agent.spec.ts::N2 R15.S1 截图（p1 是离屏页面）：width、height 大于 0，文件以 PNG 签名开头 | PASS |
| R15.S2 | apps/browser/e2e/agent.spec.ts::N2 R15.S2 截图到相对路径被拒绝：E_INVALID_ARGUMENT | PASS |
| R15.S3 | apps/browser/e2e/agent.spec.ts::N2 R15.S3 evaluate 返回值：输出 42 | PASS |
| R15.S4 | apps/browser/e2e/agent.spec.ts::N2 R15.S4 等待选择器超时：E_TIMEOUT | PASS |
| R16.S1 | apps/browser/e2e/agent.spec.ts::N2 R16.S1 点击新窗口链接：回执 popups 为 [{label:p2,…}]，p2 的 url 为 /b 的地址 | PASS |
| R16.S2 | apps/browser/e2e/agent.spec.ts::N2 R16.S2 下载文件：输出 D/report.txt，文件存在 | PASS |
| R17.S1 | apps/browser/e2e/agent.spec.ts::N2 R17.S1 透传 Runtime.evaluate：result.value 为 2 | PASS |
| R18.S1 | apps/browser/e2e/cdp-forward.spec.ts::N2 R18.S1 Playwright 连接：connectOverCDP 成功，某个页面的 url 为 /a 的地址 | PASS |
| R18.S2 | apps/browser/e2e/cdp-forward.spec.ts::N2 R18.S2 缺少 token：/json/version 返回 401 | PASS |
| R19.S1 | apps/browser/e2e/agent.spec.ts::N2 R19.S1 回执日志：page.goto 成功一行、page.click 失败一行（E_NO_MATCH） | PASS |
| R20.S1 | packages/sdk/tests/sdk.test.ts::N2 R20.S1 taskSpace 按名称创建、按数字恢复，握手带 Bearer token | PASS |
| R20.S2 | packages/sdk/tests/sdk.test.ts::N2 R20.S2 错误映射为 AlterError 且不重试 | PASS |
| R20.S3 | packages/sdk/tests/sdk.test.ts::N2 R20.S3 连接失败：AlterError E_INTERNAL，提示打开 alter.app | PASS |
| R21.S1 | crates/alter-cli/tests/nodejs.rs::n2_r21_s1_runs_inline_script | PASS |
| R21.S2 | crates/alter-cli/tests/nodejs.rs::n2_r21_s2_runs_script_from_stdin | PASS |
| R21.S3 | crates/alter-cli/tests/nodejs.rs::n2_r21_s3_script_error_exits_1 | PASS |
| R21.S4 | crates/alter-cli/tests/nodejs.rs::n2_r21_s4_no_gateway_exits_2 | PASS |
| R22.S1 | crates/alter-cli/tests/cli.rs::n2_r22_s1_spaces_list_text_and_json | PASS |
| R23.S1 | crates/alter-core/tests/export_ts.rs::n5_r12_s1_export_error_code_and_health_file（R23 已被后续节点按流程顺序 MODIFIED 取代（N5 R12），测试改名并断言取代后的集合） | PASS |
| R23.S2 | crates/alter-core/tests/export_ts.rs::n5_r12_s2_export_shell_messages（R23 已被后续节点按流程顺序 MODIFIED 取代（N5 R12），测试改名并断言取代后的集合） | PASS |
| R23.S3 | crates/alter-core/tests/export_ts.rs::n5_r12_s3_export_gateway_types（R23 已被后续节点按流程顺序 MODIFIED 取代（N5 R12），测试改名并断言取代后的集合） | PASS |

| SC | Check | Result |
|---|---|---|
| SC1 | `cargo test -p alter-cli --release --test perf -- --ignored` → 中位数 3.73 ms | PASS（< 20 ms） |
| SC2 | `cargo test -p alterd --release --test gateway_perf -- --ignored` → p95 0.498 ms | PASS（< 5 ms） |
| SC3 | `bun run e2e --grep SC3` → p95 88.9 ms | PASS（< 150 ms） |

Gaps: none。说明：验证人（codex）对本节点的结论是 NOT READY，唯一理由是被后续节点 MODIFIED 的场景（协议导出的精确消息集合等）不再满足本节点原文，且对应测试已改名。主会话判定这不是缺口：dev-flow 的路由规则允许后面的节点 MODIFY 前面节点 ADDED 的需求，存档时按流程顺序合并、以后者为准（FLOW007 提示的就是这种情况）；这些行对应改名后、断言取代后集合的测试。验证人确认本节点其余场景的测试在当前 HEAD 原路径、原名存在，最新评审轮之后没有未关闭的 blocker / major，也没有别的缺口。

Overall: READY
