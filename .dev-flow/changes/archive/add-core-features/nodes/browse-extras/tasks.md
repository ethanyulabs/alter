---
change: add-core-features
---

# 执行文档：命令栏、分屏、Peek 与自动归档

<!-- 行语法：- [ ] T<n> [covers:R1.S1] [after:T<k>] [P] <做什么> — seam: <测试驱动的公开入口> — verify: <命令>
对应的证据写进 log.md 之后才能勾选 [x]。完整语法见 references/formats.md -->

## Repo: alter

- [x] T1 [covers:R12.S1,R12.S2,R12.S3] 协议：搜索、分屏、恢复事件与 `shell.searchResults`，快照的 `splitGroup` 与 `archived` — seam: `export-ts`（`crates/alter-core/tests/export_ts.rs`） — verify: `cargo test -p alter-core --test export_ts`
  - 字段照 design.md「接口契约」；跑 `bun run protocol:export`。
- [x] T2 [covers:R1.S1] [after:T1] 历史记录 — seam: Shell 假客户端（`crates/alterd/tests/shell.rs`） — verify: `cargo test -p alterd --test shell n4_r1_`
  - 新文件 `crates/alter-core/src/store/history.rs`；`TabService::handle` 的 `TabUpdated` 分支在更新标签之后写历史（`url` 有值就追加；`title` 有值就更新同 Space、同 URL 最新一条）。
- [x] T3 [covers:R3.S1,R3.S2,R3.S3,R3.S4] [after:T2] 搜索 — seam: Shell 假客户端（`crates/alterd/tests/shell.rs`） — verify: `cargo test -p alterd --test shell n4_r3_`
  - 匹配规则：`trim()` 后为空返回空数组；否则 `LIKE '%' || q || '%'`，转义 `%`、`_`、`\`（T14 起改为 Unicode 小写折叠后字面包含，见 design.md「接口契约」）；标签按「当前 Space 优先，再按 Space 顺序与 position」排，Space 按 position，历史按最近访问；三类合计截到 8 项，再加 `open` 项。
  - 在本任务里用 10 万行历史测一次搜索耗时，写进 log.md（design.md「风险」）。
- [x] T4 [covers:R6.S1,R6.S2,R6.S3,R7.S1,R7.S2] [after:T1] 分屏的状态 — seam: Shell 假客户端（`crates/alterd/tests/shell.rs`） — verify: `cargo test -p alterd --test shell n4_r6_ n4_r7_`
- [x] T5 [covers:R8.S1,R9.S1,R9.S2,R9.S3] [after:T1] 自动归档、已归档列表与恢复 — seam: 真 alterd + Shell 假客户端（`crates/alterd/tests/shell.rs`） — verify: `cargo test -p alterd --test shell n4_r8_ n4_r9_`
  - 阈值：`ALTER_ARCHIVE_AFTER_MS` 优先，其次 config.toml 的 `archive_after_hours`，默认 12 小时。config.toml 的解析放在 `crates/alter-core/src/config.rs`：N3 先做时已经有了（`handoff_timeout_minutes`），直接加字段；N4 先做时由本任务新建。
  - R8.S1 需要一个被 TaskSpace 托管的标签：经 Gateway `taskSpace.create` 建出 `p1` 即可（N2 已交付）。
  - 恢复的位置：普通分区末尾，复用 N1 的 `renumber`。
- [x] T6 [covers:R2.S1,R4.S1,R4.S2,R4.S3,R5.S1,R5.S2,R5.S3,R5.S4,R5.S5,R5.S6,R5.S7] [after:T3] 命令栏 UI，`tab.new` 改为打开命令栏 — seam: Electron e2e（`apps/browser/e2e/command-bar.spec.ts`、`shortcuts.spec.ts`） — verify: `bun run e2e --grep "N4 R[245]\."`
  - R5.S1–S7 是 MODIFIED 后保留的场景，N1 的 `shortcuts.spec.ts` 已有用例：本任务先把用例标题改成 `N4 R5.Sx …`，跑一遍记为基线（S5 预期会先失败，因为 `tab.new` 的行为变了，这就是它的 RED），实现后全部通过。
  - Renderer `CommandBar.tsx`：`data-testid="command-bar"`、输入框 `command-input`、结果项 `command-item`（`data-kind`）；↑↓ 选择，回车执行，Esc 关闭。
  - Main：打开时摘下当前视图，关闭或执行后按快照重新挂载。
- [x] T7 [covers:R6.S4] [after:T4] 分屏布局 UI — seam: Electron e2e（`apps/browser/e2e/split.spec.ts`） — verify: `bun run e2e --grep "N4 R6\.S4"`
  - `TabViews.apply` 的布局：组内两个视图各占内容区一半；窗口 resize 时重新计算。断言从主进程读 `view.getBounds()`。
  - 行的右键菜单在 Renderer 里画（`data-testid="tab-menu"`，项 `tab-menu-split`），与 N1 的 Space 面板同一套弹层样式。
- [x] T8 [covers:R9.S4] [after:T5] 已归档列表 UI — seam: Electron e2e（`apps/browser/e2e/archive.spec.ts`） — verify: `bun run e2e --grep "N4 R9\.S4"`
  - Sidebar 底部「已归档」按钮（`archived-toggle`），展开 `ArchivedList.tsx`（项 `archived-item`）。
- [x] T9 [covers:R1.S2,R10.S1,R11.S1,R11.S2] [after:T2] Peek，以及 iframe 导航不写历史的端到端验证 — seam: Electron e2e（`apps/browser/e2e/peek.spec.ts`、`history.spec.ts`） — verify: `bun run e2e --grep "N4 R(1\.S2|10|11)\."`
  - Shift+点击：在主进程对页面视图 `webContents.sendInputEvent` 发 `mouseDown` / `mouseUp`，`modifiers: ["shift"]`，坐标取 `Open B` 的位置（经 `executeJavaScript` 读 `getBoundingClientRect`，这是测试代码，不是产品代码）。
  - 新文件 `apps/browser/src/main/peek.ts`；Peek 视图上设置 `data-peek` 标记的办法：主进程维护一个 `WeakSet<WebContentsView>`，测试经 `app.evaluate` 读取。
  - R1.S2 已由 PR #3 的「iframe 页内导航不回写」间接保证，这里补上对 `history` 表的断言。

## Convergence

- [x] T10 [after:T9] 打开命令栏前先关闭 Peek（CR1） — seam: Electron e2e（`apps/browser/e2e/peek.spec.ts`） — verify: `bun run e2e --grep "N4 CR1 "`
- [x] T11 [after:T9] 当前 Space 变化或 Peek 所在 Space 被删除时关闭 Peek（CR2） — seam: Electron e2e（`apps/browser/e2e/peek.spec.ts`） — verify: `bun run e2e --grep "N4 CR2 "`
- [x] T12 [after:T9] `Store::close_tab` 在同一事务里解散所关闭标签的分屏，覆盖 Gateway 的关闭路径（CR3） — seam: Gateway WS + Shell 假客户端（`crates/alterd/tests/gateway.rs`） — verify: `cargo test -p alterd --test gateway n4_cr3_`
- [x] T13 [after:T9] 标题回写只更新该标签最近一次导航写入的那一行历史（CR4） — seam: Shell 假客户端（`crates/alterd/tests/shell.rs`） — verify: `cargo test -p alterd --test shell n4_`
- [x] T14 [after:T9] 历史搜索的大小写匹配与标签一致（Unicode 小写折叠，字面包含）（CR5） — seam: Shell 假客户端（`crates/alterd/tests/shell.rs`） — verify: `cargo test -p alterd --test shell n4_`
- [x] T15 [after:T9] R5.S3、R5.S4、R5.S5（地址框聚焦）、R5.S7 的时限从触发之前计；R8.S1 在等满 1.5 秒时检查已经收到的最新快照与数据库（CR6） — seam: Electron e2e（`apps/browser/e2e/shortcuts.spec.ts`）；Shell 假客户端（`crates/alterd/tests/shell.rs`） — verify: `bun run e2e --grep "N4 R5\." && cargo test -p alterd --test shell n4_r8_`
- [x] T16 [after:T9] R9.S3 按顺序比较（归档时间降序）（CR7） — seam: Shell 假客户端（`crates/alterd/tests/shell.rs`） — verify: `cargo test -p alterd --test shell n4_r9_s3`
- [x] T17 [after:T14] design.md 同步：标题回写按该标签最近一次导航的历史行、搜索一律 Unicode 小写折叠后字面包含；SQL 示例、时序图、风险描述同步（CR8，文档修正） — seam: 无代码改动 — verify: `python3 -I ~/.claude/skills/dev-flow/scripts/dflow.py validate add-core-features`
- [x] T18 [after:T13] 关闭、自动归档、agent 关闭页面、删除 Space 时清理 `history_rows`（CR9） — seam: 代码确认（映射在外部不可见） — verify: `cargo test -p alterd --test shell n4_`
- [x] T19 [after:T17] 时序图图源 `assets/seq-browse-extras.mmd` 同步搜索规则后 `dflow diagrams sync`（CR10，文档修正） — seam: 无代码改动 — verify: `python3 -I ~/.claude/skills/dev-flow/scripts/dflow.py validate add-core-features`
- [x] T20 [after:T19] ER 图注同步标题回写规则（CR11，文档修正） — seam: 无代码改动 — verify: `python3 -I ~/.claude/skills/dev-flow/scripts/dflow.py validate add-core-features`
- [x] T21 [after:T20] R1.S2 先确认 iframe 的 hash 已变为 `#x`，再从那一刻起等 2 秒检查历史（验证缺口） — seam: Electron e2e（`apps/browser/e2e/history.spec.ts`） — verify: `bun run e2e --grep "N4 R1\.S2"`
- [x] T22 [after:T20] R8.S1 等满 1.5 秒时立即查数据库与已到达的快照，不再等待（验证缺口） — seam: Shell 假客户端（`crates/alterd/tests/shell.rs`） — verify: `cargo test -p alterd --test shell n4_r8_`
- [x] T23 [after:T20] 命令栏执行 Space 结果项切到该 Space（验证缺口） — seam: Electron e2e（`apps/browser/e2e/command-bar.spec.ts`） — verify: `bun run e2e --grep "N4 V4 "`
- [x] T24 [after:T20] 分屏视图断言：两个视图各占内容区一半、激活另一个后仍并排且左右不变、解散后只挂一个（验证缺口） — seam: Electron e2e（`apps/browser/e2e/split.spec.ts`） — verify: `bun run e2e --grep "N4 (R6\.S4|V5 )"`
- [x] T25 [after:T20] Peek：点击浮层外的遮罩关闭（验证缺口） — seam: Electron e2e（`apps/browser/e2e/peek.spec.ts`） — verify: `bun run e2e --grep "N4 V6 "`
- [x] T26 [after:T20] `history.rs` 的注释改为当前规则（验证缺口） — seam: 无行为改动 — verify: `cargo build -p alter-core`
- [x] T27 [after:T26] 测试断言收紧：R8.S1 断言 `archived_at` 不晚于睡眠前记下的截止时刻；分屏断言 y=0 与高度、取消分屏后 x=240；V6 断言遮罩消失、/b 的 webContents 关闭；V5 计时从点击之前开始（CR12–CR15） — seam: Shell 假客户端（`crates/alterd/tests/shell.rs`）；Electron e2e（`split.spec.ts`、`peek.spec.ts`） — verify: `cargo test -p alterd --test shell n4_r8_ && bun run e2e --grep "N4 (R6\.S4|V5 |V6 )"`
- [x] T28 [after:T27] R8.S1 在等待期间持续接收并记下快照到达时刻，截止前必须已收到不含 A 的快照（CR16） — seam: Shell 假客户端（`crates/alterd/tests/shell.rs`） — verify: `cargo test -p alterd --test shell n4_r8_`

