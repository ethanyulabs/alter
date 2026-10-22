---
change: add-core-features
---

# 执行日志：Space 管理与标签整理

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

### T1 基线：迁移改动前，WAL / 外键与 alterd 启动退出的已有行为
- Revision: 1
- BASELINE: `cargo test -q -p alter-core --test store r1_s3` → PASS on unchanged code（`r1_s3_wal_and_foreign_keys`，1 passed）(R13.S3)
- BASELINE: `cargo test -q -p alterd --test lifecycle r3_s2` → PASS on unchanged code（`r3_s2_sigterm_exits_cleanly`，1 passed）(R15.S2)
- BASELINE: `cargo test -q -p alterd --test lifecycle r3_s3` → PASS on unchanged code（`r3_s3_second_instance_is_rejected`、`r3_s3_concurrent_start_leaves_one_instance`，2 passed）(R15.S3)
- BASELINE: `cargo test -q -p alterd --test lifecycle r3_s4` → PASS on unchanged code（`r3_s4_stale_health_file_is_taken_over`，1 passed）(R15.S4)
- VERIFY: `cargo test -p alter-core --test store && cargo test -p alterd --test lifecycle` → PASS（store 3 passed；lifecycle 8 passed）

### T2 迁移 0002 与迁移前备份
- Revision: 1
- RED: `cargo test -q -p alter-core --test store n1_` → FAIL: `n1_r13_s1_fresh_open_creates_schema` assertion `left == right` failed left: 1 right: 2（还没有迁移 2）(R13.S1)
- RED: `cargo test -q -p alter-core --test store n1_` → FAIL: `n1_r13_s2_reopen_is_idempotent` left: 1 right: 2（schema_version 只有一行）(R13.S2)
- GREEN: `cargo test -q -p alter-core --test store` → PASS (3 tests)，新增 `0002_spaces_tabs.sql`（加两列、回填 `pinned_url`）并登记到 `MIGRATIONS` (R13.S1)
- GREEN: `cargo test -q -p alter-core --test store` → PASS (3 tests) (R13.S2)
- RED: `cargo test -q -p alter-core --test store n1_r13_s4` → FAIL: left: (0, 5, 9) right: (1, 0, 2)（迁移还没有重新编号）(R13.S4)
- GREEN: `cargo test -q -p alter-core --test store` → PASS (4 tests)，迁移里用临时表 + `ROW_NUMBER()` + `UPDATE … FROM` 重新编号 (R13.S4)
- RED (mutation: 换回 design.md 原来的相关子查询 UPDATE，同时测试加了第二个 Space「两个普通标签在固定标签之前」): `cargo test -q -p alter-core --test store n1_r13_s4` → FAIL: left: (1, 1, 0) right: (1, 2, 0)（编号重复）(R13.S4)
- GREEN: 恢复临时表写法后 `cargo test -q -p alter-core --test store n1_r13_s4` → PASS (1 test) (R13.S4)
- RED: `cargo test -q -p alter-core --test store n1_r14` → FAIL: `n1_r14_s1_upgrade_from_v1_writes_backup` panicked: 应当生成 alter.db.bak-1 (R14.S1)
- GREEN: `cargo test -q -p alter-core --test store` → PASS (5 tests)，`migrate::apply` 在 `current >= 1 && pending` 时先 `conn.backup(MAIN_DB, "<db>.bak-<current>")`；根 `Cargo.toml` 给 rusqlite 加 `backup` feature (R14.S1)
- RED (mutation: 备份错误改为 `let _ =` 忽略): `cargo test -q -p alter-core --test store backup_failure` → FAIL: `n1_r14_s1_backup_failure_aborts_migration` panicked: 备份失败时打开应当失败 (R14.S1)
- GREEN: 恢复 `?` 后 `cargo test -q -p alter-core --test store` → PASS (7 tests) (R14.S1)
- RED (mutation: 条件去掉 `current >= 1`): `cargo test -q -p alter-core --test store n1_r14_s2` → FAIL: 不应有备份文件：["alter.db.bak-0"] (R14.S2)
- RED (mutation: 条件去掉 `pending`): `cargo test -q -p alter-core --test store n1_r14_s2` → FAIL: 不应有备份文件：["alter.db.bak-2"] (R14.S2)
- GREEN: 恢复 `current >= 1 && pending` 后 `cargo test -q -p alter-core --test store` → PASS (7 tests) (R14.S2)
- VERIFY: `cargo test -q -p alter-core --test store n1_` → PASS (6 tests)；R13.S3 的 `r1_s3_wal_and_foreign_keys` 仍通过
- VERIFY (Widen，含 T3): `cargo test --no-fail-fast` → cli 4、alter-core 1+2+7、lifecycle 8、shell 21 全部 PASS；`bun test` 1 pass；`cargo fmt --check`（先跑过一次 `cargo fmt` 修测试里的换行）、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 均无输出错误

### T3 alterd 启动后数据库为版本 2
- Revision: 1
- RED (mutation: `MIGRATIONS` 里去掉迁移 2 的登记；期望值已改为 2 的 `n1_r15_s1_run_writes_health_file`): `cargo test -q -p alterd --test lifecycle n1_r15_s1` → FAIL: assertion `left == right` failed left: 1 right: 2 (R15.S1)
- GREEN: 恢复登记后 `cargo test -q -p alterd --test lifecycle` → PASS (8 tests) (R15.S1)

### T4 协议：Space / 固定 / 移动事件、`shell.discard`、`TabView.pinned`、`SPACE_COLORS` 导出
- Revision: 1
- RED: `cargo test -q -p alter-core --test export_ts` → FAIL: `n1_r16_s2_export_shell_messages` left: {"shell.error", "shell.navigate", "shell.stateChanged"} right: {"shell.discard", …}（还没有新消息）(R16.S2)
- GREEN: `cargo test -q -p alter-core --test export_ts` → PASS (3 tests)，`shell.rs` 加 `ShellCommand::Discard`、6 个 `ShellEvent` 变体、`TabView.pinned`、`SPACE_COLORS`，`export_ts` 另写 `spaceColors.ts` (R16.S2)
- RED (mutation: `export_ts` 里注释掉 `HealthFile::export_all`；`r13_s1` 改名为 `n1_r16_s1_export_error_code_and_health_file`，内容未变): `cargo test -q -p alter-core --test export_ts n1_r16_s1` → FAIL: panicked at export_ts.rs:48（读取 HealthFile.ts 失败）(R16.S1)
- GREEN: 恢复后 `cargo test -q -p alter-core --test export_ts` → PASS (3 tests) (R16.S1)
- VERIFY: `n1_export_space_colors`（任务子项，非场景）先 FAIL（读取 spaceColors.ts 失败，panicked at export_ts.rs:48），实现后 PASS；`bun run protocol:export` 重新生成 `ShellCommand.ts`、`ShellEvent.ts`、`ShellState.ts`、`TabView.ts` 并新增 `spaceColors.ts`，`packages/protocol/src/index.ts` 再导出 `SPACE_COLORS`
- VERIFY (Widen): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、lifecycle 8、shell 21 全部 PASS；`bun test` 1 pass；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过

### T5 alterd：新建 / 编辑 / 切换 Space，当前 Space 持久化
- Revision: 1
- RED: `cargo test -q -p alterd --test shell n1_r1_` → FAIL: `n1_r1_s1_created_space_becomes_active` 收到 {"code":"E_INTERNAL","message":"尚未实现：shell.event.spaceCreated"}，期望 shell.stateChanged (R1.S1)
- RED: `cargo test -q -p alterd --test shell n1_r1_` → FAIL: `n1_r1_s2_blank_name_is_rejected` left: "E_INTERNAL" right: "E_INVALID_ARGUMENT" (R1.S2)
- RED: `cargo test -q -p alterd --test shell n1_r1_` → FAIL: `n1_r1_s3_name_longer_than_32_is_rejected` left: "E_INTERNAL" right: "E_INVALID_ARGUMENT" (R1.S3)
- RED: `cargo test -q -p alterd --test shell n1_r1_` → FAIL: `n1_r1_s4_unknown_color_is_rejected` left: "E_INTERNAL" right: "E_INVALID_ARGUMENT" (R1.S4)
- GREEN: `cargo test -q -p alterd --test shell n1_r1_` → PASS (4 tests)，`Store::create_space` / `active_space_id`，TabService 校验名称（trim 后 1–32 个字符）与颜色（`SPACE_COLORS`），快照的当前 Space 改用 `active_space_id()` (R1.S1)
- GREEN: `cargo test -q -p alterd --test shell n1_r1_` → PASS (4 tests) (R1.S2)
- GREEN: `cargo test -q -p alterd --test shell n1_r1_` → PASS (4 tests)；测试同时断言 32 个汉字（96 字节）被接受 (R1.S3)
- GREEN: `cargo test -q -p alterd --test shell n1_r1_` → PASS (4 tests) (R1.S4)
- RED: `cargo test -q -p alterd --test shell n1_r` → FAIL: `n1_r3_s1_activate_changes_active_space` 收到 E_INTERNAL「尚未实现：shell.event.spaceActivated」(R3.S1)
- RED: `cargo test -q -p alterd --test shell n1_r` → FAIL: `n1_r3_s2_activate_unknown_space_is_rejected` 收到 E_INTERNAL，期望 E_SPACE_NOT_FOUND (R3.S2)
- GREEN: `cargo test -q -p alterd --test shell n1_r` → `n1_r3_*` PASS（6 passed; 2 failed，剩下的 2 个失败是 R2），`Store::activate_space` (R3.S1)
- GREEN: `cargo test -q -p alterd --test shell n1_r` → `n1_r3_s2_*` PASS (R3.S2)
- RED: `cargo test -q -p alterd --test shell n1_r` → FAIL: `n1_r2_s1_rename_keeps_id_and_active_space` 收到 E_INTERNAL「尚未实现：shell.event.spaceUpdated」（切换已实现后，失败点落在改名这一步）(R2.S1)
- RED: `cargo test -q -p alterd --test shell n1_r` → FAIL: `n1_r2_s2_update_unknown_space_is_rejected` 收到 E_INTERNAL，期望 E_SPACE_NOT_FOUND (R2.S2)
- GREEN: `cargo test -q -p alterd --test shell n1_r` → PASS (8 tests)，`Store::update_space`（只改给出的字段，值没变返回 false 不广播）(R2.S1)
- GREEN: `cargo test -q -p alterd --test shell n1_r` → PASS (8 tests) (R2.S2)
- RED (mutation: `active_space_id` 改为 `ORDER BY position LIMIT 1`): `cargo test -q -p alterd --test shell n1_r4` → FAIL: `n1_r4_s1_restart_keeps_active_space` activeSpaceId 为 Personal 的 id (R4.S1)
- GREEN: 恢复后 `cargo test -q -p alterd --test shell n1_r4` → PASS (2 tests) (R4.S1)
- RED (mutation: 全为 NULL 时的兜底改为 `position DESC`): `cargo test -q -p alterd --test shell n1_r4` → FAIL: `n1_r4_s2_legacy_db_activates_first_space` activeSpaceId 为 "s-work" (R4.S2)
- GREEN: 恢复后 `cargo test -q -p alterd --test shell n1_r4` → PASS (2 tests) (R4.S2)
- VERIFY (Widen): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、lifecycle 8、shell 31 全部 PASS；`bun test` 1 pass；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过

### T6 alterd：删除 Space 与启动清理 partition 目录
- Revision: 1
- RED: `cargo test -q -p alterd --test shell n1_r5_` → FAIL: `n1_r5_s1_delete_active_middle_space_activates_next` 收到 E_INTERNAL「尚未实现：shell.event.spaceDeleted」，期望 shell.stateChanged (R5.S1)
- RED: `cargo test -q -p alterd --test shell n1_r5_` → FAIL: `n1_r5_s2_delete_active_last_space_activates_previous` 同上 (R5.S2)
- RED: `cargo test -q -p alterd --test shell n1_r5_` → FAIL: `n1_r5_s3_delete_inactive_space_keeps_active` 同上 (R5.S3)
- RED: `cargo test -q -p alterd --test shell n1_r5_` → FAIL: `n1_r5_s4_last_space_cannot_be_deleted` 收到 E_INTERNAL，期望 E_SPACE_NOT_FOUND (R5.S4)
- GREEN: `cargo test -q -p alterd --test shell n1_r5_` → PASS (4 tests)，`Store::delete_space` 在一个事务里拒绝最后一个、为当前 Space 选后一个（没有就前一个）相邻 Space、`DELETE` 级联删标签 (R5.S1)
- GREEN: `cargo test -q -p alterd --test shell n1_r5_` → PASS (4 tests) (R5.S2)
- GREEN: `cargo test -q -p alterd --test shell n1_r5_` → PASS (4 tests) (R5.S3)
- GREEN: `cargo test -q -p alterd --test shell n1_r5_` → PASS (4 tests) (R5.S4)
- RED: `cargo test -q -p alterd --test shell n1_r6_` → FAIL: `n1_r6_s1_startup_sweeps_orphan_partitions` panicked: 孤儿目录应当已被删除 (R6.S1)
- GREEN: `cargo test -q -p alterd --test shell n1_r6_` → PASS (1 test)，新文件 `crates/alterd/src/partitions.rs::sweep`，`lifecycle::run` 在 `Store::open` 之后、写 `alterd.json` 之前调用；`Store::list_space_ids` 含已归档的 Space (R6.S1)
- RED (mutation: 比较前不把 Space id 转小写): `cargo test -q -p alterd --test shell n1_r6_` → FAIL: 现存 Space 的目录应当保留 (R6.S1)
- GREEN: 恢复 `to_lowercase()` 后 `cargo test -q -p alterd --test shell n1_r6_` → PASS (1 test) (R6.S1)
- VERIFY: `cargo test -p alterd --test shell -- n1_r5_ n1_r6_` → PASS (5 tests)
- VERIFY (Widen): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、lifecycle 8、shell 36 全部 PASS；`bun test` 1 pass；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过

### T7 alterd：固定、关闭固定标签触发 discard、移动标签
- Revision: 1
- RED: `cargo test -q -p alterd --test shell n1_r8_` → FAIL: `n1_r8_s1_pin_moves_tab_to_pinned_section` 收到 E_INTERNAL「尚未实现：shell.event.tabPinned」，期望 shell.stateChanged (R8.S1)
- RED: `cargo test -q -p alterd --test shell n1_r8_` → FAIL: `n1_r8_s2_unpin_moves_tab_to_head_of_normal_section` 同上 (R8.S2)
- RED: `cargo test -q -p alterd --test shell n1_r8_` → FAIL: `n1_r8_s3_pin_unknown_tab_is_rejected` left: "E_INTERNAL" right: "E_TAB_NOT_FOUND" (R8.S3)
- GREEN: `cargo test -q -p alterd --test shell n1_r8_` → PASS (3 tests)，`Store::set_pinned` + 私有的 `sections_without` / `set_pin_flag` / `renumber`；`insert_tab` 的 position 改为未归档标签的最大值 + 1 (R8.S1)
- GREEN: `cargo test -q -p alterd --test shell n1_r8_` → PASS (3 tests) (R8.S2)
- GREEN: `cargo test -q -p alterd --test shell n1_r8_` → PASS (3 tests) (R8.S3)
- RED: `cargo test -q -p alterd --test shell n1_r11_` → FAIL: `n1_r11_s1_move_within_normal_section` 收到 E_INTERNAL「尚未实现：shell.event.tabMoved」(R11.S1)
- RED: `cargo test -q -p alterd --test shell n1_r11_` → FAIL: `n1_r11_s2_move_into_pinned_section_pins` 同上 (R11.S2)
- RED: `cargo test -q -p alterd --test shell n1_r11_` → FAIL: `n1_r11_s3_index_out_of_range_moves_to_end` 同上 (R11.S3)
- RED: `cargo test -q -p alterd --test shell n1_r11_` → FAIL: `n1_r11_s4_move_unknown_tab_is_rejected` 收到 E_INTERNAL，期望 E_TAB_NOT_FOUND (R11.S4)
- GREEN: `cargo test -q -p alterd --test shell n1_r` → PASS (23 tests)，`Store::move_tab`（插到目标分区 `min(index, len)`，跨分区时改固定标记与 `pinned_url`）(R11.S1)
- GREEN: `cargo test -q -p alterd --test shell n1_r` → PASS (23 tests) (R11.S2)
- GREEN: `cargo test -q -p alterd --test shell n1_r` → PASS (23 tests) (R11.S3)
- GREEN: `cargo test -q -p alterd --test shell n1_r` → PASS (23 tests) (R11.S4)
- RED: `cargo test -q -p alterd --test shell n1_r10_` → FAIL: `n1_r10_s1_closing_pinned_tab_discards_it` 1 秒内没有收到消息（只有快照，没有 shell.discard：固定标签被当作普通标签归档）(R10.S1)
- GREEN: `cargo test -q -p alterd --test shell n1_r` → PASS (24 tests)，`close_tab` 返回 `TabClosing::{Archived, Discarded, NotFound}`，固定标签不归档、`url = pinned_url`，TabService 对 `Discarded` 只向发送方回 `shell.discard` (R10.S1)
- RED (mutation: `close_tab` 的相邻规则改为先选上方 `position < ?2 DESC`；原 `r10_s1_*` 改名为 `n1_r9_s1_close_active_middle_tab_activates_next`，内容未变): `cargo test -q -p alterd --test shell n1_r9_` → FAIL: left: [("https://a.example", true), ("https://c.example", false)] right: [(…a, false), (…c, true)] (R9.S1)
- GREEN: 恢复后 `cargo test -q -p alterd --test shell n1_r9_` → PASS (1 test) (R9.S1)
- VERIFY: `cargo test -p alterd --test shell -- n1_r8_ n1_r9_ n1_r10_ n1_r11_` → PASS (9 tests)；`crates/alterd/src` 里已没有「尚未实现」的占位分支
- VERIFY (Widen): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、lifecycle 8、shell 44 全部 PASS；`bun test` 1 pass；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过

### T8 Electron：SpaceBar、编辑面板、切换、删除确认与 clearStorageData，以及 Cookie 隔离
- Revision: 1
- RED: `bun run e2e --grep "N1 R(1|2|3|5|7)\."` → FAIL: `N1 R1.S5` TimeoutError: locator.click: Timeout 5000ms exceeded. waiting for getByTestId('space-add') (R1.S5)
- RED: `bun run e2e --grep "N1 R(1|2|3|5|7)\."` → FAIL: `N1 R2.S3` TimeoutError: locator.click … waiting for getByTestId('space-dot').first() (R2.S3)
- RED: `bun run e2e --grep "N1 R(1|2|3|5|7)\."` → FAIL: `N1 R3.S3` TimeoutError … waiting for getByTestId('space-add') (R3.S3)
- RED: `bun run e2e --grep "N1 R(1|2|3|5|7)\."` → FAIL: `N1 R5.S5` TimeoutError … waiting for getByTestId('space-add') (R5.S5)
- RED: `bun run e2e --grep "N1 R(1|2|3|5|7)\."` → FAIL: `N1 R5.S6` TimeoutError … waiting for getByTestId('space-add') (R5.S6)
- RED: `bun run e2e --grep "N1 R(1|2|3|5|7)\."` → FAIL: `N1 R7.S1` TimeoutError … waiting for getByTestId('space-add') (R7.S1)
- GREEN: `bun run e2e --grep "N1 R(1|2|3|5|7)\."` → PASS (6 tests)，新增 `SpaceBar.tsx` / `SpacePanel.tsx` / `ConfirmDialog.tsx`，`shared/shell.ts` 的 `SPACE_CHANNELS`，preload 与 Main 的 `space.*` IPC 转成 Shell 事件 (R1.S5)
- GREEN: `bun run e2e --grep "N1 R(1|2|3|5|7)\."` → PASS (6 tests) (R2.S3)
- GREEN: `bun run e2e --grep "N1 R(1|2|3|5|7)\."` → PASS (6 tests) (R3.S3)
- GREEN: `bun run e2e --grep "N1 R(1|2|3|5|7)\."` → PASS (6 tests)，`TabViews.apply` 先关掉随标签消失的视图，再对上一份快照有、新快照没有的 Space 调 `clearStorageData` (R5.S5)
- GREEN: `bun run e2e --grep "N1 R(1|2|3|5|7)\."` → PASS (6 tests) (R5.S6)
- GREEN: `bun run e2e --grep "N1 R(1|2|3|5|7)\."` → PASS (6 tests) (R7.S1)
- RED (mutation: `clearStorageData()` 换成什么都不做): `bun run e2e --grep "N1 R5\.S5"` → FAIL: expect(received).toBe(expected) Expected: true Received: false（Work 的 partition 仍有 Cookie）(R5.S5)
- RED (mutation: 所有标签视图都用 `persist:space-shared`): `bun run e2e --grep "N1 R7\.S1"` → FAIL: Expected: "cookie: " Received: "cookie: k=v" (R7.S1)
- GREEN: 恢复后 `bun run e2e --grep "N1 R(5\.S5|7\.S1)"` → PASS (2 tests) (R5.S5, R7.S1)
- VERIFY (Widen): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、lifecycle 8、shell 44 全部 PASS；`bun test` 1 pass；`bun run e2e` → 29 passed（含已存档需求的 23 条）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS（Darwin 25.6.0），主会话

### T9 Electron：固定分区、图钉按钮、discard、拖拽
- Revision: 1
- RED: `bun run e2e --grep "N1 R(8|9|10|11)\."` → FAIL: `N1 R8.S4` TimeoutError: locator.click: Timeout 5000ms exceeded. waiting for getByTestId('tab-row').filter({ hasText: 'Page B' }).getByTestId('tab-pin') (R8.S4)
- RED: `bun run e2e --grep "N1 R(8|9|10|11)\."` → FAIL: `N1 R10.S2` TimeoutError … waiting for getByTestId('tab-row').filter({ hasText: 'Page A' }).getByTestId('tab-pin') (R10.S2)
- RED: `bun run e2e --grep "N1 R(8|9|10|11)\."` → FAIL: `N1 R11.S5` expect(locator).toHaveText(expected) failed，拖动后顺序没变 (R11.S5)
- GREEN: `bun run e2e --grep "N1 R(8|9|10|11)\."` → PASS (4 tests)，IPC `tab.pin` / `tab.move`，`Sidebar.tsx` 分固定 / 普通两段并加 `pinned-divider`，行上加图钉按钮 `tab-pin` 与 `data-pinned` (R8.S4)
- GREEN: `bun run e2e --grep "N1 R(8|9|10|11)\."` → PASS (4 tests)，Main 收到 `shell.discard` 调 `TabViews.discard`（从窗口卸下并关闭该视图）(R10.S2)
- GREEN: `bun run e2e --grep "N1 R(8|9|10|11)\."` → PASS (4 tests)，HTML5 DnD：落在行的上半部分插到它前面、下半部分插到后面，落在分隔线上即固定到分区末尾；`locator.dragTo` 可用，没有用到 design.md 的备用方案 (R11.S5)
- RED (mutation: Main 的 `tab.close` IPC 不再转发给 alterd；`R10.S2 关闭最后一个标签` 改名为 `N1 R9.S2 …`，内容未变): `npx playwright test -c e2e/playwright.config.ts --grep "N1 R9\.S2"` → FAIL: Expected: 0 Received: 1 (R9.S2)
- GREEN: 恢复后 `bun run e2e --grep "N1 R(8|9|10|11)\."` → PASS (4 tests) (R9.S2)
- RED (mutation: 收到 `shell.discard` 什么也不做): `npx playwright test … --grep "N1 R10\.S2"` → FAIL: 固定标签的页面应当已卸载 Expected: true Received: false (R10.S2)
- RED (mutation: 落点前后判断取反): `npx playwright test … --grep "N1 R11\.S5"` → FAIL: toHaveText 期望 Page C、Page A、Page B 不符 (R11.S5)
- GREEN: 恢复后 `bun run e2e --grep "N1 R(8|9|10|11)\."` → PASS (4 tests) (R10.S2, R11.S5)
- REFACTOR: `App.tsx` 的 Shell 状态改用 `createStore` + `reconcile(next, { key: "id" })`，见决策与偏差 → `npx playwright test … --grep "N1 R11\.S5" --repeat-each 30` → 30 passed；临时诊断用例（未提交）：激活另一个标签时 `.tabs` 的子节点被移除数由 6 变为 0
- VERIFY (Widen): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、lifecycle 8、shell 44 全部 PASS；`bun test` 1 pass；`bun run e2e` → 32 passed；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T10 Electron：应用菜单与快捷键
- Revision: 1
- RED: `bun run e2e --grep "N1 R12\."` → FAIL: `N1 R12.S1` expect(received).toEqual(expected) - Expected - 18 + Received + 18（菜单项全部为 null）(R12.S1)
- RED: `bun run e2e --grep "N1 R12\."` → FAIL: `N1 R12.S2` electronApplication.evaluate: Error: 应用菜单里没有 tab.close (R12.S2)
- RED: `bun run e2e --grep "N1 R12\."` → FAIL: `N1 R12.S3` Error: 应用菜单里没有 space.1 (R12.S3)
- RED: `bun run e2e --grep "N1 R12\."` → FAIL: `N1 R12.S4` Error: 应用菜单里没有 tab.next (R12.S4)
- RED: `bun run e2e --grep "N1 R12\."` → FAIL: `N1 R12.S5` Error: 应用菜单里没有 tab.new (R12.S5)
- RED: `bun run e2e --grep "N1 R12\."` → FAIL: `N1 R12.S6` Error: 应用菜单里没有 tab.togglePin (R12.S6)
- RED: `bun run e2e --grep "N1 R12\."` → FAIL: `N1 R12.S7` Error: 应用菜单里没有 nav.reload (R12.S7)
- GREEN: `bun run e2e --grep "N1 R12\."` → PASS (7 tests)，新文件 `src/main/menu.ts`（`buildMenu(actions)`，保留 `appMenu` / `editMenu` / `windowMenu` 角色）与 `src/main/actions.ts`（`createActions(ctx)`），`index.ts` 的 IPC 处理全部改为调用 actions (R12.S1)
- GREEN: `bun run e2e --grep "N1 R12\."` → PASS (7 tests) (R12.S2)
- GREEN: `bun run e2e --grep "N1 R12\."` → PASS (7 tests) (R12.S3)
- GREEN: `bun run e2e --grep "N1 R12\."` → PASS (7 tests) (R12.S4)
- GREEN: `bun run e2e --grep "N1 R12\."` → PASS (7 tests)，Main → Renderer 的 `address.focus {newTab}`，先 `win.webContents.focus()` 把焦点从页面视图交还侧边栏 (R12.S5)
- GREEN: `bun run e2e --grep "N1 R12\."` → PASS (7 tests) (R12.S6)
- GREEN: `bun run e2e --grep "N1 R12\."` → PASS (7 tests) (R12.S7)
- RED (mutation: Renderer 收到 `address.focus {newTab:false}` 不聚焦): `npx playwright test -c e2e/playwright.config.ts --grep "N1 R12\.S5"` → FAIL: Expected: true Received: false (R12.S5)
- RED (mutation: `cycleTab` 到末尾不回到开头): `npx playwright test … --grep "N1 R12\.S4"` → FAIL: Expected pattern: /active/ Received string: "" (R12.S4)
- RED (mutation: `space.N` 不存在时取最后一个 Space): `npx playwright test … --grep "N1 R12\.S3"` → FAIL: Expected: "Personal" Received: "Work" (R12.S3)
- GREEN: 恢复后 `bun run e2e --grep "N1 R12\."` → PASS (7 tests) (R12.S3, R12.S4, R12.S5)
- VERIFY (Widen): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、lifecycle 8、shell 44 全部 PASS；`bun test` 1 pass；`bun run e2e` → 39 passed；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T11 性能：500 个标签时切换 Space 的界面耗时（父级 SC4）
- Revision: 1
- GREEN: `bun run e2e --grep SC4` → PASS (1 test)：SC4: 20 次切换 p50 13.6 ms，p95 16.2 ms，最大 22.7 ms（预算 p95 < 50 ms）
- VERIFY (mutation: `App.tsx` 应用每份快照前忙等 60 ms): `npx playwright test -c e2e/playwright.config.ts --grep SC4` → FAIL: Expected: < 50 Received: 71.19999992847443（p50 70.2 ms），证明计时能发现变慢；恢复后再跑两次 p95 15.5 ms、15.6 ms
- VERIFY (Widen): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、lifecycle 8、shell 44 全部 PASS；`bun test` 1 pass；`bun run e2e` → 40 passed（SC4 p95 14.7 ms，SC2 105 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T12 拖到分隔线即固定、拖到普通分区空白处即取消固定（CR1、CR2）
- Revision: 1
- RED: `bun run e2e --grep "N1 CR(1|2) "` → FAIL: `N1 CR1` expect(locator).toHaveText(expected) failed（拖到分隔线后顺序不变，alterd 丢弃了 index 超出 u32 的事件）
- RED: `bun run e2e --grep "N1 CR(1|2) "` → FAIL: `N1 CR2` TimeoutError: locator.dragTo: Timeout 5000ms exceeded（没有 `normal-drop` 落点）
- GREEN: `bun run e2e --grep "N1 (CR(1|2) |R(8|9|10|11)\.)"` → PASS (6 tests)，分隔线落点改发固定分区（不含被拖标签）的长度，普通分区末尾加 `normal-drop`
- - VERIFY (Widen，T12–T16 合并后一次): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、lifecycle 8、shell 45 全部 PASS；`bun test` 1 pass；`bun run e2e` → 44 passed（SC4 p95 15.8 ms，SC2 108 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T13 关闭普通标签后同 Space 的未归档标签重新连续编号（CR3）
- Revision: 1
- RED: `cargo test -q -p alterd --test shell n1_cr3_` → FAIL: assertion `left == right` failed left: (0, 2, 3) right: (0, 1, 2)
- GREEN: `cargo test -q -p alterd --test shell n1_` → PASS (25 tests)，`close_tab` 归档分支在选完相邻标签后调 `renumber`
- - VERIFY (Widen，T12–T16 合并后一次): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、lifecycle 8、shell 45 全部 PASS；`bun test` 1 pass；`bun run e2e` → 44 passed（SC4 p95 15.8 ms，SC2 108 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T14 已卸载的固定标签在用户再次激活前不重建页面（CR4）
- Revision: 1
- RED: `bun run e2e --grep "N1 CR4 " --repeat-each 3` → FAIL（3 次中 2 次）: expect(received).not.toContain(expected) Expected value: not "http://127.0.0.1:50345/a" Received array: ["http://127.0.0.1:50345/a", …]（discard 先到时随后的快照又把页面建了出来）
- GREEN: `bun run e2e --grep "N1 (CR4 |R10\.|R12\.S4)" --repeat-each 4` → 12 passed，`TabViews` 的 `discarded` 集合 + `wake(tabId)`，`actions` 的 `activateTab` / `cycleTab` 经 `ctx.activated` 唤醒
- - VERIFY (Widen，T12–T16 合并后一次): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、lifecycle 8、shell 45 全部 PASS；`bun test` 1 pass；`bun run e2e` → 44 passed（SC4 p95 15.8 ms，SC2 108 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T15 删除 Space 时同时清空 HTTP 缓存（CR5）
- Revision: 1
- RED: `bun run e2e --grep "N1 CR5 "` → FAIL: 删除后缓存应为 0，实际 4608
- GREEN: `bun run e2e --grep "N1 (CR5 |R5\.)"` → PASS (3 tests)，`TabViews.apply` 对被删的 Space 同时 `clearStorageData()` 与 `clearCache()`；测试服务器加可缓存的 `/cached`
- - VERIFY (Widen，T12–T16 合并后一次): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、lifecycle 8、shell 45 全部 PASS；`bun test` 1 pass；`bun run e2e` → 44 passed（SC4 p95 15.8 ms，SC2 108 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T16 SC4 基准断言目标 Space 的列表内容已渲染（CR6）
- Revision: 1
- GREEN: `bun run e2e --grep SC4` → PASS：两个 Space 灌 `/n/0…249` 与 `/n/1000…1249`，终点另要求第一行文字为目标 Space 的首个地址；SC4: 20 次切换 p50 13.5 ms，p95 15.5 ms
- VERIFY (mutation: Sidebar 总显示第一个 Space 的标签，即「名称切了、列表没切」): `npx playwright test -c e2e/playwright.config.ts --grep SC4` → FAIL（计时等不到终点，30 s 用例超时）；恢复后通过
- - VERIFY (Widen，T12–T16 合并后一次): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、lifecycle 8、shell 45 全部 PASS；`bun test` 1 pass；`bun run e2e` → 44 passed（SC4 p95 15.8 ms，SC2 108 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T17 `wake` 只清除已卸载标记，不再应用缓存的旧快照（CR7）
- Revision: 1
- GREEN: `bun run e2e --grep "N1 (CR4 |R10\.|R12\.S4)" --repeat-each 5` → 15 passed，`TabViews.wake` 只删 `discarded` 里的标记，删掉 `last` 字段；视图由激活后 alterd 必发的快照重建
- VERIFY (mutation: alterd 关闭固定标签时不把 url 恢复成 `pinned_url`，即「按导航后的地址重建」这一后果): `bun run e2e --grep "N1 R10\.S2"` → FAIL: Expected: true Received: false；恢复后 PASS。竞态本身（discard 已到、快照未到时立即点击）在 e2e 里无法稳定构造，没有直接的 RED；修复方式是去掉产生旧快照重建的代码路径
- - VERIFY (Widen，T17–T18 合并后一次): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、lifecycle 8、shell 45 全部 PASS；`bun test` 1 pass；`bun run e2e` → 44 passed（SC4 p95 15.5 ms，SC2 117 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T18 SC4 计时终点逐行核对 250 行都属于目标 Space（CR8）
- Revision: 1
- GREEN: `bun run e2e --grep SC4` → PASS：SC4: 20 次切换 p50 14.0 ms，p95 15.4 ms；每行文字须为目标 Space 第 i 个标签的 URL 或标题 `Page <n>`
- VERIFY (mutation: Sidebar 第一行取目标 Space、其余 249 行取第一个 Space): `npx playwright test -c e2e/playwright.config.ts --grep SC4` → FAIL（计时等不到终点，30 s 用例超时）；恢复后通过
- - VERIFY (Widen，T17–T18 合并后一次): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、lifecycle 8、shell 45 全部 PASS；`bun test` 1 pass；`bun run e2e` → 44 passed（SC4 p95 15.5 ms，SC2 117 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T19 同一事件的私有回复先于它引起的快照写出（CR9）
- Revision: 1
- RED: `cargo test -q -p alterd --test shell n1_cr9_`（连跑 3 次）→ 3 次都 FAIL: assertion `left == right` failed: 第 2 / 0 / 1 次关闭后先收到：{"activeSpaceId":…}（快照先于 shell.discard）
- GREEN: `cargo test -q -p alterd --test shell n1_cr9_`（连跑 3 次）→ 3 次 PASS，`crates/alterd/src/shell/mod.rs` 的 `tokio::select!` 加 `biased;` 并把私有回复分支放在第一个
- REFACTOR: `n1_r10_s1_closing_pinned_tab_discards_it` 改为按顺序断言（先 discard 后快照）；design.md 接口契约写明回复先于同一事件的快照 → `cargo test -q -p alterd --test shell n1_r10_` PASS
- VERIFY: `cargo test -p alterd --release --test shell_perf -- --ignored --nocapture` → SC1: 100 次 openRequested → 快照（500–600 个标签）p50 1.03 ms，p95 1.13 ms，PASS
- VERIFY (Widen): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、lifecycle 8、shell 46 全部 PASS；`bun test` 1 pass；`bun run e2e` → 44 passed（SC4 p95 15.1 ms，SC2 114 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T20 关闭固定标签后、`shell.discard` 到达前再次激活它，页面按固定地址正常打开（CR10）
- Revision: 1
- RED: `bun run e2e --grep "N1 CR10 " --repeat-each 3` → 3 次都 FAIL: Error: 内容区应当显示 /a，实际 []（迟到的 discard 打上「已卸载」标记，之后的快照不再建视图）
- GREEN: `bun run e2e --grep "N1 (CR10 |CR4 |R10\.|R12\.S2)" --repeat-each 3` → 12 passed，`TabViews.closing` 集合（`actions` 发 `tabClosed` 前经 `ctx.closing` 记入，`wake` 清掉），`discard()` 只在标签仍在集合里时打标记；design.md 接口契约同步
- VERIFY (Widen): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、lifecycle 8、shell 46 全部 PASS；`bun test` 1 pass；`bun run e2e` → 46 passed（含回归保护用例 `N1 CR11`；SC4 p95 15.7 ms，SC2 114 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T22 每个连接上消息的写出顺序与 alterd 处理事件的顺序一致（CR12）
- Revision: 1
- RED: `cargo test -q -p alterd --test shell n1_cr12_`（连跑 2 次）→ 2 次都 FAIL: 第 0 轮 left: ["shell.discard", "shell.stateChanged", "shell.stateChanged"] right: ["shell.stateChanged", "shell.discard", "shell.stateChanged"]（discard 越过了更早的 tabUpdated 快照）
- GREEN: `cargo test -q -p alterd --test shell n1_cr`（连跑 3 次）→ 每次 3 passed（CR3、CR9、CR12），`Snapshot { seq, line }` 与 `Reply { after, cmd }`，TabService 每次发布序号加 1，连接任务写回复前补写序号 ≤ `after` 的快照、按序号跳过重复快照；design.md 接口契约同步
- VERIFY: `cargo test -p alterd --release --test shell_perf -- --ignored --nocapture` → SC1: p50 0.96 ms，p95 1.08 ms，PASS
- VERIFY (Widen): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、lifecycle 8、shell 47 全部 PASS；`bun test` 1 pass；`bun run e2e` → 46 passed（SC4 p95 15.5 ms，SC2 119 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T23 连接落后、广播丢帧时回复仍不越过更新的快照（CR13）
- Revision: 1
- REFACTOR: 先把连接任务里「补写快照再写回复」提成 `shell::write_reply`（行为不变，`write_line` 改为对任意 `AsyncWrite`）→ `cargo test -q -p alterd --test shell` → PASS (47 tests)
- RED: `cargo test -q -p alterd --bin alterd n1_cr13` → FAIL: assertion `left == right` failed left: ["snap4", "{\"type\":\"shell.discard\",\"tabId\":\"t1\"}"] right: [discard, "snap4"]（容量 2 的广播发布 1–5，after=2 的回复落在快照 4 之后）
- GREEN: `cargo test -q -p alterd --bin alterd n1_cr13` → PASS (1 test)，补写循环收到序号 > `after` 的快照时先写回复、再写它；`cargo test -q -p alterd --test shell` → 47 passed
- - VERIFY (Widen，T23–T24 合并后一次): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、alterd 单元 1、lifecycle 8、shell 47 全部 PASS；`cargo test -p alterd --release --test shell_perf -- --ignored --nocapture` → SC1 p95 1.13 ms；`bun test` 1 pass；`bun run e2e` → 46 passed（SC4 p95 15.0 ms，SC2 95 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T24 关闭中或已卸载的固定标签，旧视图的页面事件不再上报（CR14）
- Revision: 1
- GREEN: `bun run e2e` → 46 passed（含 `N1 CR4`、`N1 CR10`、`N1 R10.S2`），`TabViews.create` 的 `update` 回调在标签处于 `closing` 或 `discarded` 时直接返回；design.md 接口契约同步
- VERIFY: 竞态窗口（关闭后、discard 到达前旧视图恰好完成一次导航）在 e2e 里无法稳定构造，没有直接的 RED；修复是去掉上报这条路径。顺带效果：被关闭的普通标签的旧视图迟到事件也不再上报，不会再引出 `E_TAB_NOT_FOUND` 错误提示
- - VERIFY (Widen，T23–T24 合并后一次): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、alterd 单元 1、lifecycle 8、shell 47 全部 PASS；`cargo test -p alterd --release --test shell_perf -- --ignored --nocapture` → SC1 p95 1.13 ms；`bun test` 1 pass；`bun run e2e` → 46 passed（SC4 p95 15.0 ms，SC2 95 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T25 每个连接改为一条先进先出队列，快照与回复按 TabService 的处理顺序写出（CR15）
- Revision: 1
- 作废的 RED：最初在旧实现上跑 `n1_cr15_messages_follow_processing_order_under_backlog` 得到「1 秒内没有收到消息」，排查后是测试辅助 `Daemon::spawn` 把 stderr 接成管道却不读，400 行 WARN 写满 64 KB 管道把 alterd 卡住（独立的 Python 客户端对同一个 alterd 收齐了 800 条）。加 `Daemon::start_quiet`（stderr 丢弃）后该用例在旧实现上也通过，分辨不出新旧实现，只留作回归保护。
- RED: 临时把 `crates/alterd/src` 换回 HEAD（序号 + 广播）跑 `cargo test -q -p alterd --test shell n1_cr15_slow`（2 次）→ 都 FAIL: assertion `left == right` failed: B 的第 10 份快照 left: 45 right: 10（慢连接落后后被跳过了快照）；随后换回新实现
- GREEN: `cargo test -q -p alterd --test shell n1_cr15_slow`（2 次）→ PASS；`tabs::Request::{Connect(Outbox), Event { event, outbox }}`，TabService 持有各连接发件箱并按处理顺序放入快照与回复，`shell::connection` 只按序写出；删掉 `Snapshot.seq`、`Reply.after`、`write_reply`、广播与 watch 通道（T23 的单元测试 `n1_cr13_*` 随 `write_reply` 一起删除）
- VERIFY: `cargo test -q -p alterd --test shell n1_cr`（3 次）→ CR3、CR9、CR12、两条 CR15 全部 PASS；design.md 接口契约改为发件箱模型，风险里写明发件箱不限长
- - VERIFY (Widen，T25–T26 合并后一次): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、lifecycle 8、shell 49 全部 PASS；`cargo test -p alterd --release --test shell_perf -- --ignored --nocapture` → SC1 p95 1.09 ms；`bun test` 1 pass；`bun run e2e` → 46 passed（SC4 p95 16.1 ms，SC2 114 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T26 关闭固定标签时 Main 立即关掉它的视图；页面事件只在视图仍是该标签当前登记的视图时上报（CR16）
- Revision: 1
- GREEN: `bun run e2e --grep "N1 (CR4 |CR10 |R10\.|R12\.S2)" --repeat-each 3` → 12 passed，`TabViews.closingTab(tabId, pinned)` 对固定标签立即 `closeView`，`update` 回调比对 `this.views.get(tab.id) === view`
- VERIFY: 竞态（关闭后、discard 前再次激活，同时旧视图完成导航）在 e2e 里无法稳定构造，没有直接的 RED；修复后旧视图在关闭时就已关掉，不存在迟到事件的来源
- - VERIFY (Widen，T25–T26 合并后一次): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、lifecycle 8、shell 49 全部 PASS；`cargo test -p alterd --release --test shell_perf -- --ignored --nocapture` → SC1 p95 1.09 ms；`bun test` 1 pass；`bun run e2e` → 46 passed（SC4 p95 16.1 ms，SC2 114 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T27 alterd 记录已卸载的固定标签：快照 `TabView.discarded`，再次激活时清除，已卸载期间忽略它的 `tabUpdated`（CR17、CR18）
- Revision: 1
- RED: `cargo test -q -p alterd --test shell n1_cr17_` → FAIL: assertion `left == right` failed left: Null right: true（快照里还没有 discarded）
- GREEN: `cargo test -q -p alterd --test shell` → PASS (50 tests)，`TabView.discarded`；TabService 的 `discarded: HashSet<String>`（关闭固定标签加入、`tabActivated` 移除、发布时清掉已不存在的），已卸载时 `tabUpdated` 返回 `Ok(false)`；`bun run protocol:export` 更新 `TabView.ts`；`cargo test -q -p alter-core --test export_ts` → 3 passed
- REFACTOR: `n1_cr12_replies_keep_order_with_earlier_snapshots` 每轮先激活 P：上一轮关闭后 P 已卸载，新契约下它的回写被忽略，旧写法与契约矛盾（见决策与偏差）→ PASS
- - VERIFY (Widen，T27–T29 合并后一次): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、lifecycle 8、shell 50 全部 PASS（shell 的结果行另用 `cargo test --no-fail-fast -p alterd` 确认）；`cargo test -p alterd --release --test shell_perf -- --ignored --nocapture` → SC1 p95 1.15 ms；`bun test` 1 pass；`bun run e2e` → 46 passed（SC4 p95 15.0 ms，SC2 114 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T28 Main 的视图完全跟随快照；关闭时当前视图作废（CR17、CR18）
- Revision: 1
- GREEN: `bun run e2e --grep "N1 (CR4 |CR10 |R10\.|R12\.)" --repeat-each 3` → 30 passed，`TabViews.apply` 不为 `discarded` 的标签建视图并关掉已有的，`voidView(tabId)` 把关闭时的视图放进 `WeakSet`，页面事件只在视图未作废且是当前登记视图时上报；删除 `closing` / `discarded` / `wake` / `closingTab` 与 actions 的 `activated`
- VERIFY: 旧快照迟到、连发「固定、关闭」这两种交错在 e2e 里无法稳定构造，没有直接的 RED。按构造分析：卸载状态只来自 alterd 的快照，且每个连接的消息按处理顺序到达；作废视图的事件不再上报，即使上报，alterd 在该标签已卸载期间也会忽略
- - VERIFY (Widen，T27–T29 合并后一次): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、lifecycle 8、shell 50 全部 PASS（shell 的结果行另用 `cargo test --no-fail-fast -p alterd` 确认）；`cargo test -p alterd --release --test shell_perf -- --ignored --nocapture` → SC1 p95 1.15 ms；`bun test` 1 pass；`bun run e2e` → 46 passed（SC4 p95 15.0 ms，SC2 114 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T29 发件箱清理与 TabService 退出时结束连接（CR19、CR20）
- Revision: 1
- GREEN: `cargo test -q -p alterd --test shell` → PASS (50 tests)，`Request::Connect` 时 `outboxes.retain(|o| !o.is_closed())`，连接任务 `select!` 加 `requests.closed()` 分支
- - VERIFY (Widen，T27–T29 合并后一次): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、lifecycle 8、shell 50 全部 PASS（shell 的结果行另用 `cargo test --no-fail-fast -p alterd` 确认）；`cargo test -p alterd --release --test shell_perf -- --ignored --nocapture` → SC1 p95 1.15 ms；`bun test` 1 pass；`bun run e2e` → 46 passed（SC4 p95 15.0 ms，SC2 114 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T30 关闭屏障：关闭时立即关掉视图，关闭结果到达前不为它建视图；弹窗只接受当前登记视图（CR21、CR22）
- Revision: 1
- GREEN: `bun run e2e --grep "N1 (CR4 |CR10 |R10\.|R12\.|R9\.)" --repeat-each 3` → 33 passed，`TabViews.closingTab` 关掉视图并加入 `closing`，`discard` 撤掉屏障，`apply` 清理已不存在的标签并跳过屏障中的标签，弹窗处理器比对视图实例；删除 T28 的 `voidView` / `voided`
- VERIFY: CR21 的交错（快速「激活、关闭、再激活」且页面恰好跳转回写）与 CR22 的弹窗时机在 e2e 里无法稳定构造，没有直接的 RED；按构造：关闭时视图已关掉，屏障期间到达的旧快照不能建视图，固定标签的屏障在 shell.discard 撤掉，之后的快照按 alterd 的 `discarded` 决定
- VERIFY (Widen): `cargo test --no-fail-fast -p alterd -p alter-core -p alter-cli` → cli 4、alter-core 1+3+7、lifecycle 8、shell 50 全部 PASS；`bun test` 1 pass；`bun run e2e` → 46 passed（SC4 p95 15.9 ms，SC2 111 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T31 关闭屏障按次数计（CR23）
- Revision: 1
- GREEN: `bun run e2e` → 46 passed（SC4 p95 15.1 ms，SC2 115 ms），`TabViews.closing` 改为 `Map<tabId, 次数>`，`closingTab` 加 1、`discard` 减 1 到 0 才删除，标签从快照消失时删除；design.md 同步
- VERIFY: 「关闭、激活、关闭、激活」在回复到达前连发的交错无法在 e2e 里稳定构造，没有直接的 RED；按构造，每次关闭对应 alterd 的一条 shell.discard，计数归零前屏障一直生效
- VERIFY (Widen): `cargo test --no-fail-fast -p alterd -p alter-core -p alter-cli` 全部 PASS；`bun test` 1 pass；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过；e2e 见上。运行环境：本机 macOS，主会话

### T32 在已卸载的标签里打开地址：改 url、清除卸载状态并发布快照（CR24）
- Revision: 1
- RED: `cargo test -q -p alterd --test shell n1_cr24_` → FAIL: 收到：{"tabId":…,"type":"shell.navigate","url":"https://example.com/z"} left: String("shell.navigate") right: "shell.stateChanged"
- RED: `bun run e2e --grep "N1 CR24 "` → FAIL: Error: 内容区应当显示 /b，实际 []
- GREEN: `cargo test -q -p alterd --test shell` → PASS (51 tests)；`bun run e2e --grep "N1 (CR24 |CR4 |CR10 |R10\.)"` → 4 passed。`OpenRequested` 带 `tabId` 且该标签已卸载时 `update_tab(url)`、从 `discarded` 移除、返回 `Ok(true)`；design.md 同步
- VERIFY (Widen): `cargo test --no-fail-fast -p alterd -p alter-core -p alter-cli` → cli 4、alter-core 1+3+7、lifecycle 8、shell 51 全部 PASS；`cargo test -p alterd --release --test shell_perf -- --ignored --nocapture` → SC1 p95 1.39 ms；`bun test` 1 pass；`bun run e2e` → 47 passed（SC4 p95 16.6 ms，SC2 109 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T33 已卸载的标签在快照里不算激活；写库成功后才清除卸载状态（CR26、CR27）
- Revision: 1
- RED: `cargo test -q -p alterd --test shell n1_cr27_` → FAIL: left: Bool(true) right: false（唯一的已卸载固定标签仍是激活标签）
- RED: `bun run e2e --grep "N1 CR(27|24) "` → FAIL: `N1 CR27` Expected: "没有打开的标签" element(s) not found；`N1 CR24`（改为「新开标签」的期望后）toHaveText 少一行
- GREEN: `cargo test -q -p alterd --test shell` → PASS (52 tests)；`bun run e2e --grep "N1 (CR4 |CR10 |CR24 |CR27 |R10\.|R12\.)"` → 12 passed。`snapshot()` 计算激活标签时跳过 `discarded`；`OpenRequested` 对已卸载标签先写库、成功后才 `discarded.remove`；design.md 同步
- VERIFY: CR26（写库失败）需要注入 SQLite 故障，没有自动化用例；修复是调整顺序，失败时 `?` 提前返回，卸载状态保持
- VERIFY (Widen): `cargo test --no-fail-fast -p alterd -p alter-core -p alter-cli` → cli 4、alter-core 1+3+7、lifecycle 8、shell 52 全部 PASS；`cargo test -p alterd --release --test shell_perf -- --ignored --nocapture` → SC1 p95 1.18 ms；`bun test` 1 pass；`bun run e2e` → 48 passed（SC4 p95 15.7 ms，SC2 112 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T34 没有激活标签时 `tab.next` 激活第一行、`tab.prev` 激活最后一行（CR28）
- Revision: 1
- RED: `bun run e2e --grep "N1 CR28 "` → FAIL: expect(locator).toHaveClass(expected) Expected pattern: /active/ Received string: ""（shortcuts.spec.ts:193，`tab.next` 激活了第二行）
- GREEN: `bun run e2e` → 49 passed（含 `N1 CR28`、`N1 R12.S4`；SC4 p95 15.1 ms，SC2 119 ms），`cycleTab` 在没有激活标签时向后从第一行、向前从最后一行开始
- VERIFY (Widen): `cargo test --no-fail-fast -p alterd -p alter-core -p alter-cli` 全部 PASS（shell 52）；`bun test` 1 pass；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T35 Main 作用于当前激活标签的动作不把关闭屏障中的标签当作激活标签（CR29、CR30）
- Revision: 1
- RED: `bun run e2e --grep "N1 CR29 " --repeat-each 2` → 2 次都 FAIL: expect(locator).toHaveCount(expected) Expected: 2 Received: 1（地址被打开在刚关闭的固定标签里，没有新开标签）
- 中间一次 `bun run e2e` 有 10 条失败：改动脚本中途断言失败，`actions.ts` 已调用 `ctx.isClosing` 而 `index.ts` 还没提供，属于改到一半的状态，不计入证据；补完后重跑
- GREEN: `bun run e2e` → 50 passed（含 `N1 CR29`、`N1 CR24`、`N1 CR27`、`N1 CR28`、`N1 R12.*`；SC4 p95 15.4 ms，SC2 114 ms），`ActionContext.isClosing` 查 `TabViews.isClosing`，`activeTab()` 遇到屏障中的标签返回 undefined，`cycleTab` 改用 `activeTab()`；design.md 同步
- VERIFY: CR30（关闭后、快照到达前按 `tab.next`）与 CR29 同一路径修复，没有单独的 RED
- VERIFY (Widen): `cargo test --no-fail-fast -p alterd -p alter-core -p alter-cli` 全部 PASS（shell 52）；`bun test` 1 pass；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T36 e2e 断言 THEN 的全部条件在场景时限内同时成立；R10.S2 断言保留的是被关闭的那一行
- Revision: 1
- GREEN: `bun run e2e --grep "N1 R(1\.S5|3\.S3|5\.S5|8\.S4|9\.S2|10\.S2|12\.S2|12\.S5|12\.S6) " --repeat-each 3` → 27 passed。`e2e/ui.ts` 加 `expectWithin(t0, ms, what)`，这 9 条用例在操作前记 `t0`、每个条件用场景时限等待、最后检查总耗时；R10.S2 断言 `Page B` 那一行 `data-pinned` 为 true 且固定分区恰好是它；`restart` 返回重启前的 `t0`
- - VERIFY (Widen，T36–T38 合并后一次): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、lifecycle 8、shell 52 全部 PASS；`bun test` 1 pass；`bun run e2e` → 50 passed（SC4 p95 15.9 ms，SC2 114 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T37 启动清理按不区分大小写识别 `space-<id>` 目录
- Revision: 1
- RED: `cargo test -q -p alterd --test shell n1_r6_` → FAIL: 大写 id 的孤儿目录也应当已被删除 (R6.S1)
- GREEN: `cargo test -q -p alterd --test shell n1_r6_` → PASS (1 test)，`partition_space_id` 接受字母数字 26 位，比较前 `to_ascii_lowercase()` (R6.S1)
- - VERIFY (Widen，T36–T38 合并后一次): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、lifecycle 8、shell 52 全部 PASS；`bun test` 1 pass；`bun run e2e` → 50 passed（SC4 p95 15.9 ms，SC2 114 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T38 design.md 跟上代码
- Revision: 1
- VERIFY: `python3 -I ~/.claude/skills/dev-flow/scripts/dflow.py diagrams sync add-core-features` 与 `validate` → ok。「架构」一节与组件表改为「视图跟随 `TabView.discarded`、关闭屏障」，删除时序图里「面板显示错误」改为「地址框下显示错误（最后一个 Space 的删除按钮本就置灰）」
- - VERIFY (Widen，T36–T38 合并后一次): `cargo test --no-fail-fast` → cli 4、alter-core 1+3+7、lifecycle 8、shell 52 全部 PASS；`bun test` 1 pass；`bun run e2e` → 50 passed（SC4 p95 15.9 ms，SC2 114 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T39 场景 THEN 改为时限内轮询的组合条件，单调时钟，R8.S4 分隔线与 R10.S2 标签 id 纳入组合条件（CR32–CR35）
- Revision: 1
- GREEN: `bun run e2e --grep "N1 R(1\.S5|3\.S3|5\.S5|8\.S4|9\.S2|10\.S2|12\.S2|12\.S5|12\.S6) " --repeat-each 3` → 27 passed。`e2e/ui.ts` 加 `uiState(win)`（一次 `evaluate` 读出圆点、Space 名、各行文字 / 激活 / 固定 / 是否在分隔线上方、空提示）与 `expectAllWithin(t0, ms, what, check)`（`performance.now()` 计时，时限内轮询组合条件），删掉 `expectWithin`；R10.S2 关闭前从 Main 快照记下固定标签 id，关闭后确认同一 id 仍在且固定、排在首位
- VERIFY (mutation: Sidebar 把分隔线放到所有行之前): `npx playwright test -c e2e/playwright.config.ts --grep "N1 R8\.S4"` → FAIL: R8.S4：3000 ms 内条件没有同时成立；最后一次：[{"text":"Page B",…,"aboveDivider":false},…]；恢复后通过
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS（shell 52）；`bun test` 1 pass；`bun run e2e` → 50 passed（SC4 p95 15.4 ms，SC2 114 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T40 `uiState` 按屏幕位置与可见性判断分隔线；行带 `data-tab-id`，R10.S2 在同一次界面读取里核对 id（CR36、CR37）
- Revision: 1
- GREEN: `bun run e2e --grep "N1 R(1\.S5|3\.S3|5\.S5|8\.S4|9\.S2|10\.S2|12\.S2|12\.S5|12\.S6) " --repeat-each 3` → 27 passed。`Sidebar.tsx` 的标签行加 `data-tab-id`；`uiState` 的 `aboveDivider` 改为「分隔线可见（display / visibility / opacity / 宽度）且整行底边在分隔线上沿之上」，行带 `id`；R10.S2 点击关闭前从界面记下固定行的 id，关闭后同一次读取核对首行 id、固定、在分隔线上方；删掉不再用的 `shellTabs`
- VERIFY (mutation: CSS 让分隔线 `visibility: hidden`): `npx playwright test -c e2e/playwright.config.ts --grep "N1 R8\.S4"` → FAIL: R8.S4：3000 ms 内条件没有同时成立；最后一次：[{…,"text":"Page B",…,"aboveDivider":false},…]；恢复后通过
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS（shell 52）；`bun test` 1 pass；`bun run e2e` → 50 passed（SC4 p95 15.7 ms，SC2 118 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T41 分隔线可见性同时要求上边框有宽度且颜色不透明（CR38）
- Revision: 1
- GREEN: `uiState` 的分隔线可见判断加上 `borderTopWidth > 0` 且 `borderTopColor` 不是 transparent / alpha 为 0
- VERIFY (mutation: styles.css 把 `border-top: 1px solid #d4d4d8` 改为 `transparent`，sed 同时改到了另外两处同样写法的边框): `npx playwright test -c e2e/playwright.config.ts --grep "N1 R8\.S4"` → FAIL；恢复后通过
- 偶发失败一次：改完后第一次 `bun run e2e --grep "N1 R(8\.S4|10\.S2|12\.S6) "` 中 `N1 R8.S4` 在重启之前的「固定后 1 秒内顺序为 Page B、Page A」处失败（expect(locator).toHaveText failed）；随后 `--repeat-each 10`（10 passed）与重新构建后连跑 3 次（各 3 passed）都没有复现，全量 e2e 也通过。原因未找到，见决策与偏差
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS（shell 52）；`bun test` 1 pass；`bun run e2e` → 50 passed（SC4 p95 15.3 ms，SC2 117 ms）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T42 补测 R12 正文与 R13.S1 的总数
- Revision: 1
- GREEN: `bun run e2e --grep "N1 R12"` → 8 passed（`N1 R12.S5` 另断言 `address.focus` 后地址框文字全选；新增 `N1 R12 菜单后退与前进`）；`cargo test -q -p alter-core --test store n1_r13_s1` → PASS（另断言非 `sqlite_%` 的表恰好 8 张）
- RED (mutation: AddressBar 收到 `address.focus {newTab:false}` 不再 `select()`): `npx playwright test … --grep "N1 R12\.S5"` → FAIL
- RED (mutation: 菜单 `nav.back` 调成 `actions.nav("forward")`): `npx playwright test … --grep "N1 R12 菜单后退"` → FAIL
- RED (mutation: 迁移 0002 末尾多建一张表): `cargo test -q -p alter-core --test store n1_r13_s1` → FAIL: left: 9 right: 8
- GREEN: 恢复后上述三处都通过；`cargo test -q -p alter-core --test store` → 7 passed
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS（shell 52）；`bun test` 1 pass；`bun run e2e` 连跑 2 次 → 各 51 passed（SC4 p95 15.8 / 17.1 ms，SC2 111 / 133 ms），`N1 R8.S4` 没有再失败；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T43 已存档用例 `CR7 错误提示` 等 favicon 回写到齐再归档（验证中发现的偶发失败）
- Revision: 1
- 发现：最终验证跑全量 e2e 时 `CR7 错误提示` 失败（expect(locator).toHaveCount(expected) Expected: 0 Received: 1，sidebar.spec.ts:374）；`--repeat-each 8` 中 1 次、`--repeat-each 25` 中 1 次复现，失败时 alterd 日志为「拒绝 shell 事件 code=TabNotFound event=shell.event.tabUpdated」：标题之后的 favicon 回写在测试直接归档标签之后才到，被拒绝，提前出现了同样的提示
- GREEN: 前置步骤在归档前等 `tab-favicon` 可见 → `npx playwright test -c e2e/playwright.config.ts --grep "CR7 错误提示" --repeat-each 30` → 30 passed
- VERIFY (Widen，与 T44 合并): 见 T44

### T44 `N1 CR28` 的前置步骤先确认要固定的行已激活再按快捷键（验证中发现的偶发失败）
- Revision: 1
- 发现：全量 e2e 中 `N1 CR28` 偶发失败一次（expect(locator).toHaveAttribute(expected) failed）：前置步骤点击某行后立即触发菜单 `tab.togglePin`，Main 手上的快照还没反映这次激活，固定了另一行
- GREEN: 前置步骤改为「点击要固定的行 → 等它为激活样式 → `tab.togglePin` → 等它固定」→ `npx playwright test -c e2e/playwright.config.ts --grep "N1 CR28 " --repeat-each 15` → 15 passed
- VERIFY (Widen，T43–T44): `bun run lint`、`bun run typecheck` 通过；`cargo test --no-fail-fast` 全部 PASS（shell 52）；`bun run e2e` 连跑 2 次 → 各 51 passed（期间负载平均值 8→24）。另：T43 之后的两次全量 e2e 中，已存档用例 `R7.S1 启动拉起 alterd`（每次全量运行的第一条，3 秒内出现健康文件）各失败过一次（3.4 s、3.7 s），单独运行与重新构建后运行均在约 0.5–0.8 s 通过；当时负载平均值约 10，Microsoft Defender 进程占用较高，判断为环境负载，见决策与偏差。运行环境：本机 macOS，主会话

## 决策与偏差
- 起点（2026-10-21）：仓库 `alter`，分支 `feature/add-core-features`，HEAD `ca698a8`，工作区干净，非 linked worktree；`cargo test`（cli 4、alter-core 1+2+3、lifecycle 8、shell 21）与 `bun test`（1）全部通过，起点为绿。
- design: 迁移 0002 的重新编号由「UPDATE 里用相关子查询数排在前面的行」→「临时表 + `ROW_NUMBER()` + `UPDATE … FROM`」，因为 SQLite 逐行更新时子查询会读到本语句已改过的行，A(0)、B(1) 普通加 C(2) 固定会编成 (1, 1, 0)；已改 design.md「数据模型」，R13.S4 的测试加了这组数据。备份调用名照 rusqlite 0.40 改为 `MAIN_DB`。
- T2 额外加了 `n1_r14_s1_backup_failure_aborts_migration`（任务子项「备份失败要让 `Store::open` 返回错误」），归在 R14.S1 下。
- T3 的 RED 用变异取得：T2 的 GREEN 已让 alterd 建出版本 2 的库，把 `r3_s1` 改名为 `n1_r15_s1_run_writes_health_file` 并改期望为 2 后一跑就通过；Widen 时旧期望 1 的失败（left: 2 right: 1）就是这一点。
- T4 为了让 alterd 能编译：`TabRow` 带出已有的 `pinned` 列，快照照实输出；6 个新事件在 T5–T7 实现前统一回 `E_INTERNAL`「尚未实现」，T7 结束时这个分支应当已经全部被替换。
- T5 的 R4 两条测试在实现完成后才写（R1–R3 的实现已满足它们），RED 用两个变异取得；第一次跑变异时测试因漏导入 `v1_db` 编译失败，那次不算，修好导入后重跑并记录。`v1_db` 辅助放在 `crates/alterd/tests/common/mod.rs`。
- tasks.md：T6、T7 的 verify 原写作 `cargo test -p alterd --test shell n1_r5_ n1_r6_`，cargo 只接受一个 TESTNAME（报 Usage 错误），改为 `cargo test -p alterd --test shell -- n1_r5_ n1_r6_`（T7 同理）。
- T8：`e2e/app.ts` 的 `launchApp` 加 `app.context().setDefaultTimeout(5_000)`。第一次 RED 时元素缺失的点击一直等到 30 秒用例超时、不报定位器；配置文件的 `use.actionTimeout` 管不到 Electron 窗口（试过，无效，已撤回），只能在 context 上设。新增 `e2e/ui.ts` 放 N1 之后各 spec 共用的辅助（`ready`、`openNewTab`、`attachedUrls`、`attachedText`、`queryDb`、`spaceIdByName`），`sidebar.spec.ts` 里原有的同名辅助不动。
- T8：编辑面板与确认框画在侧边栏里（`.sidebar` 设 `position: relative`），因为内容区被标签页面的 WebContentsView 覆盖；只剩一个 Space 时面板里的「删除」按钮置灰（alterd 也会拒绝）。
- T9 拖拽用例偶发失败（第一轮 `--repeat-each 5` 有 1 次、`--repeat-each 20` 有 1 次）。根因：每份快照都是新对象，Solid 的 `<For>` 按引用对比，于是每份快照（包括 favicon 回写这类无关变化）都会把全部标签行的 DOM 节点重建一遍；拖拽中途行被替换就丢了这次拖拽，`boundingBox()` 也会因节点脱离而返回 null。修复：`App.tsx` 用 `solid-js/store` 的 `reconcile` 按 id 合并快照（solid-js 自带，不加依赖）。修复前诊断：激活另一个标签移除 6 个行节点；修复后为 0，拖拽用例连跑 30 次通过。用例的落点同时由距行顶 2 px 改为行高的四分之一，仍在「Page A 上方」。
- 评审第 1 轮 codex 的疑问：`shell.discard` 只发给请求方，若多个 app 共用一个 alterd，其他连接的固定标签视图会停在旧 URL。按设计 Electron Main 是 shell.sock 唯一的客户端（单窗口、多窗口不在范围内），不作为发现；多客户端支持留待以后需要时再定（后续）。
- CR11 原计划的收敛任务 T21（改为「下一次用户操作时清空错误」）没有执行就删去：`bun run e2e --grep "N1 CR11 " --repeat-each 5` 在未改代码时 5 次全部通过，拿不到 RED，见第 4 轮的 accepted 理由；用例保留。
- T22 让每个连接严格按处理顺序写出后，CR11 描述的「错误越过更早的快照」也不会再发生；CR11 的处理仍按第 4 轮记为 accepted，不改历史记录。
- T23 的 seam 由 tasks.md 写的 Shell 假客户端改为 `crates/alterd/src/shell/mod.rs` 里对 `write_reply` 的单元测试：要让连接在补写途中被另一个客户端挤到丢帧，契约测试的时机无法稳定；单元测试用容量 2 的广播通道确定地构造丢帧。
- 第 3–7 轮围绕消息顺序连续出现新发现（CR9、CR12、CR13、CR15），根因是 T19 起用「私有回复优先 + 序号补写」修补广播模型；T25 改为每个连接一条发件箱，顺序由构造保证，T19 / T22 / T23 的机制随之删除。
- T27：`n1_cr12_*` 原来每轮直接对上一轮已关闭（已卸载）的固定标签发 `tabUpdated`，与「已卸载期间忽略回写」的新契约矛盾，改为每轮先激活；测试本意（回复不越过更早的快照）不变。
- 第 8 轮起 Main 不再自己推断卸载状态（T20、T26 的 `closing` / `discarded` / `wake` 删除），改由 alterd 在快照里给出 `discarded`。
- T33：规格 R10 只规定「有相邻标签时把激活转给它」，没有相邻标签时未写。补成「已卸载的标签不算激活」：Space 只剩已卸载标签时没有激活标签，内容区显示「没有打开的标签」，作用于激活标签的刷新等操作随之不可用，在地址框打开地址会新建标签（`N1 CR24` 的 e2e 期望因此由「在固定标签里打开」改为「新开标签」）。场景的 WHEN/THEN 都不变。
- T41 期间 `N1 R8.S4` 偶发失败一次（固定后 1 秒内顺序未变），之后定向 13 次、全量 e2e 4 次都没有复现，原因未找到。用户已决定接受并记录（2026-10-21）。后续：给 e2e 加失败时保留 trace，再出现时可查。
- T43：已存档需求的用例 `CR7 错误提示` 有前置步骤竞态（迟到的 favicon 回写），只改测试的前置等待，断言不变。
- 已存档用例 `R7.S1 启动拉起 alterd` 在高负载时偶发超过 3 秒（两次，均为全量运行的第一条用例）；3 秒是该场景本身的时限，不放宽。重新构建后首次启动 alterd 只多约 0.25 s，「新二进制首次执行被扫描」的推测不足以解释 3 秒，原因未确认。用户已决定按环境负载接受并记录（2026-10-21）；后续：e2e 失败时保留 trace，在 CI 上观察。
- 收敛任务 T12–T16 改动的文件互相重叠（`pinned.spec.ts`、`tab-views.ts`），合成一次提交；design.md 同步改了 `shell.discard` 的「已卸载」规则、删除时加 `clearCache`、关闭普通标签也重新编号，以及架构图与删除时序图。
- T10：design.md 里共用动作的名字 `togglePin` 按代码改为 `togglePinActive`（`createActions(ctx)` 生成）。快捷键只经菜单项点击测试，真实按键没有自动化覆盖（已在 GATE 1 列为风险）。
- T11：灌数据改为 app 启动前直接拉起 alterd、经 `shell.sock` 发事件（与已有 `SC2` 的 `seedTabs` 同法），因为测试拿不到主进程模块里的 `shell` 对象；计时起点由「主进程收到 `space.activate` IPC」改为「Renderer 调用 `activateSpace`」，比任务原写法更早，只会高估。

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
- Scope: alter `git diff ca698a8...HEAD`（N1 的 T1–T11，不含 `packages/protocol/src/generated/`），无未提交改动；reviewer: codex (gpt-6-sol)
- Tasks: T1-T11
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR1 | major | correctness | apps/browser/src/renderer/src/Sidebar.tsx:107 | 拖到分隔线时发 `index: Number.MAX_SAFE_INTEGER`，协议的 `index` 是 `u32`，alterd 解析失败并按 R2.S2 丢弃，标签既不移动也不固定（R11） | T12 |
| CR2 | major | correctness | apps/browser/src/renderer/src/Sidebar.tsx:99 | 所有标签都固定后普通分区没有落点，无法用拖拽把标签移回普通分区（R11） | T12 |
| CR3 | major | spec | crates/alter-core/src/store/tabs.rs:145 | 关闭普通标签只归档，剩下的 position 出现空洞（A、C 为 0、2），与 design.md「同 Space 内连续编号 0..n-1」不符（R9.S1） | T13 |
| CR4 | major | correctness | apps/browser/src/main/tab-views.ts:48 | Space 里只有一个固定标签时关闭它：`shell.discard` 先到则随后的快照立即按 pinned_url 重建页面，快照先到则保持卸载，结果取决于消息顺序（R10） | T14 |
| CR5 | major | security | apps/browser/src/main/tab-views.ts:41 | 删除 Space 只 `clearStorageData()`，HTTP 缓存要到下次启动删目录才消失；app 运行期间该 Space 的缓存仍在磁盘上（R5，父级隐私要求）。codex 建议 blocker；降为 major：数据只留到下次启动，且别的 Space 读不到 | T15 |
| CR6 | major | tests | apps/browser/e2e/perf.spec.ts:94 | 两个 Space 的 250 行内容相同，只检查名称和行数，列表没切换也会被计为完成，SC4 证明不了界面已切换 | T16 |

Review: CHANGES REQUESTED

### Round 2
- Revision: 1
- Scope: alter `git diff a653ebd...HEAD`（收敛任务 T12–T16），无未提交改动；reviewer: codex (gpt-6-sol)
- Tasks: T12, T13, T14, T15, T16
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR7 | major | correctness | apps/browser/src/main/tab-views.ts:63 | `wake()` 立即应用本地缓存的上一份快照：固定标签导航到 /b 后关闭，`shell.discard` 已到而新快照未到时用户立即点击，会按旧快照重建 /b，新快照到达时视图已存在不会改回 /a（R10.S2） | T17 |
| CR8 | major | tests | apps/browser/e2e/perf.spec.ts:97 | 计时终点只核对首行与行数，首行已换而其余 249 行仍是旧列表时也会结束计时；少于 T16 写明的「第一行和最后一行」（SC4） | T18 |

Review: CHANGES REQUESTED

### Round 3
- Revision: 1
- Scope: alter `git diff 92a9062...HEAD`（收敛任务 T17、T18），无未提交改动；reviewer: codex (gpt-6-sol)
- Tasks: T12, T13, T14, T15, T16, T17, T18
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR9 | major | correctness | crates/alterd/src/shell/mod.rs:70 | 连接任务的 `tokio::select!` 不分先后：私有回复 `shell.discard` 可能晚于关闭及随后重新激活的快照送达；用户关闭固定标签后立即点击，最后到的 discard 会卸载刚重建的视图，之后没有快照再建，内容区一直空白（R10.S2，前两轮遗漏） | T19 |

Review: CHANGES REQUESTED

### Round 4
- Revision: 1
- Scope: alter `git diff ef47ae5...HEAD`（收敛任务 T19），无未提交改动；reviewer: codex (gpt-6-sol)
- Tasks: T19
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR10 | major | correctness | apps/browser/src/main/tab-views.ts:65 | Main 发出关闭后、`shell.discard` 到达前用户再次激活该固定标签：`wake()` 无标记可清，迟到的 discard 再打上标记，之后的快照不建视图，内容区一直空白（R10.S2） | T20 |
| CR11 | minor | correctness | crates/alterd/src/shell/mod.rs:74 | 私有回复优先后，后一个事件的 `shell.error` 可能越过前一个事件尚未写出的快照；Main 收到快照即清空错误，用户看不到拒绝原因 | accepted: 本机连发「成功激活 + 关闭不存在的标签」5 次复现不出（连接任务总在 alterd 处理下一个事件前写出快照），后果只是一次错误提示被提前清掉；用例 `N1 CR11` 留作回归保护 |

Review: CHANGES REQUESTED

### Round 5
- Revision: 1
- Scope: alter `git diff 1241689...HEAD`（收敛任务 T20），无未提交改动；reviewer: codex (gpt-6-sol)
- Tasks: T20
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR12 | major | correctness | crates/alterd/src/shell/mod.rs:74 | 私有回复优先只保证回复先于同一事件的快照，却会越过更早事件尚未写出的快照：先发 `tabUpdated`（/b）再关闭并立即激活固定标签时，线上顺序可为 discard → 旧快照(/b) → 关闭快照 → 激活快照，旧快照按 /b 重建视图，内容区停在 /b（R10.S2）。根因是 T19 的 `biased` 不是先进先出 | T22 |

Review: CHANGES REQUESTED

### Round 6
- Revision: 1
- Scope: alter `git diff 191f9b8...HEAD`（收敛任务 T22），无未提交改动；reviewer: codex (gpt-6-sol)
- Tasks: T22
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR13 | major | correctness | crates/alterd/src/shell/mod.rs:78 | 连接落后超过广播容量而丢帧时，补写循环只看 `seq > written`：序号 ≤ `after` 的快照都已丢失时会先写更新的快照、再写较早事件的回复，`shell.navigate` / `shell.discard` 落在更新的状态之后（CR12、R10.S2） | T23 |
| CR14 | major | correctness | apps/browser/src/main/tab-views.ts:124 | 关闭固定标签后、`shell.discard` 到达前，旧视图若完成一次导航，`did-navigate` 照常上报 `tabUpdated`，alterd 把固定标签的 url 又改成导航后的地址，之后激活按它重建（R10.S2，T22 之前就存在） | T24 |

Review: CHANGES REQUESTED

### Round 7
- Revision: 1
- Scope: alter `git diff c47acfe...HEAD`（收敛任务 T23、T24），无未提交改动；reviewer: codex (gpt-6-sol)
- Tasks: T23, T24
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR15 | major | correctness | crates/alterd/src/shell/mod.rs:152 | 丢帧后积压两条回复时，第一条回复把更新的快照一并写出，第二条回复落在该快照之后（例如被拒绝的 `shell.error` 先被更晚的成功快照「清掉」又重新出现）；T19 / T22 / T23 的序号补写机制仍不是先进先出 | T25 |
| CR16 | major | correctness | apps/browser/src/main/tab-views.ts:119 | 关闭后、`shell.discard` 前再次激活会清掉 `closing`，此时旧视图完成导航的事件被放行，alterd 把已复位的固定标签 url 又改成导航地址，随后按错误地址重建（R10.S2、CR10） | T26 |

Review: CHANGES REQUESTED

### Round 8
- Revision: 1
- Scope: alter `git diff 6954454...HEAD`（收敛任务 T25、T26），无未提交改动；reviewer: codex (gpt-6-sol)
- Tasks: T25, T26
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR17 | major | correctness | apps/browser/src/main/tab-views.ts:78 | `closingTab` 关掉固定标签视图后，关闭前已发出、尚未送达的快照进入 `apply` 时只排除 `discarded`，会按旧 url 重建视图；若用户随后再次激活，新视图的导航事件又能把已复位的 url 写回（R10.S2） | fixed in R15 |
| CR18 | major | correctness | apps/browser/src/main/actions.ts:38 | 是否立即关视图取自 Main 已收到的快照：连发「固定、关闭」时固定快照尚未到达，`pinned` 为 false，旧视图不关，迟到页面事件仍可能写回 url（R8.S1、R10.S2） | fixed in R15 |
| CR19 | minor | maintainability | crates/alterd/src/tabs/mod.rs:91 | 断开连接的发件箱只在下一次成功发布快照时移除，只有连接与断开、没有状态变化时会累积 | T29 |
| CR20 | minor | correctness | crates/alterd/src/shell/mod.rs:56 | 连接任务自己持有发件箱发送端：TabService 意外结束后空闲连接不会关闭，客户端保持一条不再更新的连接 | T29 |

Review: CHANGES REQUESTED

### Round 9
- Revision: 1
- Scope: alter `git diff 91ea26a...HEAD`（收敛任务 T27–T29），无未提交改动；reviewer: codex (gpt-6-sol)
- Tasks: T27, T28, T29
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR21 | major | correctness | apps/browser/src/main/tab-views.ts:54 | `voidView` 只作废关闭当时已有的视图：已卸载且无视图的固定标签被快速「激活、关闭、再激活」时，第一次激活的快照可在 discard 前建出未作废的视图，它的导航回写落在第二次激活之后被接受，url 从固定地址变成导航地址（R10.S2、CR10） | T30 |
| CR22 | minor | correctness | apps/browser/src/main/tab-views.ts:117 | T28 把关闭时立即关视图改成只作废：discard 到达前旧页面仍在运行，期间 `window.open` 会让 alterd 在关闭之后新建一个标签（R10.S2） | T30 |

Review: CHANGES REQUESTED

### Round 10
- Revision: 1
- Scope: alter `git diff 79d2348...HEAD`（收敛任务 T30），无未提交改动；reviewer: codex (gpt-6-sol)
- Tasks: T30
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR23 | major | correctness | apps/browser/src/main/tab-views.ts:64 | 关闭屏障按标签只记一个标记：回复到达前依次「关闭、激活、关闭、激活」同一固定标签时，第一条 discard 撤掉了第二次关闭的屏障，第一次激活的快照随即建出视图，它在第二条 discard 前的回写或弹窗仍会生效（R10.S2、CR10） | T31 |

Review: CHANGES REQUESTED

### Round 11
- Revision: 1
- Scope: alter `git diff 833c4c5...HEAD`（收敛任务 T31）；另做一次汇总确认 `git diff a653ebd...HEAD`（第 1 轮之后全部收敛改动的最终状态，逐条复核 CR1–CR23），无未提交改动；reviewer: codex (gpt-6-sol)
- Tasks: T12, T13, T14, T15, T16, T17, T18, T19, T20, T22, T23, T24, T25, T26, T27, T28, T29, T30, T31
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR24 | major | correctness | crates/alterd/src/tabs/mod.rs:138 | 关闭当前 Space 唯一的固定标签后它仍是激活标签但已卸载：在地址框输入网址回车，alterd 只回 `shell.navigate`，Main 找不到视图静默跳过，网址打不开也没有提示（R10、已存档的「当前标签导航」） | T32 |
| CR25 | nit | maintainability | packages/protocol/src/generated/TabView.ts:14 | 生成文件有行尾空格 | accepted: ts-rs 的输出格式（已存档的 HealthFile.ts 同样如此），生成文件禁止手改 |

汇总确认：CR1–CR10、CR12–CR23 在当前代码里均已关闭，CR11 维持 accepted 且 FIFO 发件箱已消除其越序路径。

Review: CHANGES REQUESTED

### Round 12
- Revision: 1
- Scope: alter `git diff 5421a60...HEAD`（收敛任务 T32），并检查激活标签已卸载时 Main 的其他操作，无未提交改动；reviewer: codex (gpt-6-sol)
- Tasks: T32
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR26 | major | correctness | crates/alterd/src/tabs/mod.rs:143 | 先从 `discarded` 移除再写 url：写库失败时请求返回错误，但下一次广播会把旧 url 的标签标为未卸载，Main 重建旧页面（CR24） | T33 |
| CR27 | major | spec | crates/alterd/src/tabs/mod.rs:213 | 已卸载的标签在快照里仍可能是 Space 的激活标签（唯一的固定标签被关闭后）：侧边栏高亮它、刷新按钮可用，但刷新、取消固定、切换 Space 再切回都只留下空白内容区，操作静默无效（R10、R12.S7、R8.S2、R3.S3） | T33 |

Review: CHANGES REQUESTED

### Round 13
- Revision: 1
- Scope: alter `git diff 0100522...HEAD`（收敛任务 T33），无未提交改动；reviewer: codex (gpt-6-sol)
- Tasks: T33
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR28 | minor | correctness | apps/browser/src/main/actions.ts:78 | 没有激活标签时（同一 Space 的固定标签都已关闭）`cycleTab` 把找不到的 -1 当作 0，`tab.next` 跳过第一行、`tab.prev` 不从最后一行开始（R12.S4、CR27） | T34 |

Review: CHANGES REQUESTED

### Round 14
- Revision: 1
- Scope: alter `git diff efba16c...HEAD`（收敛任务 T34）与汇总确认 `git diff a653ebd...HEAD`（逐条复核 CR1–CR28），无未提交改动；reviewer: codex (gpt-6-sol)
- Tasks: T34
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR29 | major | correctness | apps/browser/src/main/actions.ts:44 | 关闭唯一的固定标签后、关闭快照到达前在地址框打开地址：Main 仍把它当激活标签附上 tabId，alterd 走「已卸载标签改 url」分支，固定标签被改写为新地址而不是新开标签（R10、CR24、CR27） | T35 |
| CR30 | minor | correctness | apps/browser/src/main/actions.ts:76 | 关闭激活标签后、新快照到达前按 `tab.next`：Main 仍按旧的激活标签计算下一行，可选错行（R12.S4、CR28） | T35 |

汇总确认：CR1–CR10、CR12–CR24、CR26–CR28 的原故障路径在当前代码里均已关闭；CR11、CR25 维持 accepted 且未恶化。

Review: CHANGES REQUESTED

### Round 15
- Revision: 1
- Scope: alter `git diff 5c234c1...HEAD`（收敛任务 T35）；连同第 14 轮对 `git diff a653ebd...HEAD` 最终状态的汇总确认（CR1–CR28），全部收敛任务的最终代码都已读过，无未提交改动；reviewer: codex (gpt-6-sol)
- Tasks: T12, T13, T14, T15, T16, T17, T18, T19, T20, T22, T23, T24, T25, T26, T27, T28, T29, T30, T31, T32, T33, T34, T35
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR31 | minor | correctness | apps/browser/src/main/actions.ts:81 | 固定标签 A 排在首位且激活，用户在关闭快照到达前连续「关闭 A、`tab.next`」时，`cycleTab` 的候选列表仍含屏障中的 A，会发 `tabActivated(A)` 把刚卸载的页面重新打开（R12.S4、CR30） | accepted: 只在几毫秒内连续操作时出现，后果是刚关闭的固定页面被重新打开，用户再操作一次即可恢复 |

Review: PASS

### Round 16
- Revision: 1
- Scope: alter `git diff c4ee797...HEAD`（验证缺口的收敛任务 T36–T38），无未提交改动；reviewer: codex (gpt-6-sol)
- Tasks: T36, T37, T38
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR32 | major | tests | apps/browser/e2e/ui.ts:72 | `expectWithin` 只限制依次断言的总耗时，条件 A 检查后消失、B 随后成立也能通过，证明不了 THEN 的各条件同时成立（R1.S5、R3.S3、R5.S5、R8.S4、R9.S2、R10.S2、R12.S2、R12.S5、R12.S6） | T39 |
| CR33 | major | tests | apps/browser/e2e/pinned.spec.ts:66 | R8.S4 的分隔线位置在时限检查之后才读，「顺序对、分隔线超过 3 秒才对」也能通过（R8.S4） | T39 |
| CR34 | major | tests | apps/browser/e2e/pinned.spec.ts:96 | R10.S2 用标题判断保留的是被关闭的行：删掉原标签、另建同标题固定标签的错误实现也能通过（R10.S2） | T39 |
| CR35 | minor | tests | apps/browser/e2e/ui.ts:76 | 计时用 `Date.now()`，系统校时回拨时会低估耗时，「只会高估」不严格成立 | T39 |

Review: CHANGES REQUESTED

### Round 17
- Revision: 1
- Scope: alter `git diff c34fe2a...HEAD`（收敛任务 T39），无未提交改动；reviewer: codex (gpt-6-sol)
- Tasks: T39
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR36 | major | tests | apps/browser/e2e/ui.ts:101 | `aboveDivider` 只比较 DOM 先后：CSS 把分隔线挪到行上方或让它不可见时测试仍通过，证明不了用户看到的固定分区（R8.S4、R10.S2、R12.S6） | T40 |
| CR37 | major | tests | apps/browser/e2e/pinned.spec.ts:127 | R10.S2 的标签 id 读自 Main 快照，可见行本身没有 id，证明不了保留下来的可见行就是被关闭的那个标签（R10.S2） | T40 |

Review: CHANGES REQUESTED

### Round 18
- Revision: 1
- Scope: alter `git diff f28f4bc...HEAD`（收敛任务 T40），无未提交改动；reviewer: codex (gpt-6-sol)
- Tasks: T40
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR38 | major | tests | apps/browser/e2e/ui.ts:101 | 分隔线由 `border-top` 画出：边框颜色误改为 `transparent` 时线看不见，元素尺寸与坐标却正常，`aboveDivider` 仍为真（R8.S4、R10.S2、R12.S6） | T41 |

Review: CHANGES REQUESTED

### Round 19
- Revision: 1
- Scope: alter `git diff 0e7ccb0...HEAD`（收敛任务 T41）；T36–T40 的代码已在第 16–18 轮读过，后一轮均确认前一轮的发现已修好，无未提交改动；reviewer: codex (gpt-6-sol)
- Tasks: T36, T37, T38, T39, T40, T41
Findings: none.

Review: PASS

### Round 20
- Revision: 1
- Scope: alter `git diff 9d72ae4...HEAD`（第二次验证缺口的收敛任务 T42），无未提交改动；reviewer: codex (gpt-6-sol)
- Tasks: T42
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR39 | minor | maintainability | .dev-flow/changes/add-core-features/nodes/space-tabs/tasks.md:126 | T42 的 verify 用 `--grep "N1 R12\."`，匹配不到新增的 `N1 R12 菜单后退与前进`，按任务清单定向验证时会漏跑 | accepted: 已把 verify 改为 `--grep "N1 R12"`（只改文档）；本轮日志里的 `--grep "N1 R12"` 与全量 e2e 都包含该用例 |

Review: PASS

### Round 21
- Revision: 1
- Scope: alter `git diff d1781b0...HEAD`（验证中发现的测试前置竞态修复 T43、T44），无未提交改动；reviewer: codex (gpt-6-sol)
- Tasks: T43, T44
Findings: none.

Review: PASS

## 验证报告
<!-- 每次验证追加一份新报告，放在最近一轮评审之后。以 "- Revision: <当前 revision>" 和 "- Review: Round <最近一轮>" 开头。由验证步骤填写：本仓库的测试结果，每个场景一行 "| R<n>.S<m> | <path::test> | PASS |"，成功标准对照表，收敛缺口，fix 类型还要写 "Fix: verified | partial | failed"，最后恰好一行 "Overall: READY" 或 "Overall: NOT READY"。 -->

- Revision: 1
- Review: Round 15
- Verifier: codex (gpt-6-sol)

| Repo | Command | Result |
|---|---|---|
| alter | `cargo test` | PASS（cli 4、alter-core 1+3+7、lifecycle 8、shell 52；shell_perf 1 ignored） |
| alter | `bun test` | PASS（1 test） |
| alter | `cargo fmt --check && cargo clippy --all-targets -- -D warnings && bun run lint` | PASS |
| alter | `bun run typecheck` | PASS |
| alter | `cargo build && bun run build` | PASS |
| alter | `bun run e2e` | PASS（50 tests） |

| Scenario | Test | Result |
|---|---|---|
| R1.S1 | crates/alterd/tests/shell.rs::n1_r1_s1_created_space_becomes_active | PASS |
| R1.S2 | crates/alterd/tests/shell.rs::n1_r1_s2_blank_name_is_rejected | PASS |
| R1.S3 | crates/alterd/tests/shell.rs::n1_r1_s3_name_longer_than_32_is_rejected | PASS |
| R1.S4 | crates/alterd/tests/shell.rs::n1_r1_s4_unknown_color_is_rejected | PASS |
| R1.S5 | apps/browser/e2e/spaces.spec.ts::N1 R1.S5 在侧边栏新建 Space | PASS |
| R2.S1 | crates/alterd/tests/shell.rs::n1_r2_s1_rename_keeps_id_and_active_space | PASS |
| R2.S2 | crates/alterd/tests/shell.rs::n1_r2_s2_update_unknown_space_is_rejected | PASS |
| R2.S3 | apps/browser/e2e/spaces.spec.ts::N1 R2.S3 在侧边栏改名 | PASS |
| R3.S1 | crates/alterd/tests/shell.rs::n1_r3_s1_activate_changes_active_space | PASS |
| R3.S2 | crates/alterd/tests/shell.rs::n1_r3_s2_activate_unknown_space_is_rejected | PASS |
| R3.S3 | apps/browser/e2e/spaces.spec.ts::N1 R3.S3 每个 Space 记住自己的激活标签 | PASS |
| R4.S1 | crates/alterd/tests/shell.rs::n1_r4_s1_restart_keeps_active_space | PASS |
| R4.S2 | crates/alterd/tests/shell.rs::n1_r4_s2_legacy_db_activates_first_space | PASS |
| R5.S1 | crates/alterd/tests/shell.rs::n1_r5_s1_delete_active_middle_space_activates_next | PASS |
| R5.S2 | crates/alterd/tests/shell.rs::n1_r5_s2_delete_active_last_space_activates_previous | PASS |
| R5.S3 | crates/alterd/tests/shell.rs::n1_r5_s3_delete_inactive_space_keeps_active | PASS |
| R5.S4 | crates/alterd/tests/shell.rs::n1_r5_s4_last_space_cannot_be_deleted | PASS |
| R5.S5 | apps/browser/e2e/spaces.spec.ts::N1 R5.S5 在侧边栏删除 Space 并清掉 Cookie | PASS |
| R5.S6 | apps/browser/e2e/spaces.spec.ts::N1 R5.S6 取消确认不删除 | PASS |
| R6.S1 | crates/alterd/tests/shell.rs::n1_r6_s1_startup_sweeps_orphan_partitions | PASS |
| R7.S1 | apps/browser/e2e/spaces.spec.ts::N1 R7.S1 另一个 Space 看不到 Cookie | PASS |
| R8.S1 | crates/alterd/tests/shell.rs::n1_r8_s1_pin_moves_tab_to_pinned_section | PASS |
| R8.S2 | crates/alterd/tests/shell.rs::n1_r8_s2_unpin_moves_tab_to_head_of_normal_section | PASS |
| R8.S3 | crates/alterd/tests/shell.rs::n1_r8_s3_pin_unknown_tab_is_rejected | PASS |
| R8.S4 | apps/browser/e2e/pinned.spec.ts::N1 R8.S4 在侧边栏固定并在重启后保持 | PASS |
| R9.S1 | crates/alterd/tests/shell.rs::n1_r9_s1_close_active_middle_tab_activates_next | PASS |
| R9.S2 | apps/browser/e2e/sidebar.spec.ts::N1 R9.S2 关闭最后一个标签 | PASS |
| R10.S1 | crates/alterd/tests/shell.rs::n1_r10_s1_closing_pinned_tab_discards_it | PASS |
| R10.S2 | apps/browser/e2e/pinned.spec.ts::N1 R10.S2 在侧边栏关闭固定标签后再打开 | PASS |
| R11.S1 | crates/alterd/tests/shell.rs::n1_r11_s1_move_within_normal_section | PASS |
| R11.S2 | crates/alterd/tests/shell.rs::n1_r11_s2_move_into_pinned_section_pins | PASS |
| R11.S3 | crates/alterd/tests/shell.rs::n1_r11_s3_index_out_of_range_moves_to_end | PASS |
| R11.S4 | crates/alterd/tests/shell.rs::n1_r11_s4_move_unknown_tab_is_rejected | PASS |
| R11.S5 | apps/browser/e2e/pinned.spec.ts::N1 R11.S5 在侧边栏拖动并在重启后保持 | PASS |
| R12.S1 | apps/browser/e2e/shortcuts.spec.ts::N1 R12.S1 菜单项与快捷键 | PASS |
| R12.S2 | apps/browser/e2e/shortcuts.spec.ts::N1 R12.S2 关闭激活标签 | PASS |
| R12.S3 | apps/browser/e2e/shortcuts.spec.ts::N1 R12.S3 按序号切换 Space | PASS |
| R12.S4 | apps/browser/e2e/shortcuts.spec.ts::N1 R12.S4 循环切换标签 | PASS |
| R12.S5 | apps/browser/e2e/shortcuts.spec.ts::N1 R12.S5 新标签与聚焦地址框 | PASS |
| R12.S6 | apps/browser/e2e/shortcuts.spec.ts::N1 R12.S6 快捷键固定激活标签 | PASS |
| R12.S7 | apps/browser/e2e/shortcuts.spec.ts::N1 R12.S7 快捷键刷新 | PASS |
| R13.S1 | crates/alter-core/tests/store.rs::n1_r13_s1_fresh_open_creates_schema | PASS |
| R13.S2 | crates/alter-core/tests/store.rs::n1_r13_s2_reopen_is_idempotent | PASS |
| R13.S3 | crates/alter-core/tests/store.rs::r1_s3_wal_and_foreign_keys | PASS |
| R13.S4 | crates/alter-core/tests/store.rs::n1_r13_s4_existing_tabs_are_renumbered | PASS |
| R14.S1 | crates/alter-core/tests/store.rs::n1_r14_s1_upgrade_from_v1_writes_backup | PASS |
| R14.S2 | crates/alter-core/tests/store.rs::n1_r14_s2_no_backup_for_fresh_or_current_db | PASS |
| R15.S1 | crates/alterd/tests/lifecycle.rs::n1_r15_s1_run_writes_health_file | PASS |
| R15.S2 | crates/alterd/tests/lifecycle.rs::r3_s2_sigterm_exits_cleanly | PASS |
| R15.S3 | crates/alterd/tests/lifecycle.rs::r3_s3_second_instance_is_rejected | PASS |
| R15.S4 | crates/alterd/tests/lifecycle.rs::r3_s4_stale_health_file_is_taken_over | PASS |
| R16.S1 | crates/alter-core/tests/export_ts.rs::n1_r16_s1_export_error_code_and_health_file | PASS |
| R16.S2 | crates/alter-core/tests/export_ts.rs::n1_r16_s2_export_shell_messages | PASS |

| SC | Check | Result |
|---|---|---|
| SC4 | `bun run e2e --grep SC4` → 20 次切换 p50 14.3 ms，p95 15.4 ms | PASS（< 50 ms） |

运行环境：主会话本机 macOS（Darwin 25.6.0）对 HEAD `12187ed`；codex 只读核对测试与代码，并在沙箱内复跑了 `cargo test -p alter-core`、`bun test`、lint、typecheck。

Gaps:
- partial：R1.S5、R3.S3、R5.S5、R8.S4、R9.S2、R10.S2、R12.S2、R12.S5、R12.S6 的 e2e 把 THEN 的几个条件分开等待，证明不了它们在场景时限内同时成立；R10.S2 没断言保留的是被关闭的那一行 → T36
- partial：启动清理只识别小写 `space-<id>` 目录，R6 写明不区分大小写 → T37
- contradicts（设计文档落后）：design.md 仍写 Main 自行记录「已卸载」、删除最后一个 Space 时「面板显示错误」 → T38
- 说明：R4.S1/S2、R15.S1 的 RED 是实现后用变异取得（tdd.md 允许，执行日志已注明）；localStorage 清除没有直接断言（R5.S5 只要求 Cookie，记为后续）

Overall: NOT READY

- Revision: 1
- Review: Round 19
- Verifier: codex (gpt-6-sol)

| Repo | Command | Result |
|---|---|---|
| alter | `cargo test` | PASS（cli 4、alter-core 1+3+7、lifecycle 8、shell 52；shell_perf 1 ignored） |
| alter | `bun test` | PASS（1 test） |
| alter | `cargo fmt --check && cargo clippy --all-targets -- -D warnings && bun run lint` | PASS |
| alter | `bun run typecheck` | PASS |
| alter | `cargo build && bun run build` | PASS |
| alter | `bun run e2e` | PASS（50 tests） |

| Scenario | Test | Result |
|---|---|---|
| R1.S1 | crates/alterd/tests/shell.rs::n1_r1_s1_created_space_becomes_active | PASS |
| R1.S2 | crates/alterd/tests/shell.rs::n1_r1_s2_blank_name_is_rejected | PASS |
| R1.S3 | crates/alterd/tests/shell.rs::n1_r1_s3_name_longer_than_32_is_rejected | PASS |
| R1.S4 | crates/alterd/tests/shell.rs::n1_r1_s4_unknown_color_is_rejected | PASS |
| R1.S5 | apps/browser/e2e/spaces.spec.ts::N1 R1.S5 在侧边栏新建 Space | PASS |
| R2.S1 | crates/alterd/tests/shell.rs::n1_r2_s1_rename_keeps_id_and_active_space | PASS |
| R2.S2 | crates/alterd/tests/shell.rs::n1_r2_s2_update_unknown_space_is_rejected | PASS |
| R2.S3 | apps/browser/e2e/spaces.spec.ts::N1 R2.S3 在侧边栏改名 | PASS |
| R3.S1 | crates/alterd/tests/shell.rs::n1_r3_s1_activate_changes_active_space | PASS |
| R3.S2 | crates/alterd/tests/shell.rs::n1_r3_s2_activate_unknown_space_is_rejected | PASS |
| R3.S3 | apps/browser/e2e/spaces.spec.ts::N1 R3.S3 每个 Space 记住自己的激活标签 | PASS |
| R4.S1 | crates/alterd/tests/shell.rs::n1_r4_s1_restart_keeps_active_space | PASS |
| R4.S2 | crates/alterd/tests/shell.rs::n1_r4_s2_legacy_db_activates_first_space | PASS |
| R5.S1 | crates/alterd/tests/shell.rs::n1_r5_s1_delete_active_middle_space_activates_next | PASS |
| R5.S2 | crates/alterd/tests/shell.rs::n1_r5_s2_delete_active_last_space_activates_previous | PASS |
| R5.S3 | crates/alterd/tests/shell.rs::n1_r5_s3_delete_inactive_space_keeps_active | PASS |
| R5.S4 | crates/alterd/tests/shell.rs::n1_r5_s4_last_space_cannot_be_deleted | PASS |
| R5.S5 | apps/browser/e2e/spaces.spec.ts::N1 R5.S5 在侧边栏删除 Space 并清掉 Cookie | PASS |
| R5.S6 | apps/browser/e2e/spaces.spec.ts::N1 R5.S6 取消确认不删除 | PASS |
| R6.S1 | crates/alterd/tests/shell.rs::n1_r6_s1_startup_sweeps_orphan_partitions | PASS |
| R7.S1 | apps/browser/e2e/spaces.spec.ts::N1 R7.S1 另一个 Space 看不到 Cookie | PASS |
| R8.S1 | crates/alterd/tests/shell.rs::n1_r8_s1_pin_moves_tab_to_pinned_section | PASS |
| R8.S2 | crates/alterd/tests/shell.rs::n1_r8_s2_unpin_moves_tab_to_head_of_normal_section | PASS |
| R8.S3 | crates/alterd/tests/shell.rs::n1_r8_s3_pin_unknown_tab_is_rejected | PASS |
| R8.S4 | apps/browser/e2e/pinned.spec.ts::N1 R8.S4 在侧边栏固定并在重启后保持 | PASS |
| R9.S1 | crates/alterd/tests/shell.rs::n1_r9_s1_close_active_middle_tab_activates_next | PASS |
| R9.S2 | apps/browser/e2e/sidebar.spec.ts::N1 R9.S2 关闭最后一个标签 | PASS |
| R10.S1 | crates/alterd/tests/shell.rs::n1_r10_s1_closing_pinned_tab_discards_it | PASS |
| R10.S2 | apps/browser/e2e/pinned.spec.ts::N1 R10.S2 在侧边栏关闭固定标签后再打开 | PASS |
| R11.S1 | crates/alterd/tests/shell.rs::n1_r11_s1_move_within_normal_section | PASS |
| R11.S2 | crates/alterd/tests/shell.rs::n1_r11_s2_move_into_pinned_section_pins | PASS |
| R11.S3 | crates/alterd/tests/shell.rs::n1_r11_s3_index_out_of_range_moves_to_end | PASS |
| R11.S4 | crates/alterd/tests/shell.rs::n1_r11_s4_move_unknown_tab_is_rejected | PASS |
| R11.S5 | apps/browser/e2e/pinned.spec.ts::N1 R11.S5 在侧边栏拖动并在重启后保持 | PASS |
| R12.S1 | apps/browser/e2e/shortcuts.spec.ts::N1 R12.S1 菜单项与快捷键 | PASS |
| R12.S2 | apps/browser/e2e/shortcuts.spec.ts::N1 R12.S2 关闭激活标签 | PASS |
| R12.S3 | apps/browser/e2e/shortcuts.spec.ts::N1 R12.S3 按序号切换 Space | PASS |
| R12.S4 | apps/browser/e2e/shortcuts.spec.ts::N1 R12.S4 循环切换标签 | PASS |
| R12.S5 | apps/browser/e2e/shortcuts.spec.ts::N1 R12.S5 新标签与聚焦地址框 | PASS |
| R12.S6 | apps/browser/e2e/shortcuts.spec.ts::N1 R12.S6 快捷键固定激活标签 | PASS |
| R12.S7 | apps/browser/e2e/shortcuts.spec.ts::N1 R12.S7 快捷键刷新 | PASS |
| R13.S1 | crates/alter-core/tests/store.rs::n1_r13_s1_fresh_open_creates_schema | PASS |
| R13.S2 | crates/alter-core/tests/store.rs::n1_r13_s2_reopen_is_idempotent | PASS |
| R13.S3 | crates/alter-core/tests/store.rs::r1_s3_wal_and_foreign_keys | PASS |
| R13.S4 | crates/alter-core/tests/store.rs::n1_r13_s4_existing_tabs_are_renumbered | PASS |
| R14.S1 | crates/alter-core/tests/store.rs::n1_r14_s1_upgrade_from_v1_writes_backup | PASS |
| R14.S2 | crates/alter-core/tests/store.rs::n1_r14_s2_no_backup_for_fresh_or_current_db | PASS |
| R15.S1 | crates/alterd/tests/lifecycle.rs::n1_r15_s1_run_writes_health_file | PASS |
| R15.S2 | crates/alterd/tests/lifecycle.rs::r3_s2_sigterm_exits_cleanly | PASS |
| R15.S3 | crates/alterd/tests/lifecycle.rs::r3_s3_second_instance_is_rejected | PASS |
| R15.S4 | crates/alterd/tests/lifecycle.rs::r3_s4_stale_health_file_is_taken_over | PASS |
| R16.S1 | crates/alter-core/tests/export_ts.rs::n1_r16_s1_export_error_code_and_health_file | PASS |
| R16.S2 | crates/alter-core/tests/export_ts.rs::n1_r16_s2_export_shell_messages | PASS |

| SC | Check | Result |
|---|---|---|
| SC4 | `bun run e2e --grep SC4` → 20 次切换 p50 14.3 ms，p95 15.9 ms | PASS（< 50 ms） |

运行环境：主会话本机 macOS 对 HEAD `63bf03d`；codex 只读核对测试与代码，并跑了 `dflow validate`、`dflow diagrams check`、`git diff --check`。上一份报告的三个缺口（T36–T38）已确认补上。

Gaps:
- partial：`N1 R8.S4` 在 T41 期间偶发失败一次（重启前「固定后 1 秒内顺序变为 B、A」的准备步骤），之后 13 次与每次全量 e2e 都没有复现，原因未找到，用户尚未决定是否接受
- partial：R12 正文的「选中地址框文字」与菜单 `nav.back` / `nav.forward` 没有测试；R13.S1 没有断言「共 8 张」的总数 → T42
- 说明：localStorage 清除没有直接断言（R5.S5 只要求 Cookie，后续）

Overall: NOT READY

- Revision: 1
- Review: Round 21
- Verifier: codex (gpt-6-sol)

| Repo | Command | Result |
|---|---|---|
| alter | `cargo test` | PASS（cli 4、alter-core 1+3+7、lifecycle 8、shell 52；shell_perf 1 ignored） |
| alter | `bun test` | PASS（1 test） |
| alter | `cargo fmt --check && cargo clippy --all-targets -- -D warnings && bun run lint` | PASS |
| alter | `bun run typecheck` | PASS |
| alter | `cargo build && bun run build` | PASS |
| alter | `bun run e2e` | PASS（51 tests，T44 之后在同一代码上连跑 2 次各 51 passed） |

| Scenario | Test | Result |
|---|---|---|
| R1.S1 | crates/alterd/tests/shell.rs::n1_r1_s1_created_space_becomes_active | PASS |
| R1.S2 | crates/alterd/tests/shell.rs::n1_r1_s2_blank_name_is_rejected | PASS |
| R1.S3 | crates/alterd/tests/shell.rs::n1_r1_s3_name_longer_than_32_is_rejected | PASS |
| R1.S4 | crates/alterd/tests/shell.rs::n1_r1_s4_unknown_color_is_rejected | PASS |
| R1.S5 | apps/browser/e2e/spaces.spec.ts::N1 R1.S5 在侧边栏新建 Space | PASS |
| R2.S1 | crates/alterd/tests/shell.rs::n1_r2_s1_rename_keeps_id_and_active_space | PASS |
| R2.S2 | crates/alterd/tests/shell.rs::n1_r2_s2_update_unknown_space_is_rejected | PASS |
| R2.S3 | apps/browser/e2e/spaces.spec.ts::N1 R2.S3 在侧边栏改名 | PASS |
| R3.S1 | crates/alterd/tests/shell.rs::n1_r3_s1_activate_changes_active_space | PASS |
| R3.S2 | crates/alterd/tests/shell.rs::n1_r3_s2_activate_unknown_space_is_rejected | PASS |
| R3.S3 | apps/browser/e2e/spaces.spec.ts::N1 R3.S3 每个 Space 记住自己的激活标签 | PASS |
| R4.S1 | crates/alterd/tests/shell.rs::n1_r4_s1_restart_keeps_active_space | PASS |
| R4.S2 | crates/alterd/tests/shell.rs::n1_r4_s2_legacy_db_activates_first_space | PASS |
| R5.S1 | crates/alterd/tests/shell.rs::n1_r5_s1_delete_active_middle_space_activates_next | PASS |
| R5.S2 | crates/alterd/tests/shell.rs::n1_r5_s2_delete_active_last_space_activates_previous | PASS |
| R5.S3 | crates/alterd/tests/shell.rs::n1_r5_s3_delete_inactive_space_keeps_active | PASS |
| R5.S4 | crates/alterd/tests/shell.rs::n1_r5_s4_last_space_cannot_be_deleted | PASS |
| R5.S5 | apps/browser/e2e/spaces.spec.ts::N1 R5.S5 在侧边栏删除 Space 并清掉 Cookie | PASS |
| R5.S6 | apps/browser/e2e/spaces.spec.ts::N1 R5.S6 取消确认不删除 | PASS |
| R6.S1 | crates/alterd/tests/shell.rs::n1_r6_s1_startup_sweeps_orphan_partitions | PASS |
| R7.S1 | apps/browser/e2e/spaces.spec.ts::N1 R7.S1 另一个 Space 看不到 Cookie | PASS |
| R8.S1 | crates/alterd/tests/shell.rs::n1_r8_s1_pin_moves_tab_to_pinned_section | PASS |
| R8.S2 | crates/alterd/tests/shell.rs::n1_r8_s2_unpin_moves_tab_to_head_of_normal_section | PASS |
| R8.S3 | crates/alterd/tests/shell.rs::n1_r8_s3_pin_unknown_tab_is_rejected | PASS |
| R8.S4 | apps/browser/e2e/pinned.spec.ts::N1 R8.S4 在侧边栏固定并在重启后保持 | PASS |
| R9.S1 | crates/alterd/tests/shell.rs::n1_r9_s1_close_active_middle_tab_activates_next | PASS |
| R9.S2 | apps/browser/e2e/sidebar.spec.ts::N1 R9.S2 关闭最后一个标签 | PASS |
| R10.S1 | crates/alterd/tests/shell.rs::n1_r10_s1_closing_pinned_tab_discards_it | PASS |
| R10.S2 | apps/browser/e2e/pinned.spec.ts::N1 R10.S2 在侧边栏关闭固定标签后再打开 | PASS |
| R11.S1 | crates/alterd/tests/shell.rs::n1_r11_s1_move_within_normal_section | PASS |
| R11.S2 | crates/alterd/tests/shell.rs::n1_r11_s2_move_into_pinned_section_pins | PASS |
| R11.S3 | crates/alterd/tests/shell.rs::n1_r11_s3_index_out_of_range_moves_to_end | PASS |
| R11.S4 | crates/alterd/tests/shell.rs::n1_r11_s4_move_unknown_tab_is_rejected | PASS |
| R11.S5 | apps/browser/e2e/pinned.spec.ts::N1 R11.S5 在侧边栏拖动并在重启后保持 | PASS |
| R12.S1 | apps/browser/e2e/shortcuts.spec.ts::N1 R12.S1 菜单项与快捷键 | PASS |
| R12.S2 | apps/browser/e2e/shortcuts.spec.ts::N1 R12.S2 关闭激活标签 | PASS |
| R12.S3 | apps/browser/e2e/shortcuts.spec.ts::N1 R12.S3 按序号切换 Space | PASS |
| R12.S4 | apps/browser/e2e/shortcuts.spec.ts::N1 R12.S4 循环切换标签 | PASS |
| R12.S5 | apps/browser/e2e/shortcuts.spec.ts::N1 R12.S5 新标签与聚焦地址框 | PASS |
| R12.S6 | apps/browser/e2e/shortcuts.spec.ts::N1 R12.S6 快捷键固定激活标签 | PASS |
| R12.S7 | apps/browser/e2e/shortcuts.spec.ts::N1 R12.S7 快捷键刷新 | PASS |
| R13.S1 | crates/alter-core/tests/store.rs::n1_r13_s1_fresh_open_creates_schema | PASS |
| R13.S2 | crates/alter-core/tests/store.rs::n1_r13_s2_reopen_is_idempotent | PASS |
| R13.S3 | crates/alter-core/tests/store.rs::r1_s3_wal_and_foreign_keys | PASS |
| R13.S4 | crates/alter-core/tests/store.rs::n1_r13_s4_existing_tabs_are_renumbered | PASS |
| R14.S1 | crates/alter-core/tests/store.rs::n1_r14_s1_upgrade_from_v1_writes_backup | PASS |
| R14.S2 | crates/alter-core/tests/store.rs::n1_r14_s2_no_backup_for_fresh_or_current_db | PASS |
| R15.S1 | crates/alterd/tests/lifecycle.rs::n1_r15_s1_run_writes_health_file | PASS |
| R15.S2 | crates/alterd/tests/lifecycle.rs::r3_s2_sigterm_exits_cleanly | PASS |
| R15.S3 | crates/alterd/tests/lifecycle.rs::r3_s3_second_instance_is_rejected | PASS |
| R15.S4 | crates/alterd/tests/lifecycle.rs::r3_s4_stale_health_file_is_taken_over | PASS |
| R16.S1 | crates/alter-core/tests/export_ts.rs::n1_r16_s1_export_error_code_and_health_file | PASS |
| R16.S2 | crates/alter-core/tests/export_ts.rs::n1_r16_s2_export_shell_messages | PASS |

| SC | Check | Result |
|---|---|---|
| SC4 | `bun run e2e --grep SC4` → 20 次切换 p50 14–15 ms，p95 15.8–17.1 ms（最近几次全量运行） | PASS（< 50 ms） |

运行环境：主会话本机 macOS（Darwin 25.6.0），非 e2e 检查对 HEAD `29c9645`，e2e 对同一代码（之后只改了文档）。codex 在第二次验证里逐行核对了 53 个场景的测试与需求、设计的收敛（结论里只剩下面的缺口），T42–T44 的补测与竞态修复由第 20、21 轮评审确认。

Gaps: none.

已接受（用户决定，2026-10-21）：
- `N1 R8.S4` 在 T41 期间偶发失败一次（准备步骤），定向 13 次与之后每次全量 e2e 都没有复现，原因未找到；后续给 e2e 加失败时保留 trace。
- 已存档用例 `R7.S1 启动拉起 alterd` 在高负载（负载平均值约 10、Microsoft Defender 占用较高）时两次超过 3 秒，均为全量运行的第一条用例；单独运行与重新构建后约 0.5–0.8 s，之后两次全量 e2e 都通过。按环境负载接受，3 秒时限不变；后续在 CI 上观察。

说明：R4.S1/S2、R15.S1 等场景的 RED 是实现后用变异取得（tdd.md 允许，执行日志已注明）；快捷键只经菜单项点击测试；localStorage 清除没有直接断言（R5.S5 只要求 Cookie）。

Overall: READY
