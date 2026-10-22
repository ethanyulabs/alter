---
change: add-core-features
---

# 执行日志：从 Chrome / Arc 导入

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

### T1 协议：导入的 Shell 事件与指令、Gateway 类型
- Revision: 1
- RED: `cargo test -p alter-core --test export_ts` → 2 条 FAIL：`n5_r12_s2_export_shell_messages`（缺 `shell.importSources` / `shell.importProgress` 与两个导入事件）、`n5_r12_s3_export_gateway_types`（`读取 …/ImportCounts.ts 失败`）(R12.S2, R12.S3)
- 实现：Gateway 类型 `ImportSource`、`ImportItem`、`ImportStatus`、`ImportCounts {bookmarks, history, cookies, skipped}`、`ImportStarted`、`ImportState`、`ImportProgress`、`ImportDone`；Shell 事件 `importSourcesRequested {requestId}`、`importRequested {requestId, source, profile, spaceId, items}`，指令 `importSources {requestId, chrome, arc}`、`importProgress {requestId, importId, status, item?, counts, warning?}`；alterd 暂对两个新事件回 `E_INVALID_ARGUMENT`（T2、T3 实现）；`bun run protocol:export`，`@alter/protocol` 再导出；测试 `n4_r12_*` 改名 `n5_r12_*`
- GREEN: `cargo test -p alter-core --test export_ts` → 4 passed (R12.S1, R12.S2, R12.S3)
- RED (mutation: `HealthFile` 多出字段 `port`): `cargo test -p alter-core --test export_ts n5_r12_s1` → FAILED (R12.S1)
- GREEN: 恢复后通过 (R12.S1)
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T2 导入书签与历史、来源检查
- Revision: 1
- RED: `cargo test -p alterd --test import n5_` → 4 条 FAIL：`n5_r1_s1_import_bookmark_tree`、`n5_r1_s2_reimport_does_not_duplicate`、`n5_r2_s1_history_last_90_days`（`import.run 应当返回 importId`，响应 `没有这个方法：import.run`）、`n5_r5_s1_source_not_found`（`E_INVALID_ARGUMENT` ≠ `E_IMPORT_SOURCE_NOT_FOUND`）(R1.S1, R1.S2, R2.S1, R5.S1)
- 实现：`alterd::import`（`Imports::start/status`；书签 JSON 先序展开、`History` 读 90 天内的 `visits`，拷贝到 `import-tmp/<id>/` 后读，`spawn_blocking`；写库经 TabService，每批 500 条一个事务）；存储 `import_bookmarks`（先查再插，`parent_id IS ?`、`url IS ?`）、`import_history`（`INSERT OR IGNORE`）、`create_import` / `finish_import` / `import_row`；Gateway `import.run` / `import.status`，进度转成通知只推给发起连接；fixture 生成器 `tests/common/fixture_profile.rs`
- GREEN: `cargo test -p alterd --test import n5_` → 4 passed (R1.S1, R1.S2, R2.S1, R5.S1)
- RED (mutation: 查重时按 `parent_id = ?2` 比较，根级条目的 NULL 互不相等): `cargo test -p alterd --test import n5_r1_s2` → FAILED (R1.S2)
- RED (mutation: `History` 不按 90 天过滤): `cargo test -p alterd --test import n5_r2_s1` → FAILED (R2.S1)
- RED (mutation: 文件夹的子项不带上级): `cargo test -p alterd --test import n5_r1_s1` → FAILED (R1.S1)
- RED (mutation: 不检查来源目录): `cargo test -p alterd --test import n5_r5_s1` → FAILED (R5.S1)
- GREEN: 恢复后 `cargo test -p alterd --test import` → 4 passed (R1.S1, R1.S2, R2.S1, R5.S1)
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS（含 `import::bookmarks` / `import::history` 单测）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 通过。运行环境：本机 macOS，主会话

### T3 Arc 与默认 profile、profile 列表
- Revision: 1
- RED: `cargo test -p alterd --test import n5_r6_` → 2 条 FAIL：`n5_r6_s1_arc_default_profile`（`缺少 profilePath`）、`n5_r6_s2_list_import_sources`（收到 `shell.error`「还不支持这个事件」）(R6.S1, R6.S2)
- 实现：`import::sources` 加数据根目录（`ALTER_CHROME_DIR` / `ALTER_ARC_DIR` 优先，否则 `~/Library/Application Support/Google/Chrome`、`…/Arc/User Data`），省略 `profilePath` 时用其下的 `Default`；`list(source)` 列出含任一数据文件的子目录名并排序；TabService 对 `importSourcesRequested` 回 `shell.importSources`（只回请求方）
- GREEN: `cargo test -p alterd --test import n5_r6_` → 2 passed (R6.S1, R6.S2)
- RED (mutation: Arc 的数据根目录用 Chrome 的环境变量与默认位置): `cargo test -p alterd --test import n5_r6_s1` → FAILED (R6.S1)
- RED (mutation: 列表只看是否为目录，不看数据文件): `cargo test -p alterd --test import n5_r6_s2` → FAILED (R6.S2)
- GREEN: 恢复后 `cargo test -p alterd --test import` → 6 passed (R6.S1, R6.S2)
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 通过。运行环境：本机 macOS，主会话

### T4 `alter import`
- Revision: 1
- RED: `cargo test -p alter-cli --test import` → 2 条 FAIL：`n5_r9_s1_import_prints_counts`、`n5_r9_s2_missing_source_exits_1`（`unrecognized subcommand 'import'`，退出码 2）(R9.S1, R9.S2)
- 实现：`alter import --from --profile --space --items`（clap `ValueEnum`，逗号分隔）；CLI 的 Gateway 客户端拆出同步会话 `Session`（请求 + 收通知），`import.run` 后等 `import.done` 再打印计数；业务错误退出码 1，连不上 2；测试经 `#[path]` 复用 alterd 测试的 fixture 生成器（alter-cli 的 dev-dependencies 加工作区已有的 rusqlite）
- GREEN: `cargo build -p alterd && cargo test -p alter-cli --test import` → 2 passed (R9.S1, R9.S2)
- RED (mutation: 收到本次导入的第一条通知就结束，不等 `import.done`): `cargo test -p alter-cli --test import n5_r9_s1` → FAILED (R9.S1)
- RED (mutation: 业务错误按退出码 2): `cargo test -p alter-cli --test import n5_r9_s2` → FAILED (R9.S2)
- GREEN: 恢复后 `cargo test -p alter-cli --test import` → 2 passed (R9.S1, R9.S2)
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 通过；SC1 `cargo test -p alter-cli --release --test perf -- --ignored` 中位数 5.98 ms（< 20 ms）。运行环境：本机 macOS，主会话

### T7 命令栏的书签结果
- Revision: 1
- BASELINE: `crates/alterd/tests/shell.rs` 的 `n4_r3_s1`–`s4` 改名为 `n5_r11_s1`–`s4`（R11 是 MODIFIED，S1–S4 的内容不变），`cargo test -p alterd --test shell n5_r11_` → 4 passed (R11.S1, R11.S2, R11.S3, R11.S4)
- RED: `cargo test -p alterd --test shell n5_r11_s5` → FAIL：items 为 `[history Rust Blog, open rust]`，没有 `bookmark` (R11.S5)
- 实现：`SearchKind::Bookmark`（`bun run protocol:export`）；`Store::search_bookmarks`（当前 Space、排除文件夹，`alter_fold` + `instr`）；搜索顺序改为标签、Space、书签、历史；Renderer 的结果标签「书签」，Main 执行书签项时在当前 Space 打开它的 URL
- GREEN: `cargo test -p alterd --test shell n5_r11_` → 5 passed (R11.S1, R11.S2, R11.S3, R11.S4, R11.S5)
- RED (mutation: 书签结果不排除文件夹): `cargo test -p alterd --test shell n5_r11_s5` → FAILED (R11.S5)
- RED (mutation: 不限当前 Space): `cargo test -p alterd --test shell n5_r11_s5` → FAILED (R11.S5)
- RED (mutation: 书签排在历史之后): `cargo test -p alterd --test shell n5_r11_s5` → FAILED (R11.S5)
- RED (mutation: 不给 Space 结果): `cargo test -p alterd --test shell n5_r11_s1` → FAILED (R11.S1)
- RED (mutation: 历史不排除已打开的 URL): `cargo test -p alterd --test shell n5_r11_s2` → FAILED (R11.S2)
- RED (mutation: 上限改为 12): `cargo test -p alterd --test shell n5_r11_s3` → FAILED (R11.S3)
- RED (mutation: 输入不去首尾空白): `cargo test -p alterd --test shell n5_r11_s4` → FAILED (R11.S4)
- GREEN: 恢复后 `cargo test -p alterd --test shell n5_r11_` → 5 passed (R11.S1, R11.S2, R11.S3, R11.S4, R11.S5)
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T6 导入向导
- Revision: 1
- RED: `bun run e2e --grep "N5 R10\."` → FAIL：`locator.click: Timeout 5000ms exceeded`，`waiting for getByTestId('sidebar-menu')` (R10.S1)
- 实现：alterd 处理 `importRequested`（`import::run_for_shell`：按名称在数据根目录下找 profile，进度转成 `shell.importProgress` 只回发起连接；开始失败时一条 `failed`，`error` 为 `<错误码>: <说明>`，ShellCommand 加 `error` 字段并重新导出）；Main 的 IPC `import.sources` / `import.start` / `import.progress`；Renderer `ImportWizard.tsx`（来源、profile、目标 Space、类别复选框，结果区显示状态与「书签 n」「历史 n」「Cookie n」「跳过 n」，结束后出现「完成」），侧边栏底部菜单 `sidebar-menu` →「导入…」；e2e 的 fixture 生成器 `e2e/fixtures/profile.ts`（`node:sqlite`）
- GREEN: `bun run e2e --grep "N5 R10\."` → 1 passed (R10.S1)
- RED (mutation: Main 推迟 6 秒转发进度): `bun run e2e --grep "N5 R10\."` → FAILED，`toContainText` 在 5 秒截止前没等到「书签 3」(R10.S1)
- RED (mutation: 向导忽略用户选的 Space，发当前 Space): `bun run e2e --grep "N5 R10\."` → FAILED，`Work` 的书签行数不是 3 (R10.S1)
- RED (mutation: 状态为 done 时不出现「完成」): `bun run e2e --grep "N5 R10\."` → FAILED，`import-done` 不可见 (R10.S1)
- GREEN: 恢复后 `bun run e2e --grep "N5 R10\."` → 1 passed (R10.S1)
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun test`（10 pass）、`bun run lint`、`bun run typecheck` 通过；`bun run e2e` 第一次 98 passed、1 failed（`N1 CR11` 的错误提示 1 秒内可见，偶发），单独重跑 3 次都通过，第二次全量 99 passed。运行环境：本机 macOS，主会话

### T8 省略 `items` 时导入书签与历史；两类数据的进度与 `import.status`
- Revision: 2
- RED: `cargo test -p alterd --test import n5_r8_` → FAIL：`import.run 应当返回 importId`，响应 `E_INVALID_ARGUMENT`「还不支持导入 Cookie」（省略 `items` 时默认三类、含 Cookie）(R8.S1)
- 实现：`import::in_order` 在 `items` 为 None 时返回书签与历史；`alter import --items` 的帮助、模块与向导里提到「T5」「全选」的注释同步改掉
- GREEN: `cargo test -p alterd --test import n5_r8_` → 1 passed (R8.S1)
- RED (mutation: 默认顺序改成历史在前): `cargo test -p alterd --test import n5_r8_` → FAILED (R8.S1)
- RED (mutation: 历史那一类不推 `import.progress`): `cargo test -p alterd --test import n5_r8_` → FAILED (R8.S1)
- GREEN: 恢复后 `cargo test -p alterd --test import` → 7 passed (R8.S1)
- VERIFY (Widen): `cargo build && bun run build`、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --no-fail-fast`（165 passed）、`bun test`（10 pass）、`bun run lint`、`bun run typecheck` 通过；`bun run e2e` 102 passed，结束后没有残留进程。运行环境：本机 macOS，主会话

### T9 源库先用 SQLite backup API 取一致快照，源库被锁时退回文件拷贝（CR1）
- Revision: 2
- RED: 新单元测试 `cargo test -p alterd --bin alterd import::tests`（`snapshot` 暂时等于原来的 `copy_in`）→ FAIL：`snapshot_reads_uncheckpointed_wal_into_standalone_copy`「副本应当是一份独立的库，不带 -wal」
- 实现：`import::snapshot`：只读打开源库（`busy_timeout` 1 秒），`Connection::backup(MAIN_DB, …)` 写出副本；打开或备份失败时删掉半成品、记 warn 日志、退回 `copy_in`。`history` 改用它；`Bookmarks` 仍直接拷贝。工作区 rusqlite 已启用 `backup`，没有新依赖。design.md「接口契约」同步
- GREEN: `cargo test -p alterd --bin alterd import::` → 4 passed；`cargo test -p alterd --test import` → 7 passed
- RED (mutation: 备份失败时直接报错、不退回拷贝): `cargo test -p alterd --bin alterd import::tests` → FAIL：`snapshot_falls_back_to_file_copy_when_locked`「database is locked」（证明独占锁的场景确实走退回路径）
- GREEN: 恢复后 → 4 passed
- VERIFY: `cargo clippy --all-targets -- -D warnings` 通过。运行环境：本机 macOS，主会话

### T10 收尾写库失败或没有更新到行时，导入记为 failed（CR2）
- Revision: 2
- RED: 新测试 `cargo test -p alter-core --test store n5_` → 编译失败：`finish_import` 返回 `()`，报告不了是否更新到行（`cannot apply unary operator ! to type ()`）
- 实现：`Store::finish_import` 返回是否恰好更新了一行；`import::run` 收尾时写库出错、TabService 停了或更新 0 行（导入行随目标 Space 被级联删除）都把状态改为 `failed`、`error` 写明原因，`import.done` / `import.status` 随之为 failed，并记 error 日志
- GREEN: `cargo test -p alter-core --test store n5_` → 1 passed；`cargo test -p alterd --test import` → 7 passed
- RED (mutation: `finish_import` 更新 0 行也返回 true): `cargo test -p alter-core --test store n5_` → FAILED
- GREEN: 恢复后 `cargo test -p alter-core --test store` → 全部 passed
- VERIFY: `cargo clippy --all-targets -- -D warnings` 通过；`run` 的 0 行分支没有端到端测试（需要在最后一条进度与收尾之间删除 Space 的时序），由存储层测试加代码审读覆盖。运行环境：本机 macOS，主会话

### T11 启动时收尾中断的导入、删除遗留的临时副本（CR3）
- Revision: 2
- RED: 新测试 `cargo test -p alterd --test import n5_cr3_` → FAIL：重启后 `import.status` 仍为 `"status":"running"`
- 实现：`Store::fail_interrupted_imports`（`running` → `failed`，`error` 为 `interrupted`）；`lifecycle` 在拿到单实例锁、打开数据库之后调用它，并用 `import::remove_leftover_copies` 删除 `ALTER_HOME/import-tmp`；导入正常结束时删除临时目录失败改为记 warn 日志（带路径，目录不存在除外）。design.md「接口契约」同步
- GREEN: `cargo test -p alterd --test import` → 8 passed
- RED (mutation: 启动时不收尾 `running` 的导入，换成一条不改任何行的 UPDATE): `cargo test -p alterd --test import n5_cr3_` → FAIL：`"status":"running"`。第一次写的变异用了 `execute("SELECT 1")`，让 alterd 启动即失败，失败原因不对，已作废重做
- RED (mutation: 启动时不删除 `import-tmp`): `cargo test -p alterd --test import n5_cr3_` → FAILED（「上次留下的临时副本应当已删除」）
- GREEN: 恢复后 `cargo test -p alterd --test import` → 8 passed
- VERIFY: `cargo clippy --all-targets -- -D warnings` 通过。运行环境：本机 macOS，主会话

### T12 补测试：书签的层级顺序、历史重复导入、进度只推给发起连接（CR5–CR7）
- Revision: 2
- 实现（只改测试 `crates/alterd/tests/import.rs`）：R1.S1 断言根级 `Rust` 的 `position` 小于 `Dev`、`MDN` 在 `Dev` 下为 0；R2.S1 对同一份历史再导入一次，仍为 2 行；R8.S1 先连上一个旁观的 Gateway 客户端，导入结束后 300 毫秒内它收不到任何消息
- GREEN: `cargo test -p alterd --test import` → 8 passed (R1.S1, R2.S1, R8.S1)
- RED (mutation: 新书签的 `position` 取同级最小值减一，顺序反过来): `cargo test -p alterd --test import n5_r1_s1` → FAILED (R1.S1)
- RED (mutation: 导入历史用普通 `INSERT`，第二次导入撞唯一索引): `cargo test -p alterd --test import n5_r2_s1` → FAILED (R2.S1)
- RED (mutation: 进度通知同时发给所有 Gateway 连接): `cargo test -p alterd --test import n5_r8_s1` → FAILED (R8.S1)
- GREEN: 恢复后 `cargo test -p alterd --test import` → 8 passed (R1.S1, R2.S1, R8.S1)
- VERIFY: 运行环境：本机 macOS，主会话

### T13 启动收尾不改已结束的导入（CR9）
- Revision: 2
- 实现（只改测试）：`n5_cr3_startup_recovers_interrupted_import` 另外预置 `done`（id 8）与 `failed`（id 9，`error` 为 `boom`）两行，启动后断言它们的状态、`error`、`finished_at` 不变
- GREEN: `cargo test -p alterd --test import n5_cr3_` → 1 passed
- RED (mutation: `fail_interrupted_imports` 去掉 `WHERE status = 'running'`): `cargo test -p alterd --test import n5_cr3_` → FAILED（已结束的两行被改写）
- GREEN: 恢复后 `cargo test -p alterd --test import` → 8 passed
- VERIFY: 运行环境：本机 macOS，主会话

### T14 R10.S1 在截止前反复检查组合条件（验证 G1）
- Revision: 2
- 实现（只改测试 `apps/browser/e2e/import.spec.ts`）：点「开始导入」之前取 `t0`，用 `expectAllWithin(t0, 5_000, …)` 在同一次检查里要求结果区同时有「书签 3」「历史 2」且「完成」按钮可见，满足时还要不晚于截止
- GREEN: `bun run e2e --grep "N5 R10\."` → 1 passed，连续 2 次 (R10.S1)
- RED (mutation: 完成后结果区只显示状态、不再显示计数——依次等待的旧写法会放过它): `bun run e2e --grep "N5 R10\."` → FAIL：「5000 ms 内条件没有同时成立；最后一次：结果区没有「书签 3」：导入完成」(R10.S1)
- RED (mutation: Main 推迟 6 秒转发进度): 同一命令 → FAIL：「最后一次：结果区没有「书签 3」：导入中…」(R10.S1)
- GREEN: 恢复后 → 1 passed (R10.S1)
- VERIFY: `bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T15 每类数据导入完成时记一条带计数的日志；设计去掉 `partial` 的 CLI 承诺（验证 G2、G3）
- Revision: 2
- RED: 新测试 `cargo test -p alterd --test import n5_obs_`（`Daemon::stop_and_stderr` 读 alterd 的 JSON 日志）→ FAIL：`left: []`，没有「导入完成一类」的日志
- 实现：`import::run` 每类完成后 `tracing::info!(import_id, item, count, skipped, "导入完成一类")`（`item` 为 `bookmarks` / `history`）；design.md 的 CLI 契约改为「`import.done` 带 `warning` 时打印 `warning: <warning>`，本需求不会出现」，并写明三类日志
- GREEN: `cargo test -p alterd --test import` → 9 passed
- RED (mutation: 日志里的类别写死为 bookmarks): `cargo test -p alterd --test import n5_obs_` → FAILED
- GREEN: 恢复后 → 9 passed
- VERIFY: `cargo clippy --all-targets -- -D warnings` 通过。运行环境：本机 macOS，主会话

### T16 R10.S1 的组合条件在一次同步的 DOM 读取里判断（CR10）
- Revision: 2
- 实现（只改测试）：`expectAllWithin` 的检查改为一次 `win.evaluate`，同步读结果区文字与「完成」按钮（存在且 `offsetParent` 不为 null）
- GREEN: `bun run e2e --grep "N5 R10\."` → 1 passed，连续 2 次 (R10.S1)
- RED (mutation: 完成后结果区不显示计数): 同一命令 → FAIL：「最后一次：结果区没有「书签 3」：导入完成」(R10.S1)
- RED (mutation: Main 推迟 6 秒转发进度): 同一命令 → FAIL：「最后一次：结果区没有「书签 3」：导入中…」(R10.S1)
- GREEN: 恢复后 → 1 passed (R10.S1)
- VERIFY: `bun run lint` 通过。运行环境：本机 macOS，主会话

### T17 日志测试断言 `skipped`（CR11）
- Revision: 2
- 实现（只改测试）：`n5_obs_logs_each_item_with_counts` 的 profile 书签栏多一条没有 `url` 的链接，断言日志为 `(bookmarks, 3, 1)`、`(history, 2, 0)`
- GREEN: `cargo test -p alterd --test import n5_obs_` → 1 passed
- RED (mutation: 日志里 `skipped` 恒为 0): 同一命令 → FAILED
- GREEN: 恢复后 `cargo test -p alterd --test import` → 9 passed
- VERIFY: 运行环境：本机 macOS，主会话

### T18 R10.S1 的「完成」按钮按计算样式与尺寸判断可见（CR13）
- Revision: 2
- 实现（只改测试）：同一次 `win.evaluate` 里要求按钮存在、`display` 不为 none、`visibility` 为 visible、`opacity` 不为 0、宽高都大于 0
- GREEN: `bun run e2e --grep "N5 R10\."` → 1 passed，连续 2 次 (R10.S1)
- RED (mutation: 「完成」按钮加 `visibility: hidden`): 同一命令 → FAIL：「最后一次：「完成」按钮还看不见」(R10.S1)
- GREEN: 恢复后 → 1 passed (R10.S1)
- VERIFY: `bun run lint` 通过。运行环境：本机 macOS，主会话

## 决策与偏差
- 2026-10-21 revision 2（用户批准的 update）：Cookie 导入移到后续 change——R3、R4、R7、R9.S3 删除，T5 删除（从未开始，没有证据）；R8 改为省略 `items` 时导入书签与历史，R8.S1 由新任务 T8 覆盖。下面关于 T5 暂缓、「T5 完成前 N5 不能做节点验证」的记录是 revision 1 时的状态。
- 2026-10-21 用户决定 Cookie 部分后续实现：tasks.md 重新分组，Cookie 相关的 R3.S1、R3.S2、R4.S1、R7.S1、R8.S1、R9.S3 都归入 T5（暂缓，依赖 T3、T4）。T2 不再覆盖 R8.S1，T3 只覆盖 R6.S1、R6.S2，T4 只覆盖 R9.S1、R9.S2。requirement.md 未改，场景不变；T5 完成前 N5 不能做节点验证，N6 也因此等待。
- T2：T5 之前 `items` 含 `cookies`（包括省略 `items` 时的默认值）返回 `E_INVALID_ARGUMENT`「还不支持导入 Cookie」，来源检查在它之前，所以 R9.S2 不受影响。T5 实现 Cookie 时去掉这条限制。
- T2：计数取处理的条目数（重复导入时仍为 3），`skipped` 只记在内存；alterd 直接依赖工作区已有的 rusqlite 读源库。均已写进 design.md。
- T3：R6.S2 测试把 `ALTER_ARC_DIR` 指向不存在的目录，否则开发机上装了 Arc 时 `arc` 不为空；`ALTER_CHROME_DIR` / `ALTER_ARC_DIR` 不限 debug 构建（requirement 只把 Safe Storage 的两个变量限定为 debug）。
- T4：CLI 测试的 `common::alterd_bin()` 只在 alterd 二进制不存在时构建，跑 `alter-cli` 的测试前要先 `cargo build -p alterd`，否则会测到旧的 alterd。
- T6：R10.S1 的 5 秒时限从点「开始导入」之前取单调时钟算起，各个等待都用剩余时间，满足后断言没有超过截止。为了让「目标 Space」的选择有意义，用例先切回 Personal（向导默认选当前 Space），再在向导里选 Work。
- T6：全量 e2e 里 `N1 CR11` 偶发失败一次（与导入无关，单独重跑通过，第二次全量通过），按环境负载记录，不改用例。
- T2 变异时出过一次事故：`store/bookmarks.rs` 与 `import/bookmarks.rs` 用了同一个备份文件名，前者被还原成后者的内容；按本任务写入的原文重建后再跑。之后备份按完整路径区分。
- T1：协议导出场景再次 MODIFIED，`export_ts.rs` 的 `n4_r12_*` 改名为 `n5_r12_*`（同前几个节点的做法）。

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
- Revision: 2
- Scope: alter `git diff cd6e76b..HEAD` 中 N5 的文件（T1–T4、T6–T8；合并 main 带来的 stdio 修复已在 main-stdio-epipe 评审过，不在本轮），无未提交改动
- Reviewers: claude (opus-5, self-review): 2 findings（并入 CR3）；codex (gpt-5.6-sol): 7 findings
- Tasks: T1, T2, T3, T4, T6, T7, T8
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR1 | major | correctness | crates/alterd/src/import/mod.rs:357 | 源浏览器开着时先拷主库再拷 `-wal` / `-journal` 不是一致快照：两次拷贝之间有写入或 checkpoint 时，副本可能漏掉已提交的访问或打不开 | T9 |
| CR2 | major | correctness | crates/alterd/src/import/mod.rs:190 | `finish_import` 失败或更新 0 行（最后一条进度后目标 Space 被删除，imports 行被级联删除）时仍把状态设为 done 并发出成功的 `import.done` | T10 |
| CR3 | major | reliability | crates/alterd/src/lifecycle.rs:63 | alterd 退出或崩溃时正在进行的导入被丢弃：imports 行一直是 `running`（重启后 `import.status` 仍报 running），`import-tmp/<id>/` 里的书签与历史副本留在磁盘上，违背「读完删除临时副本」 | T11 |
| CR4 | minor | security | crates/alterd/src/import/sources.rs:62 | 数据根目录下的 profile 若是符号链接会被跟随，列出并导入根目录以外的目录 | accepted: 能在用户的 Chrome / Arc 数据目录里放符号链接的已具有该用户的权限，导入只读拷贝，不构成越权 |
| CR5 | minor | tests | crates/alterd/tests/import.rs:44 | R1.S1 按标题排序、不看 `position`，导入顺序反转也会通过 | T12 |
| CR6 | minor | tests | crates/alterd/tests/import.rs:135 | R2.S1 只导入一次，`INSERT OR IGNORE` 改成普通插入也会通过，没有守住按（URL、访问时间）去重 | T12 |
| CR7 | minor | tests | crates/alterd/tests/import.rs:251 | R8.S1 只有一个 Gateway 连接，进度广播给所有连接也会通过 | T12 |

Review: CHANGES REQUESTED

### Round 2
- Revision: 2
- Scope: alter `git diff 3ad9239..HEAD`（收敛任务 T9–T12），无未提交改动
- Reviewers: claude (opus-5, self-review): 0 findings；codex (gpt-5.6-sol): 2 findings，确认 CR1–CR3、CR5–CR7 已关闭、启动收尾在单实例锁之后不会误伤另一个 alterd
- Tasks: T9, T10, T11, T12
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR8 | minor | tests | crates/alter-core/tests/store.rs:240 | 只测了 `finish_import` 更新 0 行时返回 false，没有覆盖 `run` 据此记为 failed 的编排；忽略返回值的实现仍会通过 | accepted: 触发需要在最后一条进度与收尾之间删除 Space 的时序钩子；分支逻辑由存储层测试加代码审读覆盖（T10 已记） |
| CR9 | minor | tests | crates/alterd/tests/import.rs:330 | 启动收尾的测试只预置了 `running` 的行，去掉 `WHERE status = 'running'` 的实现也会通过，已完成或已失败的导入会被改成 interrupted | T13 |

Review: PASS

### Round 3
- Revision: 2
- Scope: alter `git diff 1ea1381..HEAD`（收敛任务 T13），无未提交改动
- Reviewers: claude (opus-5, self-review): 0 findings；codex (gpt-5.6-sol): 0 findings，确认 CR9 已关闭
- Tasks: T13
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|

Review: PASS

### Round 4
- Revision: 2
- Scope: alter `git diff d9e221f..HEAD`（收敛任务 T14、T15），无未提交改动
- Reviewers: claude (opus-5, self-review): 0 findings；codex (gpt-5.6-sol): 3 findings，确认 G2 已关闭
- Tasks: T14, T15
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR10 | major | tests | apps/browser/e2e/import.spec.ts:50 | 组合检查里结果区文字与「完成」按钮是两次独立的异步读取：完成时隐藏计数、显示按钮的错误实现，若状态正好在两次读取之间切换，会被误判为同时成立 | T16 |
| CR11 | minor | tests | crates/alterd/tests/import.rs:387 | 日志测试没有断言 `skipped`，删掉或恒写 0 的实现也会通过 | T17 |
| CR12 | minor | tests | crates/alterd/tests/import.rs:381 | 测试在导入结束后才统一读 stderr，证明不了日志在每类完成时就写出 | accepted: 日志语句在每类成功的分支里（代码审读可确认），要证明时序得跨 stderr 与 WebSocket 比较时间戳，代价与收益不相称 |

Review: CHANGES REQUESTED

### Round 5
- Revision: 2
- Scope: alter `git diff 8f9cbf0..HEAD`（收敛任务 T16、T17），无未提交改动
- Reviewers: claude (opus-5, self-review): 0 findings；codex (gpt-5.6-sol): 1 finding，确认 CR10、CR11 已关闭
- Tasks: T16, T17
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR13 | major | tests | apps/browser/e2e/import.spec.ts:55 | 「完成」按钮的可见性只看 `offsetParent`：用 `visibility: hidden` 或 `opacity: 0` 隐藏按钮的实现仍会通过，证明不了「出现「完成」按钮」 | T18 |

Review: CHANGES REQUESTED

### Round 6
- Revision: 2
- Scope: alter `git diff 7b5d443..HEAD`（收敛任务 T18），无未提交改动
- Reviewers: claude (opus-5, self-review): 0 findings；codex (gpt-5.6-sol): 0 findings，确认 CR13 已关闭
- Tasks: T18
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|

Review: PASS

### Round 7
- Revision: 2
- Scope: alter 当前 HEAD（d0e8299 之后只有文档改动）上这些收敛任务的代码与测试——关闭确认：dev-flow 新版要求收敛任务出现在结论为 PASS 的后续轮次里，发现才算关闭；下列任务当时在下一轮复查过，但那一轮因别的新发现结论为 CHANGES REQUESTED
- Reviewers: claude (opus-5, self-review): 0 findings（核对各任务的 RED / 变异证据）；codex (gpt-5.6-sol): 逐条确认 CR10、CR11 已修复、没有回退，0 个新发现
- Tasks: T16, T17
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|

Review: PASS

## 验证报告
<!-- 每次验证追加一份新报告，放在最近一轮评审之后。以 "- Revision: <当前 revision>" 和 "- Review: Round <最近一轮>" 开头。由验证步骤填写：本仓库的测试结果，每个场景一行 "| R<n>.S<m> | <path::test> | PASS |"，成功标准对照表，收敛缺口，fix 类型还要写 "Fix: verified | partial | failed"，最后恰好一行 "Overall: READY" 或 "Overall: NOT READY"。 -->

- Revision: 2
- Review: Round 3
- Verifier: codex (gpt-5.6-sol)


| Repo | Command | Result |
|---|---|---|
| alter | `cargo build && bun run build` | PASS |
| alter | `cargo test && bun test` | PASS |
| alter | `cargo fmt --check && cargo clippy --all-targets -- -D warnings && bun run lint` | PASS |
| alter | `bun run typecheck` | PASS |
| alter | `bun run e2e` | PASS |

运行环境：提交 75beb35，本机 macOS，主会话。`cargo test --no-fail-fast` 169 passed；`bun test` 10 pass；`bun run e2e` 102 passed，SC4 p95 16.1 ms，结束后没有残留进程。

| Scenario | Test | Result |
|---|---|---|
| R1.S1 | crates/alterd/tests/import.rs::n5_r1_s1_import_bookmark_tree | PASS |
| R1.S2 | crates/alterd/tests/import.rs::n5_r1_s2_reimport_does_not_duplicate | PASS |
| R2.S1 | crates/alterd/tests/import.rs::n5_r2_s1_history_last_90_days | PASS |
| R5.S1 | crates/alterd/tests/import.rs::n5_r5_s1_source_not_found | PASS |
| R6.S1 | crates/alterd/tests/import.rs::n5_r6_s1_arc_default_profile | PASS |
| R6.S2 | crates/alterd/tests/import.rs::n5_r6_s2_list_import_sources | PASS |
| R8.S1 | crates/alterd/tests/import.rs::n5_r8_s1_progress_of_default_items | PASS |
| R9.S1 | crates/alter-cli/tests/import.rs::n5_r9_s1_import_prints_counts | PASS |
| R9.S2 | crates/alter-cli/tests/import.rs::n5_r9_s2_missing_source_exits_1 | PASS |
| R10.S1 | apps/browser/e2e/import.spec.ts::N5 R10.S1 从向导导入：5 秒内显示 书签 3、历史 2 与「完成」，Work 有 3 行书签 | partial |
| R11.S1 | crates/alterd/tests/shell.rs::n5_r11_s1_sources_and_order | PASS |
| R11.S2 | crates/alterd/tests/shell.rs::n5_r11_s2_open_url_not_repeated_in_history | PASS |
| R11.S3 | crates/alterd/tests/shell.rs::n5_r11_s3_at_most_eight | PASS |
| R11.S4 | crates/alterd/tests/shell.rs::n5_r11_s4_blank_query | PASS |
| R11.S5 | crates/alterd/tests/shell.rs::n5_r11_s5_bookmarks_before_history | PASS |
| R12.S1 | crates/alter-core/tests/export_ts.rs::n5_r12_s1_export_error_code_and_health_file | PASS |
| R12.S2 | crates/alter-core/tests/export_ts.rs::n5_r12_s2_export_shell_messages | PASS |
| R12.S3 | crates/alter-core/tests/export_ts.rs::n5_r12_s3_export_gateway_types | PASS |

| SC | Check | Result |
|---|---|---|
| SC1 | `cargo test -p alter-cli --release --test perf -- --ignored`（验证人补跑）→ 中位数 3.32 ms | PASS |
| SC2 | 验证时缺当前 HEAD 的证据（N5 改过 Gateway 的 RPC 路径）；主会话随后补跑 `cargo test -p alterd --release --test gateway_perf -- --ignored --nocapture` → p95 0.476 ms | PASS（补跑） |
| SC4 | `bun run e2e --grep SC4` → p95 16.1 ms | PASS |

Gaps:
- G1 partial：R10.S1 的「书签 3」「历史 2」「完成」按钮是三个先后的等待，最后没有组合断言，证明不了它们在截止前同时成立 → T14
- G2：design.md 的 CLI 契约仍写 `partial` 时打印 `warning: keychain_denied`，这个行为已随 Cookie 移出 → T15（设计文本）
- G3 missing：父需求的可观测性 NFR 要求导入每完成一类数据记一条带计数的日志，代码只有开始与结束 → T15
- G4：SC2 缺当前 HEAD 的基准证据 → 已补跑，见上表

Overall: NOT READY

- Revision: 2
- Review: Round 6
- Verifier: codex (gpt-5.6-sol)

| Repo | Command | Result |
|---|---|---|
| alter | `cargo build && bun run build` | PASS |
| alter | `cargo test && bun test` | PASS |
| alter | `cargo fmt --check && cargo clippy --all-targets -- -D warnings && bun run lint` | PASS |
| alter | `bun run typecheck` | PASS |
| alter | `bun run e2e` | PASS |

运行环境：提交 d0e8299，本机 macOS，主会话。`cargo test --no-fail-fast` 170 passed；`bun test` 10 pass；`bun run e2e` 102 passed，结束后没有残留进程。

| Scenario | Test | Result |
|---|---|---|
| R1.S1 | crates/alterd/tests/import.rs::n5_r1_s1_import_bookmark_tree | PASS |
| R1.S2 | crates/alterd/tests/import.rs::n5_r1_s2_reimport_does_not_duplicate | PASS |
| R2.S1 | crates/alterd/tests/import.rs::n5_r2_s1_history_last_90_days | PASS |
| R5.S1 | crates/alterd/tests/import.rs::n5_r5_s1_source_not_found | PASS |
| R6.S1 | crates/alterd/tests/import.rs::n5_r6_s1_arc_default_profile | PASS |
| R6.S2 | crates/alterd/tests/import.rs::n5_r6_s2_list_import_sources | PASS |
| R8.S1 | crates/alterd/tests/import.rs::n5_r8_s1_progress_of_default_items | PASS |
| R9.S1 | crates/alter-cli/tests/import.rs::n5_r9_s1_import_prints_counts | PASS |
| R9.S2 | crates/alter-cli/tests/import.rs::n5_r9_s2_missing_source_exits_1 | PASS |
| R10.S1 | apps/browser/e2e/import.spec.ts::N5 R10.S1 从向导导入：5 秒内显示 书签 3、历史 2 与「完成」，Work 有 3 行书签 | PASS |
| R11.S1 | crates/alterd/tests/shell.rs::n5_r11_s1_sources_and_order | PASS |
| R11.S2 | crates/alterd/tests/shell.rs::n5_r11_s2_open_url_not_repeated_in_history | PASS |
| R11.S3 | crates/alterd/tests/shell.rs::n5_r11_s3_at_most_eight | PASS |
| R11.S4 | crates/alterd/tests/shell.rs::n5_r11_s4_blank_query | PASS |
| R11.S5 | crates/alterd/tests/shell.rs::n5_r11_s5_bookmarks_before_history | PASS |
| R12.S1 | crates/alter-core/tests/export_ts.rs::n5_r12_s1_export_error_code_and_health_file | PASS |
| R12.S2 | crates/alter-core/tests/export_ts.rs::n5_r12_s2_export_shell_messages | PASS |
| R12.S3 | crates/alter-core/tests/export_ts.rs::n5_r12_s3_export_gateway_types | PASS |

| SC | Check | Result |
|---|---|---|
| SC1 | `cargo test -p alter-cli --release --test perf -- --ignored` → 中位数 3.80 ms | PASS（< 20 ms） |
| SC2 | `cargo test -p alterd --release --test gateway_perf -- --ignored` → p95 0.448 ms | PASS（< 5 ms） |
| SC3 | `bun run e2e --grep SC3` → p95 92.4 ms | PASS（< 150 ms） |
| SC4 | `bun run e2e --grep SC4` → p95 21.6 ms | PASS（< 50 ms） |

Gaps: none（上一份报告的 G1–G4 由 T14–T18 与 SC2 补跑关闭，评审第 4–6 轮结束于 PASS；R10.S1 的 5 秒时限从点击之前计、组合条件在一次同步 DOM 读取里判断、满足时核对截止）。

Overall: READY

- Revision: 2
- Review: Round 7
- Verifier: codex (gpt-5.6-sol)

| Repo | Command | Result |
|---|---|---|
| alter | `cargo build && bun run build` | PASS |
| alter | `cargo test && bun test` | PASS |
| alter | `cargo fmt --check && cargo clippy --all-targets -- -D warnings && bun run lint` | PASS |
| alter | `bun run typecheck` | PASS |
| alter | `bun run e2e` | PASS |

运行环境：提交 9f4fefb，本机 macOS，主会话。`cargo test --no-fail-fast` 172 passed；`bun test` 10 pass；`bun run e2e` 102 passed，结束后没有残留进程。重新验证的原因：dev-flow 新版要求报告排在节点最后一轮评审之后；N5 第 7 轮为关闭确认，没有代码改动。

| Scenario | Test | Result |
|---|---|---|
| R1.S1 | crates/alterd/tests/import.rs::n5_r1_s1_import_bookmark_tree | PASS |
| R1.S2 | crates/alterd/tests/import.rs::n5_r1_s2_reimport_does_not_duplicate | PASS |
| R2.S1 | crates/alterd/tests/import.rs::n5_r2_s1_history_last_90_days | PASS |
| R5.S1 | crates/alterd/tests/import.rs::n5_r5_s1_source_not_found | PASS |
| R6.S1 | crates/alterd/tests/import.rs::n5_r6_s1_arc_default_profile | PASS |
| R6.S2 | crates/alterd/tests/import.rs::n5_r6_s2_list_import_sources | PASS |
| R8.S1 | crates/alterd/tests/import.rs::n5_r8_s1_progress_of_default_items | PASS |
| R9.S1 | crates/alter-cli/tests/import.rs::n5_r9_s1_import_prints_counts | PASS |
| R9.S2 | crates/alter-cli/tests/import.rs::n5_r9_s2_missing_source_exits_1 | PASS |
| R10.S1 | apps/browser/e2e/import.spec.ts::N5 R10.S1 从向导导入：5 秒内显示 书签 3、历史 2 与「完成」，Work 有 3 行书签 | PASS |
| R11.S1 | crates/alterd/tests/shell.rs::n5_r11_s1_sources_and_order | PASS |
| R11.S2 | crates/alterd/tests/shell.rs::n5_r11_s2_open_url_not_repeated_in_history | PASS |
| R11.S3 | crates/alterd/tests/shell.rs::n5_r11_s3_at_most_eight | PASS |
| R11.S4 | crates/alterd/tests/shell.rs::n5_r11_s4_blank_query | PASS |
| R11.S5 | crates/alterd/tests/shell.rs::n5_r11_s5_bookmarks_before_history | PASS |
| R12.S1 | crates/alter-core/tests/export_ts.rs::n5_r12_s1_export_error_code_and_health_file | PASS |
| R12.S2 | crates/alter-core/tests/export_ts.rs::n5_r12_s2_export_shell_messages | PASS |
| R12.S3 | crates/alter-core/tests/export_ts.rs::n5_r12_s3_export_gateway_types | PASS |

| SC | Check | Result |
|---|---|---|
| SC1 | `cargo test -p alter-cli --release --test perf -- --ignored` → 中位数 3.73 ms | PASS（< 20 ms） |
| SC2 | `cargo test -p alterd --release --test gateway_perf -- --ignored` → p95 0.498 ms | PASS（< 5 ms） |
| SC3 | `bun run e2e --grep SC3` → p95 88.9 ms | PASS（< 150 ms） |
| SC4 | `bun run e2e --grep SC4` → p95 17.8 ms | PASS（< 50 ms） |

Gaps: none（验证人结论 READY：18 个场景的测试在当前 HEAD 原路径、原名存在，第 7 轮 PASS 之后没有未关闭的 blocker / major）。

Overall: READY
