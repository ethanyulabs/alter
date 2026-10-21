---
change: fix-orphan-alterd
---

# 执行日志：app 异常结束后 alterd 残留，重启显示「已退出」

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

### T1 alterd 加 `--exit-on-stdin-close`
- Revision: 1
- RED: `cargo test -p alterd --test lifecycle exit_on_stdin` → FAIL: 1 秒内没有出现 alterd.json，stderr：error: unexpected argument '--exit-on-stdin-close' found (R1.S1)
- GREEN: `cargo test -p alterd --test lifecycle exit_on_stdin` → PASS (1 test) (R1.S1)
- RED (mutation: `lifecycle.rs` 里把监视条件改成 `exit_on_stdin_close || true`，不带参数也监视 stdin): `cargo test -p alterd --test lifecycle exit_on_stdin` → FAIL: exit_on_stdin_r1_s2_without_flag_ignores_stdin_eof panicked: assertion failed: wait_until(Duration::from_secs(1), || health_path(home.path()).exists())（stdin 为 /dev/null 时立即退出并删掉了健康文件） (R1.S2)
- GREEN: `cargo test -p alterd --test lifecycle exit_on_stdin` → PASS (2 tests，变异已恢复) (R1.S2)
- VERIFY: `bun run ci:check` → exit 0（fmt、clippy -D warnings、cargo test 全部通过，lifecycle 7 个用例通过；lint、typecheck、bun test 1 pass、version:check）

### T2 app 以管道 stdin 和 `--exit-on-stdin-close` 拉起 alterd
- Revision: 1
- RED: `bun run e2e --grep "R2\\.S1"` → FAIL: orphan.spec.ts:30 `expect(await waitUntil(2_000, () => !isAlive(alive))).toBe(true)` Expected: true Received: false（SIGKILL app 后 alterd 仍存活） (R2.S1)
- 第一次实现（只改 spawn）后仍 FAIL，失败点移到 orphan.spec.ts:31 `existsSync(healthPath(home))` Expected: false Received: true：alterd 死了，但健康文件还在。排查见「决策与偏差」，第二处修复是 `main.rs` 的 `.log_internal_errors(false)`。
- GREEN: `bun run e2e --grep "R2\\.S1"` → PASS (1 test, 781ms) (R2.S1)
- VERIFY: `bun run e2e` → 6 passed（R7.S1、R7.S2、R7.S3、R7.S1 迟到就绪、R2.S1、R6.S1），本机 macOS（Darwin 25.6.0）主会话运行；`bun run ci:check` → exit 0

### T3 退出码 3 时复用已在运行的实例
- Revision: 1
- RED: `bun run e2e --grep "R3\\.S1"` → FAIL: orphan.spec.ts:57 `expect(win.getByText("alterd 运行中")).toBeVisible({ timeout: 3_000 })` Error: element(s) not found (R3.S1)
- GREEN: `bun run e2e --grep "R3\\.S1"` → PASS (1 test) (R3.S1)
- VERIFY: `bun run e2e` → 7 passed，本机 macOS（Darwin 25.6.0）主会话运行；`bun run ci:check` → exit 0；`pgrep -fl target/debug/alterd` 无残留

### T4 stdin 读取出错时同样清理退出，日志区分 EOF 与出错
- Revision: 2
- RED (mutation: `watch_stdin_eof` 只在 `io::copy` 返回 Ok 时发通知，出错时线程挂起): `cargo test -p alterd --test lifecycle exit_on_stdin_r1_s3` → FAIL: panicked at crates/alterd/tests/lifecycle.rs:293:10: stdin 读取出错后 2 秒内应当退出 (R1.S3)
- GREEN: `cargo test -p alterd --test lifecycle exit_on_stdin` → PASS (3 tests，变异已恢复) (R1.S3)
- REFACTOR: `watch_stdin` 把 `io::copy` 的结果发回主循环，`signal` 字段区分 `stdin-eof` / `stdin-error`，出错时另记一条 WARN 带错误文本；手动 `ALTER_HOME=<临时目录> alterd run --exit-on-stdin-close < <目录>` → 日志 `"error":"Is a directory (os error 21)"`、`"signal":"stdin-error"`、`"code":0`，exit=0 → PASS
- VERIFY: `bun run ci:check` → exit 0（lifecycle 8 passed；第一次因 clippy `unnecessary_lazy_evaluations` 失败，改为 `unwrap_or(Ok(0))` 后通过）；`bun run e2e` → 7 passed，本机 macOS（Darwin 25.6.0）主会话运行

## 决策与偏差
- 2026-10-20：起始状态：分支 `fix/fix-orphan-alterd`（从 `origin/main` 9f76156 切出），工作区干净，改动前 `cargo test` 与 `bun test` 全绿。
- 2026-10-20：tasks.md 原写 R1.S2「记为 BASELINE」，但 T1 不是 `[baseline]` 任务，证据规则要求每个场景一对 RED/GREEN，所以 R1.S2 改用变异 RED（变异未提交）。
- 2026-10-21：T2 发现根因之外的第二个原因：宿主被强制结束后 alterd 的 stderr 管道也断了，退出路径上的日志写失败，tracing-subscriber 默认 `log_internal_errors` 会再 `eprintln!` 报告，`eprintln!` 写 stderr 失败即 panic（退出码 101），跳过删除健康文件。复现：Python 拉起 `alterd run --exit-on-stdin-close`，同时关闭 stdin 与 stderr → returncode 101、健康文件仍在；源码见 tracing-subscriber 0.3.23 `src/fmt/fmt_layer.rs:1051`。修复：`crates/alterd/src/main.rs` 的 subscriber 加 `.log_internal_errors(false)`，修复后同一实验 returncode 0、健康文件已删。Spec Delta 与任务范围不变，只是 requirement.md 的根因分析漏了这一条；T1 的 Rust 用例 stderr 一直开着，所以没测出来，回归由 R2.S1 的 e2e 守住。
- 2026-10-21：手动排查时发现本机还运行着 `/Applications/alter.app/Contents/Resources/bin/alterd run`（pid 82404），与本仓库测试无关，未动它。
- 2026-10-21：T3 的 `stop()` 未改：复用时被拉起的子进程已以退出码 3 结束，`stop()` 原有的「子进程已退出就直接返回」分支就保证不去停外部实例，R3.S1 的「app 退出后外部实例仍在」由用例断言守住。
- 2026-10-21：后续（评审时发现，规格未要求）：复用外部 alterd 后，app 不再监视它；外部实例之后退出时页面仍显示「alterd 运行中」。等网关需求加握手 / 心跳时一并处理。
- 2026-10-21：验证时 codex 指出「stdin 读取出错也会清理退出」是规格没写的行为；按用户选择走 update，revision 2 补进 R1 与场景 R1.S3、任务 T4，用户已再批准。
- 2026-10-21：第 2 轮 codex 提到 R1.S3 的测试不断言日志里的 `signal` / `error` 字段。日志区分只写在 T4 的子条目里，规格没有要求，所以不记为发现；目前由 T4 的手工验证覆盖。
- 2026-10-20：用户在对话中明确授权「先把它批准并做完」，按此批准 revision 1（范围：requirement.md / tasks.md 现有内容，不扩大）。

## 代码评审
### Round 1
- Revision: 1
- Scope: alter `git diff 9f76156...HEAD`（无未提交改动）；reviewer: codex（`codex exec --sandbox read-only`，只读，未跑 Electron），加主会话复核
- Tasks: T1-T3
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR1 | minor | spec | crates/alterd/src/main.rs:20 | 新参数 `--exit-on-stdin-close`（默认关闭、stdin EOF 走清理退出、退出码 0）只写在需求文档里，`infra/api.md`「进程状态」还只描述 SIGTERM / SIGINT，维护者按接口文档看不全宿主异常退出时的契约（R1.S1、R2.S1） | accepted: 本需求没有 design.md，存档时按 `dflow archive` 的 infra_docs 清单同步更新 api.md「进程状态」与 core-flows「app 启动并托管 alterd」 |

Review: PASS
<!-- 由评审步骤填写，每次评审一轮：
### Round <n>
- Revision: <当前 revision>
- Scope: <repo> `git diff <base>...HEAD`（含未提交的改动）
- Tasks: T1, T2, …（这一轮读过代码的任务；收敛任务和重新打开的任务需要新的一轮）
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
最后恰好一行 "Review: PASS" 或 "Review: CHANGES REQUESTED"。语法见 references/formats.md -->

### Round 2
- Revision: 2
- Scope: alter `git diff 34177ab...HEAD`（第 1 轮之后的改动，无未提交改动）；reviewer: codex（`codex exec --sandbox read-only`），加主会话复核
- Tasks: T4
Findings: none.

Review: PASS

## 验证报告
<!-- 每次验证追加一份新报告，放在最近一轮评审之后。以 "- Revision: <当前 revision>" 和 "- Review: Round <最近一轮>" 开头。由验证步骤填写：本仓库的测试结果，每个场景一行 "| R<n>.S<m> | <path::test> | PASS |"，成功标准对照表，收敛缺口，fix 类型还要写 "Fix: verified | partial | failed"，最后恰好一行 "Overall: READY" 或 "Overall: NOT READY"。 -->

## 验证报告
- Revision: 2
- Review: Round 2
- Verifier: codex（test / lint / typecheck / build、场景对照、差距检查）；e2e 与原始反馈循环由主会话在本机 macOS（Darwin 25.6.0）运行

| Repo | Command | Result |
|---|---|---|
| alter | `cargo test && bun test` | PASS （Rust 17 passed、0 failed，其中 lifecycle 8 passed；bun 1 pass、0 fail） |
| alter | `cargo fmt --check && cargo clippy --all-targets -- -D warnings && bun run lint` | PASS （exit 0，无警告） |
| alter | `bun run typecheck` | PASS （exit 0） |
| alter | `cargo build && bun run build` | PASS （exit 0） |
| alter | `bun run e2e` | PASS （7 passed，主会话本机运行，代码为 6fee6a3） |

| Scenario | Test | Result |
|---|---|---|
| R1.S1 | crates/alterd/tests/lifecycle.rs::exit_on_stdin_r1_s1_closing_pipe_exits_cleanly | PASS |
| R1.S2 | crates/alterd/tests/lifecycle.rs::exit_on_stdin_r1_s2_without_flag_ignores_stdin_eof | PASS |
| R1.S3 | crates/alterd/tests/lifecycle.rs::exit_on_stdin_r1_s3_read_error_exits_cleanly | PASS |
| R2.S1 | apps/browser/e2e/orphan.spec.ts::R2.S1 SIGKILL app 后 alterd 退出 | PASS |
| R3.S1 | apps/browser/e2e/orphan.spec.ts::R3.S1 已有实例时复用 | PASS |

原始反馈循环（主会话手动重跑，代码为 6fee6a3）：以临时 `ALTER_HOME` 启动 app → 对 Electron 主进程 `kill -9` → 2 秒后「孤儿 alterd: 无；alterd.json: 无」→ 同一 `ALTER_HOME` 再启动，新 alterd 的父进程为新的 Electron → SIGTERM 退出后 alterd 无残留。

Gaps: none。已接受的 CR1（api.md / core-flows.md 未记录新参数与复用行为）在存档时同步；复用外部实例后不再监视它，已记为后续事项。

Fix: verified

Overall: READY

