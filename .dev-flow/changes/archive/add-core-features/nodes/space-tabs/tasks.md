---
change: add-core-features
---

# 执行文档：Space 管理与标签整理

<!-- 行语法：- [ ] T<n> [covers:R1.S1] [after:T<k>] [P] <做什么> — seam: <测试驱动的公开入口> — verify: <命令>
有变更的场景用 RED/GREEN，已有行为用 BASELINE，没有 covers 的任务用 GREEN/VERIFY。对应的证据写进 log.md 之后才能勾选 [x]。完整语法见 references/formats.md -->

## Repo: alter

- [x] T1 [baseline] [covers:R13.S3,R15.S2,R15.S3,R15.S4] 基线：迁移改动前，WAL / 外键与 alterd 启动退出的已有行为 — seam: `Store::open`；`alterd run`（`crates/alter-core/tests/store.rs`、`crates/alterd/tests/lifecycle.rs`） — verify: `cargo test -p alter-core --test store && cargo test -p alterd --test lifecycle`
  - 这些场景在 MODIFIED 后原样保留，已有测试覆盖；在动迁移之前先跑一遍并记 BASELINE，之后每个任务结束时它们必须仍然通过。
- [x] T2 [after:T1] [covers:R13.S1,R13.S2,R13.S4,R14.S1,R14.S2] 迁移 0002 与迁移前备份 — seam: `Store::open`（`crates/alter-core/tests/store.rs`） — verify: `cargo test -p alter-core --test store n1_`
  - 新文件 `crates/alter-core/migrations/0002_spaces_tabs.sql`（DDL 见 design.md「数据模型」），在 `crates/alter-core/src/store/migrate.rs` 的迁移列表里登记；根 `Cargo.toml` 给 rusqlite 加 `backup` feature。
  - 测试名以 `n1_` 开头，避免与已存档需求的 `r1_s1` 等撞名（`cargo test … r13_` 会选中旧用例）。
  - 构造「只应用了 0001 的库」：在测试里直接执行 `0001_init.sql` 并插入 `schema_version(1)`，不要调 `Store::open`。
  - R13.S3（WAL 与外键）已有测试 `crates/alter-core/tests/store.rs`，保持通过即可，不重复写。
  - 备份失败要让 `Store::open` 返回错误，不能在没有备份的情况下继续迁移。
  - 现有断言 `MAX(version) = 1`、`COUNT(*) = 1` 会变红，按 MODIFIED 后的场景改成 2。
- [x] T3 [covers:R15.S1] [after:T2] alterd 启动后数据库为版本 2 — seam: `alterd run`（`crates/alterd/tests/lifecycle.rs`） — verify: `cargo test -p alterd --test lifecycle`
  - 只改现有用例里 `MAX(version)` 的期望值；R15.S2–S4 的用例不动，保持通过。
- [x] T4 [covers:R16.S1,R16.S2] [after:T2] 协议：Space / 固定 / 移动事件、`shell.discard`、`TabView.pinned`、`SPACE_COLORS` 导出 — seam: `export-ts`（`crates/alter-core/tests/export_ts.rs`） — verify: `cargo test -p alter-core --test export_ts`
  - 在 `crates/alter-core/src/protocol/shell.rs` 加 6 个 `ShellEvent` 变体和 `ShellCommand::Discard`，`TabView` 加 `pinned`；字段名照 design.md「接口契约」。
  - `SPACE_COLORS: [&str; 8]` 放在同一文件；`src/bin/export_ts.rs` 额外写 `spaceColors.ts`（`export const SPACE_COLORS = [...] as const;`）。
  - 跑 `bun run protocol:export` 更新 `packages/protocol/src/generated/`，生成文件不手改；`bun run typecheck` 必须仍然通过（Electron 端此时只是多了未处理的变体）。
- [x] T5 [covers:R1.S1,R1.S2,R1.S3,R1.S4,R2.S1,R2.S2,R3.S1,R3.S2,R4.S1,R4.S2] [after:T4] alterd：新建 / 编辑 / 切换 Space，当前 Space 持久化 — seam: Shell Control 假客户端（`crates/alterd/tests/shell.rs` + `tests/common/mod.rs`） — verify: `cargo test -p alterd --test shell n1_`
  - `crates/alter-core/src/store/spaces.rs` 加 `create_space`、`update_space`、`activate_space`、`active_space_id`；`crates/alterd/src/tabs/mod.rs` 的 `handle` 增加分派，`snapshot()` 改用 `active_space_id()`。
  - 名称校验：`trim()` 后按 `chars().count()` 计 1–32；颜色必须在 `SPACE_COLORS` 里。
  - R4.S2 要手工建一个 0001 版本的库再启动 alterd，可复用 T2 的辅助函数（移到 `crates/alterd/tests/common/mod.rs` 或复制一份）。
  - 用 `common::Client` 的现有读写方法；新增辅助 `send_json`、`expect_error(code)`。
- [x] T6 [covers:R5.S1,R5.S2,R5.S3,R5.S4,R6.S1] [after:T5] alterd：删除 Space 与启动清理 partition 目录 — seam: Shell Control 假客户端；`alterd run`（`crates/alterd/tests/shell.rs`） — verify: `cargo test -p alterd --test shell -- n1_r5_ n1_r6_`
  - `Store::delete_space` 在一个事务里：数未归档 Space ≤ 1 → 拒绝；是当前 Space 时先算相邻 Space 并更新它的 `last_active_at`；再 `DELETE FROM spaces WHERE id = ?`（外键级联删除标签）。
  - `crates/alterd/src/partitions.rs::sweep(home, &ids)`：只匹配 `^space-[0-9a-z]{26}$`，与 id 的小写比较；删除失败只记 WARN，不影响启动。在 `lifecycle::run` 中、`Store::open` 之后、写 `alterd.json` 之前调用。
  - R6.S1 的断言时机：在 `alterd.json` 出现的那一刻检查目录，计时从 `alterd.json` 的 mtime 起算（AGENTS.md 的约定）。
- [x] T7 [covers:R8.S1,R8.S2,R8.S3,R9.S1,R10.S1,R11.S1,R11.S2,R11.S3,R11.S4] [after:T5] alterd：固定、关闭固定标签触发 discard、移动标签 — seam: Shell Control 假客户端（`crates/alterd/tests/shell.rs`） — verify: `cargo test -p alterd --test shell -- n1_r8_ n1_r9_ n1_r10_ n1_r11_`
  - `crates/alter-core/src/store/tabs.rs`：`set_pinned`、`move_tab`，以及一个私有的 `renumber(tx, space_id, ordered_ids)`；`insert_tab` 的 position 改为「未归档标签的最大 position + 1」；`close_tab` 对固定标签改为不归档、`url = pinned_url`，返回值区分「已归档」和「已卸载」，让 TabService 决定是否回 `shell.discard`。
  - R9.S1 是 MODIFIED 后仍保留的场景，已有测试 `r10_s1_close_active_middle_tab_activates_next` 覆盖；本任务先跑它当基线，确认改动后仍然通过，再加固定标签的新用例。
  - `list_open_tabs` 的排序改为 `ORDER BY s.position, t.position`（重新编号后 position 已包含固定在前的规则），并带出 `pinned`。
- [x] T8 [covers:R1.S5,R2.S3,R3.S3,R5.S5,R5.S6,R7.S1] [after:T6] Electron：SpaceBar、编辑面板、切换、删除确认与 clearStorageData，以及 Cookie 隔离 — seam: Electron e2e（`apps/browser/e2e/spaces.spec.ts`） — verify: `bun run e2e --grep "N1 R(1|2|3|5|7)\."`
  - e2e 用例标题以 `N1 R1.S5` 这种形式开头，避免 `--grep` 选中已存档需求的同名 ID。
  - 测试服务器 `apps/browser/e2e/fixtures/server.ts` 增加 `/set-cookie`（`Set-Cookie: k=v; Path=/`，标题 `Set Cookie`）和 `/echo-cookie`（正文 `cookie: ` 加请求头，标题 `Echo Cookie`）。
  - Renderer：`SpaceBar.tsx`（`data-testid="space-dot"`，激活的加 `class="active"`；`space-add`）、`SpacePanel.tsx`（`space-name-input`、`space-color`×8 带 `data-color`、`space-save`、`space-delete`）、`ConfirmDialog.tsx`（`confirm-ok`、`confirm-cancel`）。右键用 `onContextMenu`。
  - Main：`TabViews.apply` 在上一份快照中有、新快照中没有的 Space，先关闭它的全部视图，再 `session.fromPartition(...).clearStorageData()`。
  - R5.S5 在主进程读 Cookie：`app.evaluate(({ session }, p) => session.fromPartition(p).cookies.get({}), "persist:space-<id>")`；Space id 从 `alter.db`（`node:sqlite` 只读）查。
- [x] T9 [covers:R8.S4,R9.S2,R10.S2,R11.S5] [after:T7,T8] Electron：固定分区、图钉按钮、discard、拖拽 — seam: Electron e2e（`apps/browser/e2e/pinned.spec.ts`） — verify: `bun run e2e --grep "N1 R(8|9|10|11)\."`
  - `Sidebar.tsx` 分两段渲染，中间 `data-testid="pinned-divider"`；行加 `data-pinned`、`draggable`、图钉按钮 `tab-pin`；drop 时算出目标分区与 index，经 IPC `tab.move` 发出。
  - Main 收到 `shell.discard` 时关闭该视图（`webContents.close()`）并从 map 里删除；若它正挂在窗口上，按新快照挂另一个。
  - R9.S2 是 MODIFIED 后保留的场景，已有 e2e `R10.S2 关闭最后一个标签` 覆盖；把那条用例改名为 `N1 R9.S2 …` 并确认仍通过，不重写。
  - 拖拽先用 `locator.dragTo`，不稳定时按 design.md「风险」的办法改为派发 DnD 事件，并在 log.md 记偏差。
- [x] T10 [covers:R12.S1,R12.S2,R12.S3,R12.S4,R12.S5,R12.S6,R12.S7] [after:T8,T9] Electron：应用菜单与快捷键 — seam: Electron e2e（`apps/browser/e2e/shortcuts.spec.ts`，主进程 `Menu.getApplicationMenu().getMenuItemById(id).click()`） — verify: `bun run e2e --grep "N1 R12\."`
  - 新文件 `apps/browser/src/main/menu.ts`（菜单结构，保留 macOS 的 app / Edit 菜单，否则复制粘贴会失效）和 `actions.ts`（菜单与 IPC 共用的动作函数）；`index.ts` 里原有的 IPC 处理改为调用 `actions.ts`。
  - `tab.new` / `address.focus`：Main 向 Renderer 发 `address.focus {newTab}`，并 `win.webContents.focus()`，否则焦点还留在页面视图里。
  - `tab.next` / `tab.prev` 按快照里当前 Space 的标签顺序循环（固定分区在前）。
- [x] T11 [after:T10] 性能：500 个标签时切换 Space 的界面耗时（父级 SC4） — seam: Electron e2e（`apps/browser/e2e/perf.spec.ts`） — verify: `bun run e2e --grep SC4`
  - 用 Shell 事件直接灌两个 Space 各 250 个标签（经主进程调 `shell.send`，不走 UI），只为激活标签建视图。
  - 计时从主进程收到 `space.activate` IPC 起，到 Renderer 在 `requestAnimationFrame` 回调里看到新列表为止，经 `performance.now()` 打点后由 `app.evaluate` 取回；20 次的 p95 < 50 ms，超标就失败。

## Convergence

- [x] T12 [after:T11] 拖到分隔线即固定、拖到普通分区空白处即取消固定（CR1、CR2） — seam: Electron e2e（`apps/browser/e2e/pinned.spec.ts`） — verify: `bun run e2e --grep "N1 CR(1|2) "`
  - 分隔线落点改发「固定分区里除被拖标签外的数量」，不再发超出 `u32` 的 `Number.MAX_SAFE_INTEGER`；普通分区末尾加一个占满剩余高度的落点 `normal-drop`，拖到这里放到普通分区末尾。
  - 先写两条用例看到 RED：拖到分隔线后 Page B 进入固定分区；全部固定后把一行拖到 `normal-drop`，它回到普通分区。
- [x] T13 [after:T11] 关闭普通标签后同 Space 的未归档标签重新连续编号（CR3） — seam: Shell Control 假客户端（`crates/alterd/tests/shell.rs`） — verify: `cargo test -p alterd --test shell n1_cr3_`
  - `close_tab` 归档分支在同一事务里调 `renumber`；测试关闭 A、B、C 中的 B 后断言库里 A、C 的 position 为 0、1，再新建 D 为 2。
- [x] T14 [after:T11] 已卸载的固定标签在用户再次激活前不重建页面（CR4） — seam: Electron e2e（`apps/browser/e2e/pinned.spec.ts`） — verify: `bun run e2e --grep "N1 CR4 "`
  - Space 里只有一个固定标签时关闭它：之后 1 秒内主进程没有任何 webContents 的 URL 是它的页面；点击该行后 2 秒内内容区显示它固定时的 URL。
  - `TabViews` 记一组已卸载的标签，`apply` 不为它们建视图；用户经 `actions.activateTab` 激活时清掉标记。
- [x] T15 [after:T11] 删除 Space 时同时清空 HTTP 缓存（CR5） — seam: Electron e2e（`apps/browser/e2e/spaces.spec.ts`） — verify: `bun run e2e --grep "N1 CR5 "`
  - 测试服务器加可缓存的 `/cached`（`Cache-Control: max-age=3600`）；在 Work 打开它，确认 `session.getCacheSize() > 0`，删除 Work 后 2 秒内为 0。`TabViews.apply` 对被删的 Space 再调 `clearCache()`。
- [x] T16 [after:T11] SC4 基准断言目标 Space 的列表内容已渲染（CR6） — seam: Electron e2e（`apps/browser/e2e/perf.spec.ts`） — verify: `bun run e2e --grep SC4`
  - 两个 Space 灌不同的地址（Personal `/n/0…249`，Work `/n/1000…1249`），计时终点除名称、行数外还要求第一行和最后一行的文字是目标 Space 的地址；用「名称切了、列表没切」的变异证明会失败。
- [x] T17 [after:T16] `wake` 只清除已卸载标记，不再应用缓存的旧快照（CR7） — seam: Electron e2e（`apps/browser/e2e/pinned.spec.ts`） — verify: `bun run e2e --grep "N1 (CR4 |R10\.)"`
  - 视图由激活之后 alterd 必发的那份快照重建（它一定排在关闭结果之后，url 已回到 `pinned_url`）；`TabViews.last` 随之删除。
  - 竞态窗口在 e2e 里无法稳定构造：用变异（`wake` 里重新应用一份 url 仍是导航后地址的旧快照）说明 R10.S2 的「回到 /a」断言依赖这一点，并记录限制。
- [x] T18 [after:T16] SC4 计时终点逐行核对 250 行都属于目标 Space（CR8） — seam: Electron e2e（`apps/browser/e2e/perf.spec.ts`） — verify: `bun run e2e --grep SC4`
  - 每行文字应为目标 Space 第 i 个标签的 URL 或其标题 `Page <n>`；用「只换第一行、其余不变」的变异证明会失败。
- [x] T19 [after:T18] 同一事件的私有回复（`shell.discard` / `shell.navigate` / `shell.error`）先于它引起的快照写出（CR9） — seam: Shell Control 假客户端（`crates/alterd/tests/shell.rs`） — verify: `cargo test -p alterd --test shell n1_cr9_`
  - 连接任务的 `tokio::select!` 加 `biased;`，私有回复分支排第一；TabService 先发回复再广播，因此回复总在该事件的快照之前，也在之后事件的快照之前。
  - 测试在一个连接上重复 30 次「固定 → 激活 → 关闭」，每次关闭后的第一条消息必须是 `shell.discard`；R10.S1 的测试改为按顺序断言。design.md 的接口契约写明这个顺序。
- [x] T20 [after:T19] 关闭固定标签后、`shell.discard` 到达前再次激活它，页面按固定地址正常打开（CR10） — seam: Electron e2e（`apps/browser/e2e/pinned.spec.ts`） — verify: `bun run e2e --grep "N1 CR10 "`
  - `TabViews` 记「已发出关闭、尚未被再次激活」的标签（`closing(tabId)`，由 `actions` 在发 `tabClosed` 时调用）；`wake` 同时清掉它；迟到的 `shell.discard` 只在标签仍在这组里时打「已卸载」标记，否则只关掉旧视图。
  - 用例：唯一的固定标签导航到 /b 后，在 Renderer 里同一轮事件循环内连发关闭与激活，2 秒内内容区显示 /a。
- [x] T22 [after:T20] 每个连接上消息的写出顺序与 alterd 处理事件的顺序一致（CR12） — seam: Shell Control 假客户端（`crates/alterd/tests/shell.rs`） — verify: `cargo test -p alterd --test shell n1_cr`
  - TabService 每次发布快照序号加 1；私有回复带上发出时最新的快照序号 `after`。连接任务仍优先处理私有回复，但写一条回复前先把序号 ≤ `after` 的快照写完。
  - 测试在一个连接上重复 30 次：不读消息连发 `tabUpdated`（url /qN）与关闭固定标签，依次必须收到「url 为 /qN 的快照 → shell.discard → url 回到固定地址的快照」。
- [x] T23 [after:T22] 连接落后、广播丢帧时回复仍不越过更新的快照（CR13） — seam: `shell::write_reply` 单元测试（`crates/alterd/src/shell/mod.rs`） — verify: `cargo test -p alterd --bin alterd n1_cr13`
  - 补写循环收到序号 > `after` 的快照时，先写回复，再写这份快照。
  - 测试：客户端停止读取，连发 300 个新建标签、1 个在已有标签打开（产生 `shell.navigate`）、再 300 个新建标签，让连接落后丢帧；之后读完全部消息，`shell.navigate` 之前的最后一份快照不超过 301 个标签，之后第一份快照至少 302 个。
- [x] T24 [after:T22] 关闭中或已卸载的固定标签，旧视图的页面事件不再上报（CR14） — seam: Electron e2e（`apps/browser/e2e/pinned.spec.ts`） — verify: `bun run e2e --grep "N1 (CR4 |CR10 |R10\.)"`
  - `TabViews.create` 里的 `update` 回调在标签处于 `closing` 或 `discarded` 时直接返回；竞态窗口（关闭后、discard 前旧视图完成导航）在 e2e 里无法稳定构造，按代码路径修复并记录限制。
- [x] T25 [after:T24] 每个连接改为一条先进先出队列，快照与回复按 TabService 的处理顺序写出（CR15，替换 T19 / T22 / T23 的序号与补写机制） — seam: Shell Control 假客户端（`crates/alterd/tests/shell.rs`） — verify: `cargo test -p alterd --test shell n1_cr`
  - TabService 持有各连接的 `mpsc::UnboundedSender<Arc<str>>`（新连接经注册通道加入，写失败或连接关闭即移除）；发布快照时依次放进每个连接的队列，回复只放进请求方的队列。删掉 `Snapshot.seq`、`Reply.after`、`write_reply` 与广播通道。
  - 测试：慢连接 B 停止读取期间 A 连开 300 个标签，B 之后按顺序收到标签数 1…300 的全部快照（能分辨新旧实现）；另一条「400 对 tabUpdated + 被拒绝的关闭」严格交替的用例留作回归保护。
- [x] T26 [after:T24] 关闭固定标签时 Main 立即关掉它的视图；页面事件只在视图仍是该标签当前登记的视图时上报（CR16） — seam: Electron e2e（`apps/browser/e2e/pinned.spec.ts`） — verify: `bun run e2e --grep "N1 (CR4 |CR10 |R10\.)"`
  - `actions` 关闭时把「是否固定」交给 `TabViews.closingTab`，固定标签立刻 `webContents.close()` 并从视图表删除；`update` 回调比对视图实例。
- [x] T27 [after:T26] alterd 记录已卸载的固定标签：快照 `TabView.discarded`，再次激活时清除，已卸载期间忽略它的 `tabUpdated`（CR17、CR18） — seam: Shell Control 假客户端（`crates/alterd/tests/shell.rs`）；`export-ts` — verify: `cargo test -p alterd --test shell n1_cr17_ && cargo test -p alter-core --test export_ts`
  - TabService 内存里的 `discarded: HashSet<String>`（不落库：重启后页面本来就没加载）；关闭固定标签时加入，`tabActivated` 移除，标签消失时移除；`snapshot()` 输出 `discarded`。
  - 测试：关闭固定标签 P 后的快照里 P 的 `discarded` 为 true；随后 `tabUpdated`（P → /x）1 秒内没有新快照、库里 url 不变；`tabActivated`（P）后的快照 `discarded` 为 false、url 为固定地址。
- [x] T28 [after:T27] Main 的视图完全跟随快照：`discarded` 的标签不建视图（已有的关掉）；用户关闭标签时把它当时的视图记为作废，作废视图的页面事件不上报；删除 `closing` / `discarded` / `wake`（CR17、CR18） — seam: Electron e2e（`apps/browser/e2e/pinned.spec.ts`） — verify: `bun run e2e --grep "N1 (CR4 |CR10 |R10\.|R12\.)"`
- [x] T29 [after:T27] 发件箱清理与 TabService 退出时结束连接（CR19、CR20） — seam: Shell Control 假客户端（`crates/alterd/tests/shell.rs`） — verify: `cargo test -p alterd --test shell`
  - `Connect` 与发布时 `retain(|o| !o.is_closed())`；连接任务的 `select!` 加 `requests.closed()` 分支。两条都是 minor，按 GREEN/VERIFY 记录。
- [x] T30 [after:T29] 关闭屏障：用户关闭标签时 Main 立即关掉它的视图，在关闭结果到达前（固定标签的 `shell.discard`、普通标签从快照消失）不为它建视图；弹窗处理器只接受当前登记视图的请求（CR21、CR22） — seam: Electron e2e（`apps/browser/e2e/pinned.spec.ts`） — verify: `bun run e2e --grep "N1 (CR4 |CR10 |R10\.|R12\.)"`
  - 替换 T28 的 `voidView` / `voided`；交错窗口在 e2e 里无法稳定构造，按 GREEN/VERIFY 记录。
- [x] T31 [after:T30] 关闭屏障按次数计：每次关闭加 1，每条 `shell.discard` 减 1，减到 0 才撤掉；标签从快照消失时清掉（CR23） — seam: Electron e2e（`apps/browser/e2e/pinned.spec.ts`） — verify: `bun run e2e --grep "N1 (CR4 |CR10 |R10\.|R12\.)"`
- [x] T32 [after:T31] 在已卸载的标签里打开地址：alterd 把它的 url 改为新地址、清除卸载状态并发布快照，而不是回 `shell.navigate`（CR24） — seam: Shell Control 假客户端（`crates/alterd/tests/shell.rs`）；Electron e2e（`apps/browser/e2e/pinned.spec.ts`） — verify: `cargo test -p alterd --test shell n1_cr24_ && bun run e2e --grep "N1 CR24 "`
  - 契约测试：关闭固定标签 P 后发 `openRequested {spaceId, input: "example.com/z", tabId: P}`，下一条是快照（不是 `shell.navigate`），P 的 `discarded` 为 false、url 为 `https://example.com/z`。
  - e2e：唯一的固定标签关闭后，在地址框输入 /b 回车，2 秒内内容区显示 /b。
- [x] T33 [after:T32] 已卸载的标签在快照里不算激活；在已卸载标签里打开地址时写库成功后才清除卸载状态（CR26、CR27） — seam: Shell Control 假客户端（`crates/alterd/tests/shell.rs`）；Electron e2e（`apps/browser/e2e/pinned.spec.ts`） — verify: `cargo test -p alterd --test shell n1_ && bun run e2e --grep "N1 (CR4 |CR24 |CR27 |R10\.)"`
  - `snapshot()` 计算每个 Space 的激活标签时跳过 `discarded` 的标签：Space 只剩已卸载标签时没有激活标签，Renderer 显示「没有打开的标签」，刷新等按钮随之不可用。
  - 契约测试：Space 只有固定标签 P，关闭后快照里 P 的 `active` 为 false；点击（`tabActivated`）后为 true 且 `discarded` 为 false。e2e：关闭唯一的固定标签后内容区显示「没有打开的标签」、该行不是激活样式。
  - CR24 的 e2e 改为：此时在地址框打开 /b 会新开一个标签显示 /b，固定标签仍在固定分区。
- [x] T34 [after:T33] 没有激活标签时 `tab.next` 激活第一行、`tab.prev` 激活最后一行（CR28） — seam: Electron e2e（`apps/browser/e2e/shortcuts.spec.ts`） — verify: `bun run e2e --grep "N1 (CR28 |R12\.S4)"`
- [x] T35 [after:T34] Main 作用于「当前激活标签」的动作不把关闭屏障中的标签当作激活标签（CR29、CR30） — seam: Electron e2e（`apps/browser/e2e/pinned.spec.ts`） — verify: `bun run e2e --grep "N1 (CR29 |CR24 |CR27 |CR28 |R12\.)"`
  - `ActionContext.isClosing(tabId)` 查 `TabViews` 的关闭屏障；`activeTab()` 遇到屏障中的标签返回 undefined，`open` / `closeActiveTab` / `togglePinActive` / `cycleTab` 都经它。
  - 用例：在 Renderer 同一轮事件循环里连发「关闭唯一的固定标签」与「地址框打开 /b」（不新建标签模式），结果是新开标签显示 /b，固定标签仍在固定分区、url 未变。
- [x] T36 [after:T35] e2e 断言场景 THEN 的全部条件在场景时限内同时成立，R10.S2 断言保留的是被关闭的那一行（验证缺口） — seam: Electron e2e（`apps/browser/e2e/spaces.spec.ts`、`pinned.spec.ts`、`sidebar.spec.ts`、`shortcuts.spec.ts`） — verify: `bun run e2e --grep "N1 R(1\.S5|3\.S3|5\.S5|8\.S4|9\.S2|10\.S2|12\.S2|12\.S5|12\.S6) "`
  - `e2e/ui.ts` 加 `expectWithin(t0, ms)`：从操作前记下的时刻算起，全部断言通过后总耗时不超过场景时限（只会高估）。
- [x] T37 [covers:R6.S1] [after:T35] 启动清理按不区分大小写识别 `space-<id>` 目录（验证缺口） — seam: `alterd run`（`crates/alterd/tests/shell.rs`） — verify: `cargo test -p alterd --test shell n1_r6_`
  - 测试另建一个大写的孤儿目录 `space-01JYYYYYYYYYYYYYYYYYYYYYYY`，启动后应被删除。
- [x] T38 [after:T35] design.md 跟上代码：卸载状态由 alterd 的 `TabView.discarded` 决定；删除最后一个 Space 时 UI 置灰删除按钮、错误显示在地址框下（验证缺口） — seam: 文档 — verify: `python3 -I ~/.claude/skills/dev-flow/scripts/dflow.py validate add-core-features`
- [x] T39 [after:T38] 场景 THEN 改为在时限内轮询的组合条件（同一次读取界面状态），计时用单调时钟；R8.S4 的分隔线检查纳入组合条件；R10.S2 按标签 id 确认保留的是同一个标签（CR32–CR35） — seam: Electron e2e（同 T36） — verify: `bun run e2e --grep "N1 R(1\.S5|3\.S3|5\.S5|8\.S4|9\.S2|10\.S2|12\.S2|12\.S5|12\.S6) "`
  - `e2e/ui.ts`：`uiState(win)` 一次读出 Space 圆点、Space 名称、各行（文字、激活、固定、是否在分隔线上方）与空内容提示；`expectAllWithin(t0, ms, what, check)` 以 `performance.now()` 计时轮询 `check`，删掉 `expectWithin`。
- [x] T40 [after:T39] `uiState` 按屏幕位置与可见性判断是否在分隔线上方；标签行带 `data-tab-id`，R10.S2 在同一次界面读取里核对被关闭的那一行的 id（CR36、CR37） — seam: Electron e2e（同 T36） — verify: `bun run e2e --grep "N1 R(1\.S5|3\.S3|5\.S5|8\.S4|9\.S2|10\.S2|12\.S2|12\.S5|12\.S6) "`
- [x] T41 [after:T40] 分隔线的可见性判断同时要求上边框宽度大于 0 且颜色不透明（CR38） — seam: Electron e2e（同 T36） — verify: `bun run e2e --grep "N1 R(8\.S4|10\.S2|12\.S6) "`
- [x] T42 [after:T41] 补测 R12 正文与 R13.S1 的总数：`address.focus` 选中地址框文字，菜单 `nav.back` / `nav.forward` 生效，库里非 SQLite 内部表恰好 8 张（验证缺口） — seam: Electron e2e（`apps/browser/e2e/shortcuts.spec.ts`）；`Store::open`（`crates/alter-core/tests/store.rs`） — verify: `bun run e2e --grep "N1 R12" && cargo test -p alter-core --test store n1_r13_s1`
- [x] T43 [after:T42] 已存档用例 `CR7 错误提示` 的前置步骤等 favicon 回写到齐再归档标签，消除迟到 `tabUpdated` 造成的偶发失败（验证中发现） — seam: Electron e2e（`apps/browser/e2e/sidebar.spec.ts`） — verify: `bun run e2e --grep "CR7 错误提示" --repeat-each 30`
- [x] T44 [after:T42] `N1 CR28` 的前置步骤先确认要固定的行已激活再按快捷键（验证中发现的偶发失败） — seam: Electron e2e（`apps/browser/e2e/shortcuts.spec.ts`） — verify: `bun run e2e --grep "N1 CR28 " --repeat-each 15`

