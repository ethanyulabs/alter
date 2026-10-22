---
change: add-core-features
---

# 执行日志：命令栏、分屏、Peek 与自动归档

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

### T1 协议：搜索、分屏、恢复事件与 `shell.searchResults`，快照的 `splitGroup` 与 `archived`
- Revision: 1
- RED: `cargo test -p alter-core --test export_ts` → `n4_r12_s2_export_shell_messages` FAIL（export_ts.rs:128，`ShellCommand.ts` 缺 `shell.searchResults`）(R12.S2)
- 实现：`ShellEvent` 加 `searchRequested {requestId, query}`、`splitRequested {tabIds}`、`splitDissolved {tabId}`、`tabRestored {tabId}`；`ShellCommand` 加 `searchResults {requestId, items}`；`SearchItem`、`SearchKind`、`ArchivedTab`；`TabView.splitGroup: string | null`、`ShellState.archived`；alterd 对新事件暂回 `E_INVALID_ARGUMENT`、快照的新字段暂为空（T2–T5 实现）；Main 的快照对象带上 `archived`；`bun run protocol:export`；测试 `n3_r9_*` 改名 `n4_r12_*`
- GREEN: `cargo test -p alter-core --test export_ts` → 4 passed (R12.S1, R12.S2, R12.S3)
- RED (mutation: `HealthFile` 多出字段 `port`): `cargo test -p alter-core --test export_ts n4_r12_s1` → FAILED (R12.S1)
- RED (mutation: `OwnershipChanged` 多出字段 `at`): `cargo test -p alter-core --test export_ts n4_r12_s3` → FAILED (R12.S3)
- GREEN: 恢复后 4 passed (R12.S1, R12.S3)
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun test`、`bun run lint`、`bun run typecheck` 通过；`bun run e2e` → 83 passed。运行环境：本机 macOS，主会话

### T2 历史记录
- Revision: 1
- RED: `cargo test -p alterd --test shell n4_r1_` → FAIL：`left: []`（回写不写历史）(R1.S1)
- 实现：新文件 `crates/alter-core/src/store/history.rs`：`add_history`（`visited_at` 取 now 与该 Space 最新一条 + 1 中较大的，保持顺序且不撞唯一索引）、`update_history_title`（同 Space、同 URL 最新一条）；`Store::tab(id)`；`TabService` 的 `TabUpdated` 分支在更新标签后写历史：带 url 追加（标题取标签当前标题），只带标题时更新最新一条；`about:blank` 不记
- GREEN: `cargo test -p alterd --test shell n4_r1_` → 1 passed (R1.S1)
- 补测：`n4_title_update_touches_only_latest_entry`（标题回写只改同 URL 最新一条）——变异「标题更新到整个 Space」时 R1.S1 仍通过（写标题时只有一条记录），所以另加这条
- RED (mutation: 标题更新同 URL 的全部记录): `cargo test -p alterd --test shell n4_` → 1 failed
- RED (mutation: 带 url 的回写不追加历史): `cargo test -p alterd --test shell n4_r1_` → FAILED (R1.S1)
- GREEN: 恢复后 `cargo test -p alterd --test shell` → 54 passed (R1.S1)
- 插曲：撤第二个变异时误用 `git checkout -- crates/alterd/src/tabs/mod.rs`，把本任务尚未提交的两处改动一并撤掉，随即重新加回并重跑全部 shell 测试（54 passed）；之后撤变异只用备份文件
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun test`、`bun run lint`、`bun run typecheck` 通过；`bun run e2e` → 83 passed（SC4 p95 14.1 ms）。运行环境：本机 macOS，主会话

### T3 搜索
- Revision: 1
- RED: `cargo test -p alterd --test shell n4_r3_` → 4 条 FAIL（`searchRequested` 被拒绝，收不到 `shell.searchResults`）(R3.S1, R3.S2, R3.S3, R3.S4)
- 实现：`tabs/search.rs`：`trim()` 后为空回空数组；标签（标题或 URL 不区分大小写包含，当前 Space 优先，其余按 Space 与 position）、Space（名称）、历史（`Store::search_history`：`LIKE` 转义 `%`、`_`、`\`，按 URL 分组取最近一次，跳过已打开的 URL），合计截到 8 项，最后附 `open` 项（`input` 为去掉首尾空白的输入）；只回请求方
- GREEN: `cargo test -p alterd --test shell n4_r3_` → 4 passed (R3.S1, R3.S2, R3.S3, R3.S4)
- RED (mutation: 不截到 8 项): `cargo test -p alterd --test shell n4_r3_s3` → FAILED (R3.S3)
- RED (mutation: 不排除已打开的 URL): `cargo test -p alterd --test shell n4_r3_s2` → FAILED (R3.S2)
- RED (mutation: 输入不去首尾空白): `cargo test -p alterd --test shell n4_r3_s4` → FAILED (R3.S4)
- GREEN: 恢复后通过 (R3.S2, R3.S3, R3.S4)
- 10 万行历史的实测（design.md「风险」）：`cargo test -p alterd --release --test shell n4_search_perf -- --ignored --nocapture`。第一版用逐行相关子查询取每个 URL 的最新一条，1 秒内没有返回；改为 `GROUP BY url` + `MAX(visited_at)` 一次扫描后：「site42」11.0 ms、「page 99999」10.7 ms、「nothing-matches-this」7.5 ms、「of site 1」13.5 ms（含 Shell 往返），低于 50 ms，不需要 FTS5
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS（含 history 的转义单测）；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 通过；`bun run e2e` → 83 passed。运行环境：本机 macOS，主会话

### T4 分屏的状态
- Revision: 1
- RED: `cargo test -p alterd --test shell n4_r6_` → R6.S1 FAIL（`splitRequested` 被占位分支拒绝，收不到快照）；`n4_r7_` → R7.S1、R7.S2 FAIL（同上）；R6.S2、R6.S3 在占位分支下已回 `E_INVALID_ARGUMENT`，用下面的变异取得 RED (R6.S1, R7.S1, R7.S2)
- 实现：`TabRow.split_group`（`list_open_tabs`、`tab` 读出），快照的 `splitGroup` 取它；store `split(left, right)`（先解散两者原有的分屏，再都写成 left）、`dissolve_split(tab)`；`splitRequested`：恰好两个、不同、都存在、同一 Space，否则 `E_INVALID_ARGUMENT` / `E_TAB_NOT_FOUND`；`splitDissolved`；`tabClosed` 先解散所在分屏
- GREEN: `cargo test -p alterd --test shell` → 63 passed (R6.S1, R6.S2, R6.S3, R7.S1, R7.S2)
- RED (mutation: 不检查是否同一 Space): `cargo test -p alterd --test shell n4_r6_s2` → FAILED (R6.S2)
- RED (mutation: 不检查是否恰好两个): `cargo test -p alterd --test shell n4_r6_s3` → FAILED (R6.S3)
- RED (mutation: 关闭标签不解散分屏): `cargo test -p alterd --test shell n4_r7_s1` → FAILED (R7.S1)
- RED (mutation: 解散只清请求的那个标签): `cargo test -p alterd --test shell n4_r7_s2` → FAILED (R7.S2)
- GREEN: 恢复后通过 (R6.S2, R6.S3, R7.S1, R7.S2)
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 通过；`bun run e2e` → 83 passed。运行环境：本机 macOS，主会话

### T5 自动归档、已归档列表与恢复
- Revision: 1
- RED: `cargo test -p alterd --test shell n4_r8_` → FAIL（2 秒内没有归档后的快照）；`n4_r9_` → R9.S1 FAIL（`archived` 里没有 A）、R9.S2 FAIL（`tabRestored` 被拒绝）、R9.S3 FAIL（集合不符）(R8.S1, R9.S1, R9.S2, R9.S3)
- 实现：`Config::archive_after()`（`ALTER_ARCHIVE_AFTER_MS` → `archive_after_hours` → 12 小时，`alter_core::config` 由 N3 建立，这里加字段的读取）；store `archive_stale`（design.md 的条件：非固定、早于阈值、不是所在 Space 的激活标签、未被托管；解散所在分屏并重新编号）、`list_archived`（每个 Space 至多 50 个，按归档时间降序，`ROW_NUMBER()` 窗口）、`restore_tab`（清归档、放到普通分区末尾、激活）；新模块 `alterd::archive`（周期 `min(阈值 / 3, 60 s)` 的定时任务）；`tabRestored` 不存在或未归档回 `E_TAB_NOT_FOUND`；快照的 `archived`
- GREEN: `cargo test -p alterd --test shell` → 67 passed (R8.S1, R9.S1, R9.S2, R9.S3)
- RED (mutation: 固定标签也归档): `cargo test -p alterd --test shell n4_r8_` → FAILED (R8.S1)
- RED (mutation: 激活标签也归档): `cargo test -p alterd --test shell n4_r8_` → FAILED (R8.S1)
- RED (mutation: 被托管的标签也归档): `cargo test -p alterd --test shell n4_r8_` → FAILED (R8.S1)
- RED (mutation: 已归档列表不限 50 个): `cargo test -p alterd --test shell n4_r9_s3` → FAILED (R9.S3)
- RED (mutation: 恢复到普通分区开头): `cargo test -p alterd --test shell n4_r9_s2` → FAILED (R9.S2)
- GREEN: 恢复后通过 (R8.S1, R9.S2, R9.S3)
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run typecheck` 通过；`bun run e2e` → 83 passed（SC4 p95 15.1 ms）。运行环境：本机 macOS，主会话

### T6 命令栏 UI，`tab.new` 改为打开命令栏
- Revision: 1
- BASELINE: `shortcuts.spec.ts` 的 N1 R12.S1–S7 改标题为 `N4 R5.S1–S7` 后，`bun run e2e --grep "N4 R5\."` → 7 passed on unchanged code (R5.S1, R5.S2, R5.S3, R5.S4, R5.S6, R5.S7)
- RED: `bun run e2e --grep "N4 R[24]\."` → 4 条 FAIL：R2.S1、R4.S1、R4.S2、R4.S3 都等不到 `command-bar` / `command-input`（新测试文件 `apps/browser/e2e/command-bar.spec.ts`）(R2.S1, R4.S1, R4.S2, R4.S3)
- 说明：任务预计 R5.S5 先失败，实际在旧代码上通过——N1 的 `tab.new` 进入地址框的新标签模式，同样「键入地址回车开新标签」；改为命令栏后它经命令栏的「打开」项成立，用下面的变异取得 RED
- 实现：IPC `commandBar.open / close / search / results / execute`（`shared/shell.ts`、preload）；Main：`tab.new` → `openCommandBar`（`TabViews.setSuppressed(true)` 摘下页面视图，焦点交给侧边栏的 webContents），`commandBar.search` 转为 `shell.event.searchRequested`，`shell.searchResults` 转给 Renderer，`commandBar.execute` / `close` 恢复挂载（`setSuppressed(false)` 按最近的快照重新 apply），执行：标签项先切 Space 再激活、Space 项切 Space、历史与「打开」项在当前 Space `openRequested`；Renderer `CommandBar.tsx`（`command-bar`、`command-input`、`command-item` + `data-kind` + `aria-selected`；↑↓ 选择、回车执行、Esc 关闭、点遮罩关闭；每次输入一个递增的 `requestId`，丢弃过期结果；结果没回来就回车时等结果到了执行第一项）；`@alter/protocol` 再导出 `SearchItem`、`SearchKind`、`ArchivedTab`
- GREEN: `bun run e2e --grep "N4 R[245]\."` → 11 passed (R2.S1, R4.S1, R4.S2, R4.S3, R5.S1, R5.S2, R5.S3, R5.S4, R5.S5, R5.S6, R5.S7)
- RED (mutation: 打开命令栏时不摘下页面视图): `bun run e2e --grep "N4 R2\.S1"` → FAIL (R2.S1)
- RED (mutation: 执行标签项时不切 Space): `bun run e2e --grep "N4 R4\.S1"` → FAIL (R4.S1)
- RED (mutation: 历史项用标题而不是地址打开): `bun run e2e --grep "N4 R4\.S3"` → FAIL (R4.S3)
- RED (mutation: `tab.new` 什么也不做): `bun run e2e --grep "N4 R5\.S5"` → FAIL (R5.S5)
- RED (mutation: `tab.close` 的快捷键改成 `CmdOrCtrl+Shift+W`): `bun run e2e --grep "N4 R5\.S1"` → FAIL (R5.S1)
- RED (mutation: `tab.close` 什么也不做): `bun run e2e --grep "N4 R5\.S2"` → FAIL (R5.S2)
- RED (mutation: `space.N` 切到第 N+1 个): `bun run e2e --grep "N4 R5\.S3"` → FAIL (R5.S3)
- RED (mutation: `tab.next` 什么也不做): `bun run e2e --grep "N4 R5\.S4"` → FAIL (R5.S4)
- RED (mutation: `tab.togglePin` 什么也不做): `bun run e2e --grep "N4 R5\.S6"` → FAIL (R5.S6)
- RED (mutation: `nav.reload` 什么也不做): `bun run e2e --grep "N4 R5\.S7"` → FAIL (R5.S7)
- GREEN: 恢复后 `bun run e2e --grep "N4 R5\\."` → 7 passed，其余通过 (R2.S1, R4.S1, R4.S3, R5.S1, R5.S2, R5.S3, R5.S4, R5.S5, R5.S6, R5.S7)
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS；`bun test`、`bun run lint`、`bun run typecheck` 通过；`bun run e2e` → 87 passed（SC4 p95 15.4 ms）。运行环境：本机 macOS，主会话

### T7 分屏布局 UI
- Revision: 1
- RED: `bun run e2e --grep "N4 R6\.S4"` → FAIL：`locator.click: Timeout 5000ms exceeded`（侧边栏没有右键菜单 `tab-menu-split`；新测试文件 `apps/browser/e2e/split.spec.ts`）(R6.S4)
- 实现：Renderer 行的右键菜单 `tab-menu`（「与当前标签分屏」`tab-menu-split`，非激活行可见；「取消分屏」`tab-menu-unsplit`，分屏中的行可见；位置限制在侧边栏以内；样式同 Space 面板的弹层）；IPC `tab.split` / `tab.unsplit`，Main 发 `splitRequested [激活标签, 该标签]` / `splitDissolved`；`TabViews` 记分屏伙伴与左边标签，`apply` 在激活标签有 `splitGroup` 时并排挂载同组的另一个标签，`layout` 左右各半（窗口 resize 时重新计算）
- GREEN: `bun run e2e --grep "N4 R6\.S4"` → 1 passed（分屏后与以同一 `ALTER_HOME` 重启后都是左 /b、右 /a、宽度差 ≤ 1 px）(R6.S4)
- RED (mutation: 左右对调): `bun run e2e --grep "N4 R6\.S4"` → FAIL (R6.S4)
- RED (mutation: 不挂载分屏伙伴): `bun run e2e --grep "N4 R6\.S4"` → FAIL (R6.S4)
- GREEN: 恢复后通过 (R6.S4)
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS；`bun run lint`、`bun run typecheck` 通过；`bun run e2e` → 88 passed（SC4 p95 15.3 ms）。运行环境：本机 macOS，主会话

### T8 已归档列表 UI
- Revision: 1
- RED: `bun run e2e --grep "N4 R9\.S4"` → FAIL：`locator.click: Timeout 5000ms exceeded`（没有 `archived-toggle`；新测试文件 `apps/browser/e2e/archive.spec.ts`）(R9.S4)
- 实现：侧边栏底部「已归档」按钮 `archived-toggle`，展开 `ArchivedList.tsx`（当前 Space 的 `archived`，项 `archived-item`，点击恢复）；IPC `tab.restore`，Main 转为 `shell.event.tabRestored`
- GREEN: `bun run e2e --grep "N4 R9\.S4"` → 1 passed (R9.S4)
- RED (mutation: 点击已归档项不恢复): `bun run e2e --grep "N4 R9\.S4"` → FAIL (R9.S4)
- GREEN: 恢复后通过 (R9.S4)
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS；`bun run lint`、`bun run typecheck` 通过；`bun run e2e` → 89 passed。运行环境：本机 macOS，主会话

### T9 Peek，以及 iframe 导航不写历史的端到端验证
- Revision: 1
- RED: `bun run e2e --grep "N4 R(10|11)\."` → 3 条 FAIL：R10.S1 `{"p":[],"rows":2}`（Shift+点击照常开了新标签）、R11.S1 / R11.S2 等不到 Peek 显示 /b（新测试文件 `apps/browser/e2e/peek.spec.ts`）(R10.S1, R11.S1, R11.S2)
- 实现：`TabViews` 的窗口打开处理器在 `disposition === "new-window"` 且没有窗口特性时（Shift+点击）交给 Peek（agent 页面除外，带特性的 `window.open` 仍转为标签）；暂不挂载开关改为按原因（命令栏、Peek）计数；新文件 `apps/browser/src/main/peek.ts`：同 partition 的 WebContentsView，居中、内容区的 80%，打开期间摘下页面视图，页面里或 Renderer 里按 Esc 关闭，「在新标签打开」在原 Space `openRequested` 当前 URL 后关闭，Peek 视图记在 `globalThis.__alterPeekViews`（WeakSet）供测试识别；IPC `peek.state / close / promote`；Renderer `PeekChrome.tsx`（遮罩点击关闭、`peek-promote`、`peek-close`）；新测试文件 `apps/browser/e2e/history.spec.ts`
- GREEN: `bun run e2e --grep "N4 R(10|11)\.|N4 R1\.S2"` → 4 passed (R1.S2, R10.S1, R11.S1, R11.S2)
- 说明：R1.S2 依赖的「只认主框架的页内导航」在 PR #3 就已实现，在本任务改动前同样成立，用下面的变异取得 RED
- RED (mutation: iframe 的页内导航也回写 URL): `bun run e2e --grep "N4 R1\.S2"` → FAIL (R1.S2)
- RED (mutation: Peek 占 90%): `bun run e2e --grep "N4 R10\.S1"` → FAIL (R10.S1)
- RED (mutation: Renderer 里按 Esc 不关闭): `bun run e2e --grep "N4 R11\.S1"` → FAIL (R11.S1)
- RED (mutation: 「在新标签打开」不开标签): `bun run e2e --grep "N4 R11\.S2"` → FAIL (R11.S2)
- GREEN: 恢复后通过 (R1.S2, R10.S1, R11.S1, R11.S2)
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS；`cargo fmt --check`、`bun test`、`bun run lint`、`bun run typecheck` 通过；`bun run e2e` → 93 passed（SC4 p95 15.7 ms）。运行环境：本机 macOS，主会话

### T10 打开命令栏前先关闭 Peek（CR1）
- Revision: 1
- RED: `bun run e2e --grep "N4 CR1 "` → FAIL：`{"p":[{"url":".../b",…}],"views":1,"focused":true}`（Peek 视图仍挂着，盖住命令栏）
- 实现：`openCommandBar` 先 `peek.close()`
- GREEN: `bun run e2e --grep "N4 CR1 "` → 1 passed

### T11 切换或删除 Space 时关闭 Peek（CR2）
- Revision: 1
- RED: `bun run e2e --grep "N4 CR2 "` → FAIL：切到新建的 Work 后 Peek 仍挂着、/b 的 webContents 还在
- 实现：`Peek.sync(state)`：当前 Space 不是 Peek 所在的 Space、或那个 Space 已不在快照里时关闭；Main 在 `tabViews.apply` 之后调用（恢复挂载用的是新快照）
- GREEN: `bun run e2e --grep "N4 CR2 "` → 1 passed

### T12 `close_tab` 在同一事务里解散分屏（CR3）
- Revision: 1
- RED: `cargo test -p alterd --test gateway n4_cr3_` → FAIL（agent 经 `page.close` 关掉分屏里的 G 后，A 的 splitGroup 仍不为 null）
- 实现：`Store::close_tab` 的事务里先解散所关闭标签的分屏（Shell 与 Gateway 的关闭都走这里，固定标签的卸载也一样）；TabService 的 `tabClosed` 去掉单独的解散调用
- GREEN: `cargo test -p alterd --test gateway` → 44 passed；`cargo test -p alterd --test shell` → 69 passed（R7.S1 仍通过）
- RED (mutation: `close_tab` 不解散分屏): `cargo test -p alterd --test gateway n4_cr3_` → FAILED
- GREEN: 恢复后通过

### T13 标题回写只更新该标签最近一次导航的历史行（CR4）
- Revision: 1
- RED: `cargo test -p alterd --test shell n4_cr4_` → FAIL（两个标签先后访问同一 URL，t1 的标题写到了 t2 的那一行）
- 实现：`add_history` 返回新行 id；TabService 记标签 → 最近一次导航写入的历史行，标题回写按 id 更新（alterd 重启后、该标签下一次导航之前的标题回写不写历史）；`update_history_title` 改为按 id
- GREEN: `cargo test -p alterd --test shell n4_` → 全部通过（含 R1.S1、`n4_title_update_touches_only_latest_entry`）
- RED (mutation: 按 id 更新改成更新最新的一行): `cargo test -p alterd --test shell n4_cr4_` → FAILED
- GREEN: 恢复后通过

### T14 历史搜索按 Unicode 小写折叠后字面包含（CR5）
- Revision: 1
- RED: `cargo test -p alterd --test shell n4_cr5_` → FAIL（`ärger` 搜不到标题为 `Ärger im Büro` 的历史）
- 实现：rusqlite 打开 `functions` feature（已有依赖的特性，不是新依赖），`Store::open` 注册 `alter_fold(text)`（Rust `to_lowercase`）；`search_history` 用 `instr(alter_fold(…), 折叠后的输入)`，与标签的匹配规则一致，`%`、`_` 不再是通配符，去掉 LIKE 转义函数与它的单测
- GREEN: `cargo test -p alterd --test shell n4_cr5_` → 1 passed
- RED (mutation: 不折叠大小写): `cargo test -p alterd --test shell n4_cr5_` → FAILED
- GREEN: 恢复后通过
- 10 万行历史复测：`cargo test -p alterd --release --test shell n4_search_perf -- --ignored --nocapture` → 23.9 / 22.7 / 19.9 / 26.5 ms，仍低于 50 ms

### T15 R5 与 R8.S1 的计时（CR6）
- Revision: 1
- 实现：R5.S3（space.1）、R5.S4（tab.next、tab.prev）、R5.S5（address.focus，按 1 秒）、R5.S7 改为点击之前取 `now()`，用 `expectAllWithin` 在时限内检查；R5.S3 的「space.3 后 1 秒仍为 Personal」在点击返回后至少等 1 秒，原写法保留；R8.S1 等满 1.5 秒时只取已经收到的最新快照检查，不再额外等待
- VERIFY: `bun run e2e --grep "N4 R5\."` → 7 passed；`cargo test -p alterd --test shell n4_r8_` → 1 passed

### T16 R9.S3 按顺序比较（CR7）
- Revision: 1
- 实现：断言 `archived` 里该 Space 的 id 序列等于最后关闭的 50 个、按关闭时间倒序
- VERIFY: `cargo test -p alterd --test shell n4_r9_s3` → 1 passed
- VERIFY (Widen，T10–T16): `cargo test --no-fail-fast` 全部 PASS；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun test`、`bun run lint`、`bun run typecheck` 通过；`bun run e2e` → 95 passed（SC4 p95 14.6 ms，SC3 p95 84.8 ms）。运行环境：本机 macOS，主会话

### T17 design.md 同步标题回写与搜索规则（CR8，文档修正）
- Revision: 1
- 实现：「接口契约」写明标题回写只更新该标签最近一次导航写入的那一行（含重启后的行为），以及标签 / Space / 历史一律 Unicode 小写折叠后字面包含（`alter_fold` + `instr`，N5 加书签用同一规则）；「数据模型」的搜索 SQL、核心流程时序图、风险描述同步（10 万行实测 20–27 ms）
- VERIFY: `dflow.py diagrams sync add-core-features`、`dflow.py validate add-core-features` → ok

### T18 清理 `history_rows`（CR9）
- Revision: 1
- 实现：用户关闭（归档）、自动归档、agent 关闭页面（`close_agent_page`）时移除对应标签；删除 Space 后只保留仍打开的标签
- 说明：映射大小在外部不可见，没有新增测试；由代码确认各路径
- VERIFY (Widen，T17–T18): `cargo test --no-fail-fast` 全部 PASS；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 通过；`bun run e2e` → 95 passed。运行环境：本机 macOS，主会话

### T19 时序图图源同步（CR10，文档修正）
- Revision: 1
- 实现：改图源 `assets/seq-browse-extras.mmd`（「Unicode 折叠后字面包含，最多 8 项」）后 `dflow diagrams sync`；tasks.md 里 T3 子项的 `LIKE` 规则补注「T14 起改为 Unicode 小写折叠后字面包含」
- VERIFY: `grep -rn LIKE` 只剩 design.md 里说明「不用 LIKE」的那句与 tasks.md T3 子项的历史描述；`dflow.py validate add-core-features` → ok

### T20 ER 图注同步标题回写规则（CR11，文档修正）
- Revision: 1
- 实现：图源 `assets/er-browse-extras.mmd` 的 history.title 注释改为「标题回写按该标签最近一次导航的历史行 id 更新」，`dflow diagrams sync`
- VERIFY: design.md 与图源一致；`dflow.py validate add-core-features` → ok

### T21 R1.S2 确认 iframe 导航已发生（验证缺口）
- Revision: 1
- 实现：用例先在主进程里读 `/frame` 页面中 iframe 的 `location.hash`，等到 `#x`（iframe 的页内导航已发生）再从那一刻起等 2 秒查历史
- GREEN: `bun run e2e --grep "N4 R1\.S2"` → 1 passed (R1.S2)
- RED (mutation: iframe 的页内导航也回写 URL，T9 已记录): 同一用例 → FAIL (R1.S2)

### T22 R8.S1 等满 1.5 秒时立即检查（验证缺口）
- Revision: 1
- 实现：等满 1.5 秒后先查数据库（A 已归档），再只取已经到达的快照（每次等 1 ms）检查最新一份里没有 A
- GREEN: `cargo test -p alterd --test shell n4_r8_` → 1 passed (R8.S1)

### T23 命令栏执行 Space 结果项（验证缺口）
- Revision: 1
- 实现：新用例 `N4 V4`：当前为 Work，输入 Personal，选中的是 Space 项，回车后 1 秒内底部为 Personal、命令栏关闭
- GREEN: `bun run e2e --grep "N4 V4 "` → 1 passed
- RED (mutation: 执行 Space 项什么也不做): → FAIL；恢复后通过

### T24 分屏视图断言（验证缺口）
- Revision: 1
- 实现：R6.S4 的并排检查加上「左边 x 为 240、右边紧接左边、两者合起来铺满内容区、各占一半」；新用例 `N4 V5`：激活右边的 Page A 后仍是左 /b 右 /a，「取消分屏」后只挂 /a 且占满内容区
- GREEN: `bun run e2e --grep "N4 (R6\.S4|V5 )"` → 2 passed (R6.S4)
- RED (mutation: 左右跟着激活标签走): `N4 V5` → FAIL；RED (mutation: 按 40/60 分): `N4 R6.S4` → FAIL (R6.S4)
- GREEN: 恢复后通过 (R6.S4)

### T25 Peek 点击浮层外关闭（验证缺口）
- Revision: 1
- 实现：新用例 `N4 V6`：点遮罩左上角（Peek 视图之外），1 秒内不再挂载 Peek
- GREEN: `bun run e2e --grep "N4 V6 "` → 1 passed
- RED (mutation: 点遮罩不关闭): → FAIL；恢复后通过

### T26 `history.rs` 的注释（验证缺口）
- Revision: 1
- 实现：模块注释与 `search_history` 的注释改为「标题回写按该次导航写入的行 id 更新」「Unicode 小写折叠后字面包含」
- VERIFY (Widen，T21–T26): `cargo test --no-fail-fast` 全部 PASS；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun test`、`bun run lint`、`bun run typecheck` 通过；`bun run e2e` → 98 passed（SC4 p95 14.7 ms，SC3 p95 88.4 ms）。运行环境：本机 macOS，主会话

### T27 测试断言收紧（CR12–CR15）
- Revision: 1
- 实现：R8.S1 在睡眠之前记下截止时刻（墙上时钟），断言 A 的 `archived_at` 不晚于它（归档与发布新快照是 TabService 的同一步）；分屏的并排检查加上两个视图 `y = 0`、高度等于内容区，取消分屏后断言 `x = 240`、`y = 0`、宽高铺满；V6 断言遮罩消失、/b 的 webContents 已关闭；V5 初次分屏的计时挪到点击之前；去掉测试文件末尾多余的空行
- GREEN: `cargo test -p alterd --test shell n4_r8_` → 1 passed；`bun run e2e --grep "N4 (R6\.S4|V5 |V6 )"` → 3 passed (R8.S1, R6.S4)
- RED (mutation: 自动归档周期至少 2 秒): `cargo test -p alterd --test shell n4_r8_` → FAILED (R8.S1)
- RED (mutation: 分屏左边视图下移 10 px): `bun run e2e --grep "N4 R6\.S4"` → FAIL (R6.S4)
- RED (mutation: Peek 关闭时不通知 Renderer，遮罩留下): `bun run e2e --grep "N4 V6 "` → FAIL
- GREEN: 恢复后通过 (R8.S1, R6.S4)
- VERIFY (Widen): `cargo test --no-fail-fast` 全部 PASS；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`bun run lint` 通过；`bun run e2e` → 98 passed。运行环境：本机 macOS，主会话

### T28 R8.S1 按快照的实际到达时刻判断（CR16）
- Revision: 1
- 实现：不再睡眠，在 1.5 秒的等待期间持续接收消息并记下每份快照的到达时刻；截止时必须已经收到一份到达时刻不晚于截止、且不含 A 的快照；之后再查数据库 A 已归档，并用那份快照检查 P、B、G 仍在
- GREEN: `cargo test -p alterd --test shell n4_r8_` 连跑 3 次 → 均 1 passed (R8.S1)
- RED (mutation: 自动归档周期至少 2 秒): `cargo test -p alterd --test shell n4_r8_` → FAILED (R8.S1)
- GREEN: 恢复后通过 (R8.S1)
- VERIFY: `cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 通过。运行环境：本机 macOS，主会话

## 决策与偏差
- T9：Peek 打开期间摘下底下的页面视图（Renderer 的遮罩与按钮才能显示，点遮罩才能关闭），与命令栏同一做法；design.md 写的是「挂在页面视图之上」，页面视图会盖住 Renderer，遮罩与按钮画不出来。design.md 同步。
- T6：R4.S2 与 R5.S5 的 WHEN 相同（打开命令栏、键入地址、回车），R5.S2–S4、S6、S7 的行为没有变化，以基线证明；命令栏的「打开」项的 `title` 是去掉首尾空白的输入，界面显示为「打开或搜索：<输入>」。
- T5：已归档列表也列出 agent 关闭的页面（`page.close` 同样是归档），需求的「每个 Space 最近归档的标签」不区分来源；恢复时清掉固定与分屏标记（归档前已解散分屏，固定标签不会被归档）。
- T2：`about:blank` 不记进历史（新开空白页与 agent 页面的初始页不是访问）；需求没提到它，是对「主框架完成导航」的收窄。
- 执行顺序：N4 在 N3 之后做（父级 log.md「决策与偏差」），R12 的事件集合包含 N3 的 `ownershipReturned` / `userTookControl`。
- T1：协议导出场景再次 MODIFIED，`export_ts.rs` 的 `n3_r9_*` 改名为 `n4_r12_*`（同 N2、N3 的做法），N3 的验证报告引用的是改名前的测试名。

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
- Scope: alter `git diff 27a5ffd...HEAD`（N4 的 T1–T9），无未提交改动；reviewer: codex (gpt-6-sol)
- Tasks: T1-T9
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR1 | major | correctness | apps/browser/src/main/index.ts:122 | Peek 打开时按 Cmd+T 只卸下标签视图，Peek 视图仍盖在命令栏上，输入框看不到也点不到（R2、R10） | T10 |
| CR2 | major | security | apps/browser/src/main/peek.ts:30 | Peek 打开后切换 Space，旧 Space 的页面仍覆盖新 Space；删除旧 Space 后 Peek 视图仍持有它的 partition，「在新标签打开」会向已删除的 Space 发请求（R10–R11、父级隐私 NFR） | T11 |
| CR3 | major | correctness | crates/alterd/src/tabs/task_space.rs:238 | agent 的 `page.close` / `finish` 关闭标签走 `close_tab`，不解散分屏，留下的标签永久带着孤立的 `splitGroup`（R7） | T12 |
| CR4 | major | correctness | crates/alter-core/src/store/history.rs:33 | 同 Space 两个标签先后访问同一 URL，较早标签的标题回写会覆盖较晚标签的历史行，较早那条得不到标题（R1.S1） | T13 |
| CR5 | major | correctness | crates/alter-core/src/store/history.rs:56 | 历史搜索用 SQLite `LIKE`，只对 ASCII 忽略大小写：`ärger` 搜不到 `Ärger`，而同一次搜索里 Rust 处理的标签能匹配（R3） | T14 |
| CR6 | minor | tests | apps/browser/e2e/shortcuts.spec.ts:98 | R5.S3、R5.S4、R5.S5 的地址框聚焦、R5.S7 在动作返回后才开始计时；R8.S1 等 1.5 秒后又允许最多 2 秒收归档快照，证明不了 1.5 秒时已归档 | T15 |
| CR7 | minor | tests | crates/alterd/tests/shell.rs:1476 | R9.S3 只比较最后 50 个 id 的集合，列表倒序也会通过，设计要求按归档时间降序 | T16 |

Review: CHANGES REQUESTED

### Round 2
- Revision: 1
- Scope: alter `git diff f1ecf8e...HEAD`（收敛任务 T10–T16），无未提交改动；reviewer: codex (gpt-6-sol)。CR1–CR7 确认已修好
- Tasks: T10, T11, T12, T13, T14, T15, T16
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR8 | major | spec | .dev-flow/changes/add-core-features/nodes/browse-extras/design.md:90 | 接口契约仍写标题回写更新「同 Space、同 URL 最新一条」，搜索 SQL 与时序图仍写 `LIKE`；N5 照这份交接文档加书签会重新引入 CR4、CR5 的缺陷 | T17 |
| CR9 | minor | correctness | crates/alterd/src/tabs/mod.rs:587 | `history_rows` 在关闭标签、自动归档、删除 Space 后不清理，长时间运行会随曾访问过的标签数增长 | T18 |

Review: CHANGES REQUESTED

### Round 3
- Revision: 1
- Scope: alter `git diff 39ecb0d...HEAD`（收敛任务 T17、T18），无未提交改动；reviewer: codex (gpt-6-sol)。CR9 确认已修好
- Tasks: T17, T18
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR10 | major | spec | .dev-flow/changes/add-core-features/nodes/browse-extras/design.md:153 | 时序图仍写历史搜索用 `LIKE`：图源 `assets/seq-browse-extras.mmd` 没改，`dflow diagrams sync` 用图源覆盖了 design.md 里的修改；T17 记录与文件内容不符 | T19 |

Review: CHANGES REQUESTED

### Round 4
- Revision: 1
- Scope: alter `git diff a9e1068...HEAD`（收敛任务 T19），无未提交改动；reviewer: codex (gpt-6-sol)。CR10 确认已修好，时序图与图源、实现一致
- Tasks: T19
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR11 | minor | spec | .dev-flow/changes/add-core-features/nodes/browse-extras/design.md:129 | ER 图注仍写「标题回写时更新最近一条」，没限定为该标签最近一次导航的历史行 | T20 |

Review: PASS

### Round 5
- Revision: 1
- Scope: alter `git diff 1e1ca80...HEAD`（收敛任务 T20），无未提交改动；reviewer: codex (gpt-6-sol)。CR11 确认已修好，design.md 的搜索与标题回写契约与实现一致，足以交接 N5
- Tasks: T20
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|

Review: PASS

### Round 6
- Revision: 1
- Scope: alter `git diff 0d609bf...HEAD`（验证缺口的收敛任务 T21–T26），无未提交改动；reviewer: codex (gpt-6-sol)。T21、T23、T26 确认关闭
- Tasks: T21, T22, T23, T24, T25, T26
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR12 | major | tests | crates/alterd/tests/shell.rs:1363 | R8.S1 仍没有限定检查时点：`sleep` 可能晚返回，读快照的循环也可能读到 1.5 秒之后才到的快照 | T27 |
| CR13 | major | tests | apps/browser/e2e/split.spec.ts:38 | 分屏只验证了横向位置与宽度，没验证 y 与高度；取消分屏后只验宽度，视图偏移或只占上半部也会通过 | T27 |
| CR14 | minor | tests | apps/browser/e2e/peek.spec.ts:192 | V6 只查 Peek 视图卸下，遮罩或 /b 的 webContents 残留也会通过 | T27 |
| CR15 | minor | tests | apps/browser/e2e/split.spec.ts:89 | V5 初次分屏的计时在点击返回后才开始 | T27 |

Review: CHANGES REQUESTED

### Round 7
- Revision: 1
- Scope: alter `git diff 71b1c0c...HEAD`（收敛任务 T27），无未提交改动；reviewer: codex (gpt-6-sol)。CR13–CR15 确认关闭
- Tasks: T27
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR16 | major | tests | crates/alterd/tests/shell.rs:1383 | `archived_at <= 截止` 只证明归档时刻，读取循环仍可能接收截止后才发布的快照，没有证明 R8.S1 的快照时限 | T28 |

Review: CHANGES REQUESTED

### Round 8
- Revision: 1
- Scope: alter `git diff 999031e...HEAD`（收敛任务 T28），无未提交改动；reviewer: codex (gpt-6-sol)。CR16 确认关闭
- Tasks: T28
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|

Review: PASS

### Round 9
- Revision: 1
- Scope: alter 当前 HEAD（d0e8299 之后只有文档改动）上这些收敛任务的代码与测试——关闭确认：dev-flow 新版要求收敛任务出现在结论为 PASS 的后续轮次里，发现才算关闭；下列任务当时在下一轮复查过，但那一轮因别的新发现结论为 CHANGES REQUESTED
- Reviewers: claude (opus-5, self-review): 0 findings（核对各任务的 RED / 变异证据）；codex (gpt-5.6-sol): 逐条确认 CR1–CR9、CR12–CR15 已修复、没有回退，0 个新发现
- Tasks: T10, T11, T12, T13, T14, T15, T16, T17, T18, T27
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|

Review: PASS

## 验证报告
<!-- 每次验证追加一份新报告，放在最近一轮评审之后。以 "- Revision: <当前 revision>" 和 "- Review: Round <最近一轮>" 开头。由验证步骤填写：本仓库的测试结果，每个场景一行 "| R<n>.S<m> | <path::test> | PASS |"，成功标准对照表，收敛缺口，fix 类型还要写 "Fix: verified | partial | failed"，最后恰好一行 "Overall: READY" 或 "Overall: NOT READY"。 -->

- Revision: 1
- Review: Round 5
- Verifier: codex (gpt-6-sol)

| Repo | Command | Result |
|---|---|---|
| alter | `cargo test --no-fail-fast` | PASS（shell 69、gateway 44、lifecycle 8、alterd 单测 8、export_ts 4 等；本机 macOS，提交 a9e1068，之后只改文档） |
| alter | `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` | PASS |
| alter | `bun test` | PASS（10） |
| alter | `bun run lint` / `bun run typecheck` | PASS |
| alter | `bun run e2e` | PASS（95 passed） |

| Scenario | Test | Result |
|---|---|---|
| R1.S1 | crates/alterd/tests/shell.rs::n4_r1_s1_navigation_and_title_write_history | PASS |
| R1.S2 | apps/browser/e2e/history.spec.ts::N4 R1.S2 iframe 导航不写历史：2 秒后 history 中只有 /frame 的 1 条记录 | PASS |
| R2.S1 | apps/browser/e2e/command-bar.spec.ts::N4 R2.S1 打开与关闭：tab.new 后命令栏可见、输入框聚焦、不挂载页面；Esc 后恢复显示 /a | PASS |
| R3.S1 | crates/alterd/tests/shell.rs::n4_r3_s1_sources_and_order | PASS |
| R3.S2 | crates/alterd/tests/shell.rs::n4_r3_s2_open_url_not_repeated_in_history | PASS |
| R3.S3 | crates/alterd/tests/shell.rs::n4_r3_s3_at_most_eight | PASS |
| R3.S4 | crates/alterd/tests/shell.rs::n4_r3_s4_blank_query | PASS |
| R4.S1 | apps/browser/e2e/command-bar.spec.ts::N4 R4.S1 切到另一个 Space 的标签：回车后底部为 Personal、Page A 激活、命令栏关闭 | PASS |
| R4.S2 | apps/browser/e2e/command-bar.spec.ts::N4 R4.S2 输入网址打开新标签：2 秒内依次为 Page A、Page B，Page B 激活 | PASS |
| R4.S3 | apps/browser/e2e/command-bar.spec.ts::N4 R4.S3 打开历史项：方向键选中历史项回车，2 秒内出现 Page C 且激活 | PASS |
| R5.S1 | apps/browser/e2e/shortcuts.spec.ts::N4 R5.S1 菜单项与快捷键：每个菜单项都存在且 accelerator 与表中完全相同 | PASS |
| R5.S2 | apps/browser/e2e/shortcuts.spec.ts::N4 R5.S2 关闭激活标签：点 tab.close 后只剩 Page A 且为激活样式 | PASS |
| R5.S3 | apps/browser/e2e/shortcuts.spec.ts::N4 R5.S3 按序号切换 Space：space.1 切到 Personal，space.3 不存在时不变 | PASS |
| R5.S4 | apps/browser/e2e/shortcuts.spec.ts::N4 R5.S4 循环切换标签：Page C 激活时 tab.next 到 Page A，tab.prev 回到 Page C | PASS |
| R5.S5 | apps/browser/e2e/shortcuts.spec.ts::N4 R5.S5 新标签与聚焦地址框：tab.new 后键入地址回车新开 Page B，address.focus 聚焦地址框 | PASS |
| R5.S6 | apps/browser/e2e/shortcuts.spec.ts::N4 R5.S6 快捷键固定激活标签：tab.togglePin 后 Page B 排到最前并在固定分区 | PASS |
| R5.S7 | apps/browser/e2e/shortcuts.spec.ts::N4 R5.S7 快捷键刷新：nav.reload 后 2 秒内 /count 的请求次数从 1 变为 2 | PASS |
| R6.S1 | crates/alterd/tests/shell.rs::n4_r6_s1_split_two_tabs | PASS |
| R6.S2 | crates/alterd/tests/shell.rs::n4_r6_s2_cross_space_rejected | PASS |
| R6.S3 | crates/alterd/tests/shell.rs::n4_r6_s3_needs_exactly_two | PASS |
| R6.S4 | apps/browser/e2e/split.spec.ts::N4 R6.S4 在侧边栏分屏：左 /b 右 /a、宽度相差不超过 1 px，重启后仍并排 | PASS |
| R7.S1 | crates/alterd/tests/shell.rs::n4_r7_s1_close_one_dissolves | PASS |
| R7.S2 | crates/alterd/tests/shell.rs::n4_r7_s2_dissolve | PASS |
| R8.S1 | crates/alterd/tests/shell.rs::n4_r8_s1_archive_only_eligible_tabs | PASS |
| R9.S1 | crates/alterd/tests/shell.rs::n4_r9_s1_closed_tab_in_archived_list | PASS |
| R9.S2 | crates/alterd/tests/shell.rs::n4_r9_s2_restore | PASS |
| R9.S3 | crates/alterd/tests/shell.rs::n4_r9_s3_archived_list_limit | PASS |
| R9.S4 | apps/browser/e2e/archive.spec.ts::N4 R9.S4 在侧边栏恢复：点「已归档」里的 Page A，1 秒内依次为 Page B、Page A 且 Page A 激活 | PASS |
| R10.S1 | apps/browser/e2e/peek.spec.ts::N4 R10.S1 打开 Peek：/b 的 Peek 视图挂在窗口上，宽高为内容区的 80%，标签仍只有 1 行 | PASS |
| R11.S1 | apps/browser/e2e/peek.spec.ts::N4 R11.S1 Esc 关闭：1 秒内不再挂载 Peek，也没有 /b 的 webContents | PASS |
| R11.S2 | apps/browser/e2e/peek.spec.ts::N4 R11.S2 转为标签：2 秒内侧边栏依次为 Links、Page B，Page B 激活，Peek 已关闭 | PASS |
| R12.S1 | crates/alter-core/tests/export_ts.rs::n4_r12_s1_export_error_code_and_health_file | PASS |
| R12.S2 | crates/alter-core/tests/export_ts.rs::n4_r12_s2_export_shell_messages | PASS |
| R12.S3 | crates/alter-core/tests/export_ts.rs::n4_r12_s3_export_gateway_types | PASS |

Gaps:
- partial：R1.S2 没有确认 iframe 的 hash 导航已发生 → N4/T21
- partial：R8.S1 等满 1.5 秒后的取快照循环仍可能收到之后的快照 → N4/T22
- partial：R4 的 Space 结果项没有 e2e → N4/T23
- partial：R6 / R7 的分屏视图只验证了宽度差与初始激活：各占一半、激活另一个仍并排、解散后只显示一个都没有视图断言 → N4/T24
- partial：R11 的「点击浮层外关闭」没有测试 → N4/T25
- contradicts：`crates/alter-core/src/store/history.rs` 的注释仍写旧的标题回写与 ASCII 大小写规则 → N4/T26

Overall: NOT READY

- Revision: 1
- Review: Round 8
- Verifier: codex (gpt-6-sol)

| Repo | Command | Result |
|---|---|---|
| alter | `cargo test --no-fail-fast` | PASS（shell 69、gateway 44、lifecycle 8、alterd 单测 8、export_ts 4 等；本机 macOS，提交 4b86d0d） |
| alter | `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` | PASS |
| alter | `bun test` | PASS（10） |
| alter | `bun run lint` / `bun run typecheck` | PASS |
| alter | `bun run e2e` | PASS（98 passed，提交 999031e，之后只改一条 Rust 测试与文档） |

| Scenario | Test | Result |
|---|---|---|
| R1.S1 | crates/alterd/tests/shell.rs::n4_r1_s1_navigation_and_title_write_history | PASS |
| R1.S2 | apps/browser/e2e/history.spec.ts::N4 R1.S2 iframe 导航不写历史：2 秒后 history 中只有 /frame 的 1 条记录 | PASS |
| R2.S1 | apps/browser/e2e/command-bar.spec.ts::N4 R2.S1 打开与关闭：tab.new 后命令栏可见、输入框聚焦、不挂载页面；Esc 后恢复显示 /a | PASS |
| R3.S1 | crates/alterd/tests/shell.rs::n4_r3_s1_sources_and_order | PASS |
| R3.S2 | crates/alterd/tests/shell.rs::n4_r3_s2_open_url_not_repeated_in_history | PASS |
| R3.S3 | crates/alterd/tests/shell.rs::n4_r3_s3_at_most_eight | PASS |
| R3.S4 | crates/alterd/tests/shell.rs::n4_r3_s4_blank_query | PASS |
| R4.S1 | apps/browser/e2e/command-bar.spec.ts::N4 R4.S1 切到另一个 Space 的标签：回车后底部为 Personal、Page A 激活、命令栏关闭 | PASS |
| R4.S2 | apps/browser/e2e/command-bar.spec.ts::N4 R4.S2 输入网址打开新标签：2 秒内依次为 Page A、Page B，Page B 激活 | PASS |
| R4.S3 | apps/browser/e2e/command-bar.spec.ts::N4 R4.S3 打开历史项：方向键选中历史项回车，2 秒内出现 Page C 且激活 | PASS |
| R5.S1 | apps/browser/e2e/shortcuts.spec.ts::N4 R5.S1 菜单项与快捷键：每个菜单项都存在且 accelerator 与表中完全相同 | PASS |
| R5.S2 | apps/browser/e2e/shortcuts.spec.ts::N4 R5.S2 关闭激活标签：点 tab.close 后只剩 Page A 且为激活样式 | PASS |
| R5.S3 | apps/browser/e2e/shortcuts.spec.ts::N4 R5.S3 按序号切换 Space：space.1 切到 Personal，space.3 不存在时不变 | PASS |
| R5.S4 | apps/browser/e2e/shortcuts.spec.ts::N4 R5.S4 循环切换标签：Page C 激活时 tab.next 到 Page A，tab.prev 回到 Page C | PASS |
| R5.S5 | apps/browser/e2e/shortcuts.spec.ts::N4 R5.S5 新标签与聚焦地址框：tab.new 后键入地址回车新开 Page B，address.focus 聚焦地址框 | PASS |
| R5.S6 | apps/browser/e2e/shortcuts.spec.ts::N4 R5.S6 快捷键固定激活标签：tab.togglePin 后 Page B 排到最前并在固定分区 | PASS |
| R5.S7 | apps/browser/e2e/shortcuts.spec.ts::N4 R5.S7 快捷键刷新：nav.reload 后 2 秒内 /count 的请求次数从 1 变为 2 | PASS |
| R6.S1 | crates/alterd/tests/shell.rs::n4_r6_s1_split_two_tabs | PASS |
| R6.S2 | crates/alterd/tests/shell.rs::n4_r6_s2_cross_space_rejected | PASS |
| R6.S3 | crates/alterd/tests/shell.rs::n4_r6_s3_needs_exactly_two | PASS |
| R6.S4 | apps/browser/e2e/split.spec.ts::N4 R6.S4 在侧边栏分屏：左 /b 右 /a、宽度相差不超过 1 px，重启后仍并排 | PASS |
| R7.S1 | crates/alterd/tests/shell.rs::n4_r7_s1_close_one_dissolves | PASS |
| R7.S2 | crates/alterd/tests/shell.rs::n4_r7_s2_dissolve | PASS |
| R8.S1 | crates/alterd/tests/shell.rs::n4_r8_s1_archive_only_eligible_tabs | PASS |
| R9.S1 | crates/alterd/tests/shell.rs::n4_r9_s1_closed_tab_in_archived_list | PASS |
| R9.S2 | crates/alterd/tests/shell.rs::n4_r9_s2_restore | PASS |
| R9.S3 | crates/alterd/tests/shell.rs::n4_r9_s3_archived_list_limit | PASS |
| R9.S4 | apps/browser/e2e/archive.spec.ts::N4 R9.S4 在侧边栏恢复：点「已归档」里的 Page A，1 秒内依次为 Page B、Page A 且 Page A 激活 | PASS |
| R10.S1 | apps/browser/e2e/peek.spec.ts::N4 R10.S1 打开 Peek：/b 的 Peek 视图挂在窗口上，宽高为内容区的 80%，标签仍只有 1 行 | PASS |
| R11.S1 | apps/browser/e2e/peek.spec.ts::N4 R11.S1 Esc 关闭：1 秒内不再挂载 Peek，也没有 /b 的 webContents | PASS |
| R11.S2 | apps/browser/e2e/peek.spec.ts::N4 R11.S2 转为标签：2 秒内侧边栏依次为 Links、Page B，Page B 激活，Peek 已关闭 | PASS |
| R12.S1 | crates/alter-core/tests/export_ts.rs::n4_r12_s1_export_error_code_and_health_file | PASS |
| R12.S2 | crates/alter-core/tests/export_ts.rs::n4_r12_s2_export_shell_messages | PASS |
| R12.S3 | crates/alter-core/tests/export_ts.rs::n4_r12_s3_export_gateway_types | PASS |

| SC | Check | Result |
|---|---|---|
| SC3 | `bun run e2e --grep SC3` → p95 84.8 ms | PASS（< 150 ms） |
| SC4 | `bun run e2e --grep SC4` → p95 14.6 ms | PASS（< 50 ms） |

Gaps: none（上一份报告的 6 个缺口由 T21–T28 关闭，评审第 6–8 轮结束于 PASS）。

Overall: READY

- Revision: 1
- Review: Round 9
- Verifier: codex (gpt-5.6-sol)

| Repo | Command | Result |
|---|---|---|
| alter | `cargo build && bun run build` | PASS |
| alter | `cargo test && bun test` | PASS |
| alter | `cargo fmt --check && cargo clippy --all-targets -- -D warnings && bun run lint` | PASS |
| alter | `bun run typecheck` | PASS |
| alter | `bun run e2e` | PASS |

运行环境：提交 9f4fefb，本机 macOS，主会话。`cargo test --no-fail-fast` 172 passed；`bun test` 10 pass；`bun run e2e` 102 passed，结束后没有残留进程。重新验证的原因：dev-flow 新版要求报告排在节点最后一轮评审之后；N4 第 9 轮为关闭确认，没有代码改动。

| Scenario | Test | Result |
|---|---|---|
| R1.S1 | crates/alterd/tests/shell.rs::n4_r1_s1_navigation_and_title_write_history | PASS |
| R1.S2 | apps/browser/e2e/history.spec.ts::N4 R1.S2 iframe 导航不写历史：2 秒后 history 中只有 /frame 的 1 条记录 | PASS |
| R2.S1 | apps/browser/e2e/command-bar.spec.ts::N4 R2.S1 打开与关闭：tab.new 后命令栏可见、输入框聚焦、不挂载页面；Esc 后恢复显示 /a | PASS |
| R3.S1 | crates/alterd/tests/shell.rs::n5_r11_s1_sources_and_order（R3 由 N5 R11 MODIFIED（S1–S4 内容不变，测试改名）） | PASS |
| R3.S2 | crates/alterd/tests/shell.rs::n5_r11_s2_open_url_not_repeated_in_history（R3 由 N5 R11 MODIFIED（S1–S4 内容不变，测试改名）） | PASS |
| R3.S3 | crates/alterd/tests/shell.rs::n5_r11_s3_at_most_eight（R3 由 N5 R11 MODIFIED（S1–S4 内容不变，测试改名）） | PASS |
| R3.S4 | crates/alterd/tests/shell.rs::n5_r11_s4_blank_query（R3 由 N5 R11 MODIFIED（S1–S4 内容不变，测试改名）） | PASS |
| R4.S1 | apps/browser/e2e/command-bar.spec.ts::N4 R4.S1 切到另一个 Space 的标签：回车后底部为 Personal、Page A 激活、命令栏关闭 | PASS |
| R4.S2 | apps/browser/e2e/command-bar.spec.ts::N4 R4.S2 输入网址打开新标签：2 秒内依次为 Page A、Page B，Page B 激活 | PASS |
| R4.S3 | apps/browser/e2e/command-bar.spec.ts::N4 R4.S3 打开历史项：方向键选中历史项回车，2 秒内出现 Page C 且激活 | PASS |
| R5.S1 | apps/browser/e2e/shortcuts.spec.ts::N4 R5.S1 菜单项与快捷键：每个菜单项都存在且 accelerator 与表中完全相同 | PASS |
| R5.S2 | apps/browser/e2e/shortcuts.spec.ts::N4 R5.S2 关闭激活标签：点 tab.close 后只剩 Page A 且为激活样式 | PASS |
| R5.S3 | apps/browser/e2e/shortcuts.spec.ts::N4 R5.S3 按序号切换 Space：space.1 切到 Personal，space.3 不存在时不变 | PASS |
| R5.S4 | apps/browser/e2e/shortcuts.spec.ts::N4 R5.S4 循环切换标签：Page C 激活时 tab.next 到 Page A，tab.prev 回到 Page C | PASS |
| R5.S5 | apps/browser/e2e/shortcuts.spec.ts::N4 R5.S5 新标签与聚焦地址框：tab.new 后键入地址回车新开 Page B，address.focus 聚焦地址框 | PASS |
| R5.S6 | apps/browser/e2e/shortcuts.spec.ts::N4 R5.S6 快捷键固定激活标签：tab.togglePin 后 Page B 排到最前并在固定分区 | PASS |
| R5.S7 | apps/browser/e2e/shortcuts.spec.ts::N4 R5.S7 快捷键刷新：nav.reload 后 2 秒内 /count 的请求次数从 1 变为 2 | PASS |
| R6.S1 | crates/alterd/tests/shell.rs::n4_r6_s1_split_two_tabs | PASS |
| R6.S2 | crates/alterd/tests/shell.rs::n4_r6_s2_cross_space_rejected | PASS |
| R6.S3 | crates/alterd/tests/shell.rs::n4_r6_s3_needs_exactly_two | PASS |
| R6.S4 | apps/browser/e2e/split.spec.ts::N4 R6.S4 在侧边栏分屏：左 /b 右 /a、宽度相差不超过 1 px，重启后仍并排 | PASS |
| R7.S1 | crates/alterd/tests/shell.rs::n4_r7_s1_close_one_dissolves | PASS |
| R7.S2 | crates/alterd/tests/shell.rs::n4_r7_s2_dissolve | PASS |
| R8.S1 | crates/alterd/tests/shell.rs::n4_r8_s1_archive_only_eligible_tabs | PASS |
| R9.S1 | crates/alterd/tests/shell.rs::n4_r9_s1_closed_tab_in_archived_list | PASS |
| R9.S2 | crates/alterd/tests/shell.rs::n4_r9_s2_restore | PASS |
| R9.S3 | crates/alterd/tests/shell.rs::n4_r9_s3_archived_list_limit | PASS |
| R9.S4 | apps/browser/e2e/archive.spec.ts::N4 R9.S4 在侧边栏恢复：点「已归档」里的 Page A，1 秒内依次为 Page B、Page A 且 Page A 激活 | PASS |
| R10.S1 | apps/browser/e2e/peek.spec.ts::N4 R10.S1 打开 Peek：/b 的 Peek 视图挂在窗口上，宽高为内容区的 80%，标签仍只有 1 行 | PASS |
| R11.S1 | apps/browser/e2e/peek.spec.ts::N4 R11.S1 Esc 关闭：1 秒内不再挂载 Peek，也没有 /b 的 webContents | PASS |
| R11.S2 | apps/browser/e2e/peek.spec.ts::N4 R11.S2 转为标签：2 秒内侧边栏依次为 Links、Page B，Page B 激活，Peek 已关闭 | PASS |
| R12.S1 | crates/alter-core/tests/export_ts.rs::n5_r12_s1_export_error_code_and_health_file（R12 已被后续节点按流程顺序 MODIFIED 取代（N5 R12），测试改名并断言取代后的集合） | PASS |
| R12.S2 | crates/alter-core/tests/export_ts.rs::n5_r12_s2_export_shell_messages（R12 已被后续节点按流程顺序 MODIFIED 取代（N5 R12），测试改名并断言取代后的集合） | PASS |
| R12.S3 | crates/alter-core/tests/export_ts.rs::n5_r12_s3_export_gateway_types（R12 已被后续节点按流程顺序 MODIFIED 取代（N5 R12），测试改名并断言取代后的集合） | PASS |

| SC | Check | Result |
|---|---|---|
| SC3 | `bun run e2e --grep SC3` → p95 88.9 ms | PASS（< 150 ms） |
| SC4 | `bun run e2e --grep SC4` → p95 17.8 ms | PASS（< 50 ms） |

Gaps: none。说明：验证人（codex）对本节点的结论是 NOT READY，唯一理由是被后续节点 MODIFIED 的场景（协议导出的精确消息集合等）不再满足本节点原文，且对应测试已改名。主会话判定这不是缺口：dev-flow 的路由规则允许后面的节点 MODIFY 前面节点 ADDED 的需求，存档时按流程顺序合并、以后者为准（FLOW007 提示的就是这种情况）；这些行对应改名后、断言取代后集合的测试。验证人确认本节点其余场景的测试在当前 HEAD 原路径、原名存在，最新评审轮之后没有未关闭的 blocker / major，也没有别的缺口。

Overall: READY
