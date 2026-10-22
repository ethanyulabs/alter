---
change: add-core-features
---

# 执行文档：从 Chrome / Arc 导入

<!-- 行语法：- [ ] T<n> [covers:R1.S1] [after:T<k>] [P] <做什么> — seam: <测试驱动的公开入口> — verify: <命令>
对应的证据写进 log.md 之后才能勾选 [x]。完整语法见 references/formats.md -->

## Repo: alter

- [x] T1 [covers:R12.S1,R12.S2,R12.S3] 协议：导入的 Shell 事件与指令、Gateway 类型 — seam: `export-ts`（`crates/alter-core/tests/export_ts.rs`） — verify: `cargo test -p alter-core --test export_ts`
  - 字段照 design.md「接口契约」；跑 `bun run protocol:export`。
- [x] T2 [covers:R1.S1,R1.S2,R2.S1,R5.S1] [after:T1] 导入书签与历史、来源检查 — seam: 真 alterd + Gateway 客户端（新文件 `crates/alterd/tests/import.rs`） — verify: `cargo test -p alterd --test import n5_`
  - fixture 生成器放在 `crates/alterd/tests/common/fixture_profile.rs`：写 `Bookmarks` JSON，用 rusqlite 建 `History`（`urls`、`visits`）。时间用 Chrome 时间（1601 纪元的微秒）。
  - 进度事件（`import.progress` / `import.done`）与 `import.status` 在这里实现；R8.S1 由 T8 验证。
  - Cookie 导入不在本需求：`items` 含 `cookies` 时返回 `E_INVALID_ARGUMENT`（来源检查在前）。
  - 根级书签与文件夹的去重见 design.md「数据模型」的注意事项。
- [x] T3 [covers:R6.S1,R6.S2] [after:T2] Arc 与默认 profile、profile 列表 — seam: 真 alterd + Gateway 客户端 / Shell 假客户端（`crates/alterd/tests/import.rs`） — verify: `cargo test -p alterd --test import n5_r6_`
- [x] T4 [covers:R9.S1,R9.S2] [after:T2] `alter import` — seam: `alter` 二进制（新文件 `crates/alter-cli/tests/import.rs`） — verify: `cargo test -p alter-cli --test import`
  - 复用 N2 的 CLI Gateway 客户端（同步 WS）；等待 `import.done` 通知后打印。
- [x] T6 [covers:R10.S1] [after:T3] 导入向导 — seam: Electron e2e（`apps/browser/e2e/import.spec.ts`） — verify: `bun run e2e --grep "N5 R10\."`
  - Renderer `ImportWizard.tsx`：`import-source`（chrome / arc）、`import-profile`、`import-space`、`import-item`（三个复选框）、`import-start`、结果区 `import-result`、按钮 `import-done`。
  - Main：IPC `import.sources` / `import.start` 转为 Shell 事件，进度原样转给 Renderer。
- [x] T7 [covers:R11.S1,R11.S2,R11.S3,R11.S4,R11.S5] [after:T2] 命令栏的书签结果 — seam: Shell 假客户端（`crates/alterd/tests/shell.rs`） — verify: `cargo test -p alterd --test shell n5_r11_`
  - R11.S1–S4 是 MODIFIED 后保留的场景，N4 已有测试；先把测试名改为 `n5_r11_*` 并确认通过（BASELINE 意义上），再加 S5 的 RED → GREEN。
  - Renderer 的结果项支持 `data-kind="bookmark"`，执行时在当前 Space 新建标签。
- [x] T8 [covers:R8.S1] [after:T2] 省略 `items` 时导入书签与历史；两类数据的进度与 `import.status` — seam: 真 alterd + Gateway 客户端（`crates/alterd/tests/import.rs`） — verify: `cargo test -p alterd --test import n5_r8_`
  - `crates/alterd/src/import/mod.rs` 的 `in_order`：`items` 为 None 时返回书签与历史（不含 Cookie）；显式含 `cookies` 仍为 `E_INVALID_ARGUMENT`。
  - 测试收集 `import.run` 之后的全部通知，断言恰好依次是 `import.progress`（bookmarks）、`import.progress`（history）、`import.done`，再调 `import.status` 比较。
  - 同步：design.md「接口契约」已写省略时为书签与历史；`alter import` 不带 `--items` 时随之可用（R9 不变）。

## Convergence

- [x] T9 [after:T8] 源库先用 SQLite backup API 取一致快照，源库被锁时退回文件拷贝（CR1） — seam: `crates/alterd/src/import/mod.rs` 的单元测试（`#[cfg(test)]`） — verify: `cargo test -p alterd --bin alterd import:: && cargo test -p alterd --test import`
  - `copy_in` 改为 `snapshot`：只读打开源库（`busy_timeout` 1 秒），`Connection::backup` 写到 `import-tmp/<id>/`；打开或备份失败（含 `SQLITE_BUSY`）时退回原来的「主文件 + `-wal` / `-journal`」拷贝，并记一条 warn 日志。`Bookmarks` 是 JSON，仍直接拷贝。
  - 单元测试：WAL 模式的源库由另一个连接写入且不 checkpoint、连接保持打开，`snapshot` 后的副本读得到这些行（证明经 SQLite 读取，而非只拷主文件）；源库被 `BEGIN EXCLUSIVE` 锁住时退回拷贝仍能读出已提交的数据。工作区的 rusqlite 已启用 `backup`，不新增依赖。
- [x] T10 [after:T8] 收尾写库失败或没有更新到行时，导入记为 failed，不发成功的 done（CR2） — seam: `crates/alter-core/tests/store.rs` + `import::run` — verify: `cargo test -p alter-core --test store n5_ && cargo test -p alterd --test import`
  - `Store::finish_import` 返回是否恰好更新了一行；`run` 在写失败或 0 行时把状态设为 `failed`、`error` 为原因，`import.done` 的 `status` 为 `failed`。
- [x] T11 [after:T8] alterd 启动时把遗留的 `running` 导入记为 failed（`error` 为 `interrupted`）并删除 `ALTER_HOME/import-tmp`（CR3） — seam: 真 alterd + Gateway 客户端（`crates/alterd/tests/import.rs`） — verify: `cargo test -p alterd --test import n5_cr3_`
  - 测试：先用 `alterd` 建好库并停掉，再直接往库里插一行 `running` 的导入、建 `import-tmp/7/History`；启动 alterd 后 `import.status` 返回 `failed`，`import-tmp` 不存在。
  - 导入正常结束时删除临时目录失败要记 warn 日志（带路径）。
- [x] T12 [after:T8] 补测试：R1.S1 断言各层 `position`；R2.S1 重复导入后仍为 2 行；R8.S1 第二个 Gateway 连接收不到进度（CR5–CR7） — seam: 真 alterd + Gateway 客户端（`crates/alterd/tests/import.rs`） — verify: `cargo test -p alterd --test import`
  - 每条各用一个变异证明会失败：反转同级顺序、历史改为普通 `INSERT`、进度广播给所有连接。
- [x] T13 [after:T11] 启动收尾不改已结束的导入：测试再预置 `done` 与 `failed` 两行，断言启动后状态、`error` 与结束时间不变（CR9） — seam: 真 alterd + Gateway 客户端（`crates/alterd/tests/import.rs`） — verify: `cargo test -p alterd --test import n5_cr3_`
  - 变异：去掉 `WHERE status = 'running'`，用例必须失败。
- [x] T14 [after:T6] R10.S1 在截止前反复检查组合条件：「书签 3」「历史 2」与「完成」按钮同时成立（验证 G1） — seam: Electron e2e（`apps/browser/e2e/import.spec.ts`） — verify: `bun run e2e --grep "N5 R10\."`
  - 用 `apps/browser/e2e/ui.ts` 的 `expectAllWithin(t0, 5_000, …)`，`t0` 在点「开始导入」之前取。
  - 变异：完成后结果区只显示状态、不再显示计数，用例必须失败；Main 推迟 6 秒转发进度，用例必须失败。
- [x] T15 [after:T8] 每类数据导入完成时记一条带计数的日志；design.md 的 CLI 契约去掉 `partial` / `keychain_denied` 的承诺（验证 G2、G3） — seam: `crates/alterd/src/import/mod.rs` — verify: `cargo test -p alterd --test import`
  - `run` 里每类完成后 `tracing::info!(import_id, item, count, skipped, "导入完成一类")`；用 `Daemon::stop_and_stderr` 在一条契约测试里断言 stderr 有 bookmarks 与 history 两条这样的日志。
- [x] T16 [after:T14] R10.S1 的组合条件在一次同步的 DOM 读取里判断（CR10） — seam: Electron e2e（`apps/browser/e2e/import.spec.ts`） — verify: `bun run e2e --grep "N5 R10\."`
  - `win.evaluate` 里同时读 `[data-testid=import-result]` 的文字与 `[data-testid=import-done]` 是否存在且可见，返回 true 或说明。
  - 变异：同 T14 两条（完成后不显示计数、Main 推迟 6 秒）。
- [x] T17 [after:T15] 日志测试断言 `skipped`，fixture 带一条坏书签（CR11） — seam: 真 alterd + Gateway 客户端（`crates/alterd/tests/import.rs`） — verify: `cargo test -p alterd --test import n5_obs_`
  - 这个用例用标准历史加一份书签栏里多一条没有 `url` 的链接的书签：bookmarks 计数 3、skipped 1；history 计数 2、skipped 0。
  - 变异：日志里 `skipped` 恒为 0，用例必须失败。
- [x] T18 [after:T16] R10.S1 的「完成」按钮按计算样式与尺寸判断可见（CR13） — seam: Electron e2e（`apps/browser/e2e/import.spec.ts`） — verify: `bun run e2e --grep "N5 R10\."`
  - 同一次 `win.evaluate` 里检查 `getComputedStyle` 的 `display`、`visibility`、`opacity` 与 `getBoundingClientRect` 的宽高。
  - 变异：按钮加 `style="visibility:hidden"`，用例必须失败。

