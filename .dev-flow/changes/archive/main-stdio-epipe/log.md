---
change: main-stdio-epipe
---

# 执行日志：stdout/stderr 断开后主进程写日志不再报错

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

### T1 主进程的 stdout / stderr 加 `error` 防护
- Revision: 1
- RED: `bun run e2e --grep "R1\.S1 stdio"`（未修复的 main 代码）→ FAIL：`Error: 主进程出现了未捕获异常`，`Received: "Error: write EPIPE\n…"` (R1.S1)
- 实现：新文件 `apps/browser/src/main/stdio.ts` 的 `guardStdio()` 给 `process.stdout`、`process.stderr` 加忽略错误的 `error` 监听；`src/main/index.ts` 在 import 之后的第一条语句调用它
- GREEN: `bun run e2e --grep "R1\.S1 stdio"` → 1 passed，连续 3 次 (R1.S1)
- RED (mutation: 只防护 stderr): `bun run e2e --grep "R1\.S1 stdio"` → FAIL：`Received: "Error: write EPIPE"`，连续 2 次 (R1.S1)
- RED (mutation: 只防护 stdout): `bun run e2e --grep "R1\.S1 stdio"` → FAIL：`Received: "Error: write EPIPE"` (R1.S1)
- GREEN: 恢复后 `bun run e2e --grep "R1\.S1 stdio"` → 1 passed，连续 3 次 (R1.S1)
- VERIFY: `bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T2 alterd 的 stderr 改为持续读取，写不出去时丢弃
- Revision: 1
- RED: `bun run e2e --grep "R1\.S2 stdio"`（T1 已修复，`daemon.ts` 仍用 `pipe`）→ FAIL：`Error: alterd 的 stderr 没人读，写满管道后卡住`，`Expected: true  Received: false` (R1.S2)
- 实现：`stdio.ts` 加 `forwardToStderr(stream)`（`data` 事件里照写 `process.stderr`，失败由 `guardStdio` 的监听忽略）；`daemon.ts` 用它代替 `child.stderr?.pipe(process.stderr)`，更新注释
- GREEN: `bun run e2e --grep "stdio"` → 2 passed，连续 3 次 (R1.S2)
- RED (mutation: 改回 `pipe`): `bun run e2e --grep "R1\.S2 stdio"` → FAIL：`Received: false` (R1.S2)
- GREEN: 恢复后 `bun run e2e --grep "stdio"` → 2 passed (R1.S2)
- VERIFY (Widen): `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`（全部 ok）、`bun test`（1 pass）、`bun run lint`、`bun run typecheck` 通过；`bun run e2e` 25 passed。反馈循环原命令 `bun run e2e --grep "stdio"` 2 passed。运行环境：本机 macOS，主会话

### T3 alterd 日志的转发恢复背压（CR1）
- Revision: 1
- RED: 新用例「stdio 背压」`bun run e2e --grep "stdio 背压"`（T2 的无背压实现）→ FAIL：`Error: 主进程替 alterd 缓存了日志（没有背压）`，`Received: true`
- 实现：`forwardToStderr` 在 `process.stderr.write` 返回 false 时暂停读取、`drain` 后恢复；`guardStdio` 的 stderr `error` 监听记下「stderr 已断开」并恢复被暂停的转发，之后照读照丢（stdio 流出错后仍是 `writable`、`errored` 不置位，只能自己记）
- 用例：起初写 256 KB 时即使有背压也会写完——探针看到读端暂停时主进程的写缓冲停在 64 KB（`needDrain=true`），但 alterd→主进程的管道、主进程的读写缓冲、主进程→测试进程的管道、测试进程的读缓冲合计超过 256 KB。背压用例改写 4 MB（假 alterd 的写出量由 `CHATTY_BYTES` 配置，默认仍为 R1.S2 的 256 KB）
- GREEN: `bun run e2e --grep "stdio"` → 3 passed，连续 3 次 (R1.S2)
- RED (mutation: 去掉暂停，回到无背压): `bun run e2e --grep "stdio 背压"` → FAIL：`主进程替 alterd 缓存了日志（没有背压）`
- RED (mutation: stderr 断开后不恢复被暂停的转发): `bun run e2e --grep "R1\.S2 stdio"` → FAIL：`alterd 的 stderr 没人读，写满管道后卡住` (R1.S2)
- GREEN: 恢复后 `bun run e2e --grep "stdio"` → 3 passed (R1.S2)
- VERIFY: `bun run lint`、`bun run typecheck` 通过；`shellcheck e2e/fixtures/chatty-alterd.sh` 通过。上面「断开后不恢复」的变异运行留下了孤儿假 alterd（PID 82693，正是 CR2 的情形），已手动结束。运行环境：本机 macOS，主会话

### T4 假 alterd 随 stdin 关闭退出（CR2）
- Revision: 1
- RED (临时核对 `e2e/zz-orphan-probe.spec.ts`，核对完已删除：SIGKILL app 后 2 秒内假 alterd 是否退出）: `bun run e2e --grep "probe chatty"` → `CHATTY pid=87793 exited=false`
- 实现：`chatty-alterd.sh` 先 `exec 3<&0` 把 stdin 留给后台（后台列表的 stdin 默认是 /dev/null），后台 `cat <&3` 读到 EOF 后 `kill -TERM $$`，由已有的 trap 删健康文件退出
- GREEN: 同一核对 → `CHATTY pid=88282 exited=true`，之后 `pgrep -fl chatty-alterd` 无残留
- VERIFY: `shellcheck e2e/fixtures/chatty-alterd.sh` 通过；`bun run e2e --grep "stdio"` → 3 passed，连续 2 次，结束后没有残留的假 alterd 或 app。运行环境：本机 macOS，主会话

### T5 R1.S1 断言写日志那一步的限时 `evaluate` 没有超时（CR3）
- Revision: 1
- 实现：保存 console 输出那一步 `within(2_000, …)` 的结果，在未捕获异常的断言之后断言它不是 `"timeout"`
- RED (mutation: stderr 的 `error` 监听里让主进程忙等 3 秒): `bun run e2e --grep "R1\.S1 stdio"` → FAIL：`Error: 写日志那一步主进程应当在 2 秒内响应` (R1.S1)
- 同一变异下去掉新断言 → 仍 FAIL，但报在后面的 `主进程应当仍能响应`（忙等比预计的久，拖到了下一次检查）：这个变异没能单独演示 CR3 的漏洞，新断言让它在正确的那一步报出
- GREEN: 恢复后 `bun run e2e --grep "stdio"` → 3 passed，连续 3 次 (R1.S1)
- VERIFY: `bun run lint`、`bun run typecheck` 通过。运行环境：本机 macOS，主会话

### T6 带时限的断言从触发之前取截止点（验证缺口）
- Revision: 1
- 实现：R1.S1 的「写日志那一步」与「再响应一次 `app.evaluate`」各在发起调用之前取 `performance.now() + 2_000`，等待用剩余时间，完成后断言不晚于截止；「stdio 背压」在 `resume()` 之前取 3 秒截止点，`waitUntil` 用剩余时间，写完后断言不晚于截止
- RED (mutation: stderr 的 `error` 监听里忙等 3 秒，写日志那一步晚于 2 秒响应): `bun run e2e --grep "R1\.S1 stdio"` → FAIL：`写日志那一步主进程应当在 2 秒内响应` (R1.S1)
- RED (mutation: 假 alterd 写完后 `sleep 4` 才建 `stderr-done`，恢复读取后晚于 3 秒): `bun run e2e --grep "stdio 背压"` → FAIL：`恢复读取后 alterd 的日志应当写完`
- GREEN: 恢复后 `bun run e2e --grep "stdio"` → 3 passed，连续 3 次 (R1.S1)
- VERIFY: `bun run lint`、`bun run typecheck` 通过；结束后没有残留的 app 或假 alterd。运行环境：本机 macOS，主会话

## 决策与偏差
- 反馈循环阶段的诊断（临时探针 `e2e/zz-*-probe.spec.ts`，已删除；`[DEBUG-probe]` 改动已还原，`grep -rn DEBUG-probe apps` 无结果）：
  - 在 main 上点击侧边栏各控件和内容区，`process.on("warning")` 没有记录到任何警告：点击时的那条警告来自用户点到的孤儿实例（add-core-features 分支上 e2e 遗留的 app，PID 96657，`ALTER_HOME` 为 e2e 临时目录），已无从查证；修复不依赖警告的来源。
  - Playwright 的 `stream.destroy()` 关闭读端是异步的：用例要等 `close` 事件再写，否则写入可能落进管道缓冲区，得不到 EPIPE（第一次变异「只防护 stderr」因此没被抓到）。
  - `console.log` / `console.error` 的写入失败由 Node 的 Console 在回调里给没有监听器的流补一个一次性的空 `error` 监听兜住，而直接 `process.stdout.write` 的错误会成为未捕获异常；同一时刻的 console 输出还会顺带兜住直接写入的错误。stderr 这边，alterd 日志的 `pipe` 在 stderr 上挂了 `onerror`，它解除 pipe 后发现没有别的监听器就重新抛出，这是 stderr 出现未捕获异常的来路。
- R1.S1 的用例在场景写明的 `console.log`、`console.error`、`process.emitWarning` 之外，先单独做一次直接的 `process.stdout.write`：只靠 console 输出测不出 stdout 有没有防护（见上一条）。这比场景原文多一步，THEN 不变。
- T2：第一版 `forwardToStderr` 在 `process.stderr.writable` 为假时跳过写入；变异「此时停止读取（`pause`）」没被抓到——stdio 流出错后仍是 `writable`（探针看到 `destroyed=false writable=true`），那个判断是死代码，已删掉，改为照写、由 `guardStdio` 忽略错误。
- 计划里的手工核对（`bun run dev 2>&1 | head -1` 后在界面上点击）没有做：需要人手点击窗口；e2e 用例走的是同一条路径（真实 Electron 主进程、关闭的管道读端）。
- 用例的每一次 `app.evaluate` 都限时、`finally` 里 3 秒关不掉就 SIGKILL：未修复时主进程被未捕获异常的原生弹窗卡住，不限时会让用例超时并留下孤儿 app。

## 代码评审
<!-- 由评审步骤填写，每次评审一轮：
### Round <n>
- Revision: <当前 revision>
- Scope: <repo> `git diff <base>...HEAD`（含未提交的改动）
- Reviewers: <agent> (<model>, session): <n> findings; <agent> (<model>): <n> findings | unavailable (<原因>)（谁评审了、各返回了什么）
- Tasks: T1, T2, …（这一轮读过代码的任务；收敛任务和重新打开的任务需要新的一轮）
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
最后恰好一行 "Review: PASS" 或 "Review: CHANGES REQUESTED"。语法见 references/formats.md -->

### Round 1
- Revision: 1
- Scope: alter `git diff origin/main...HEAD`（T1、T2），未提交的只有与本需求无关的 `.dev-flow/config.json`；reviewer: codex (gpt-6-sol)
- Reviewers: codex (gpt-6-sol): 3 findings
- Tasks: T1, T2
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR1 | major | correctness | apps/browser/src/main/stdio.ts:18 | `forwardToStderr` 不做背压：stderr 读端还在但读得慢（终端暂停输出等）时，alterd 的日志无限缓存在主进程内存里；原来的 `pipe` 有背压，这是修复引入的退化 | T3 |
| CR2 | major | tests | apps/browser/e2e/fixtures/chatty-alterd.sh:16 | 假 alterd 不读 stdin：`closeOrKill` 强杀卡住的 app 后它不会随 stdin 关闭退出，留下循环运行的孤儿进程 | T4 |
| CR3 | minor | tests | apps/browser/e2e/stdio.spec.ts:64 | 第一次限时 `evaluate` 的结果没有断言：它超过 2 秒才恢复、后一次成功时仍通过，漏掉「2 秒内响应」 | T5 |

Review: CHANGES REQUESTED

### Round 2
- Revision: 1
- Scope: alter `git diff 33c97eb...HEAD`（收敛任务 T3–T5），未提交的只有与本需求无关的 `.dev-flow/config.json`；reviewer: codex (gpt-6-sol)
- Reviewers: codex (gpt-6-sol): 1 finding；确认 CR1、CR2、CR3 已关闭，静态检查未发现背压与「断开后恢复」之间的卡死路径
- Tasks: T3, T4, T5
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR4 | minor | tests | apps/browser/e2e/stdio.spec.ts:114 | 背压用例等固定 2 秒后以 `stderr-done` 不存在判定背压有效；测试机负载极高、假 alterd 生成 4 MB 超过 2 秒时，无背压实现也可能通过 | accepted: `head -c 4194304 /dev/zero \| tr` 生成 4 MB 只需毫秒级，与 2 秒相差三个数量级；无背压的变异在本机确定失败（T3） |

Review: PASS

### Round 3
- Revision: 2
- Scope: alter `git diff 5dfe302...HEAD`（收敛任务 T6、requirement.md revision 2 的修订），未提交的只有与本需求无关的 `.dev-flow/config.json`；reviewer: codex (gpt-6-sol)
- Reviewers: codex (gpt-6-sol): 0 findings；确认三处时限都从触发之前计、完成后核对截止，requirement.md 与实现一致
- Tasks: T6
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|

Review: PASS

## 验证报告
<!-- 每次验证追加一份新报告，放在最近一轮评审之后。以 "- Revision: <当前 revision>" 和 "- Review: Round <最近一轮>" 开头。由验证步骤填写：本仓库的测试结果，每个场景一行 "| R<n>.S<m> | <path::test> | PASS |"，成功标准对照表，收敛缺口，fix 类型还要写 "Fix: verified | partial | failed"，最后恰好一行 "Overall: READY" 或 "Overall: NOT READY"。 -->

- Revision: 1
- Review: Round 2
- Verifier: codex (gpt-6-sol)

| Repo | Command | Result |
|---|---|---|
| alter | `cargo build && bun run build` | PASS |
| alter | `cargo test && bun test` | PASS（cargo 39 passed；bun test 1 pass） |
| alter | `cargo fmt --check && cargo clippy --all-targets -- -D warnings && bun run lint` | PASS |
| alter | `bun run typecheck` | PASS |
| alter | `bun run e2e` | PASS（26 passed，提交 5dfe302，本机 macOS，主会话；结束后没有残留进程） |

| Scenario | Test | Result |
|---|---|---|
| R1.S1 | apps/browser/e2e/stdio.spec.ts::R1.S1 stdio：stdout/stderr 的读端关闭后，console 输出与进程警告不产生未捕获异常 | partial |
| R1.S2 | apps/browser/e2e/stdio.spec.ts::R1.S2 stdio：stderr 的读端关闭后，alterd 写出的 256 KB 日志 3 秒内被读走 | PASS |

Gaps:
- R1.S1 两处「2 秒内响应」在发起 `app.evaluate` 之后才开始计时，应在触发前取截止点、完成后断言不晚于截止 → T6
- 「stdio 背压」恢复读取后的 3 秒同样在 `resume()` 之后才计时、没有完成后的截止断言 → T6
- requirement.md「修复范围」「风险与回滚」仍写转发不做背压，与 T3 的实现相反 → `dev-flow update`（文档修正，需重新批准）

Fix: partial
Overall: NOT READY

- Revision: 2
- Review: Round 3
- Verifier: codex (gpt-6-sol)

| Repo | Command | Result |
|---|---|---|
| alter | `cargo build && bun run build` | PASS |
| alter | `cargo test && bun test` | PASS |
| alter | `cargo fmt --check && cargo clippy --all-targets -- -D warnings && bun run lint` | PASS |
| alter | `bun run typecheck` | PASS |
| alter | `bun run e2e` | PASS |

运行环境：提交 d1cf499，本机 macOS，主会话。`cargo test` 39 passed；`bun test` 1 pass；`bun run e2e` 26 passed，结束后没有残留进程。

| Scenario | Test | Result |
|---|---|---|
| R1.S1 | apps/browser/e2e/stdio.spec.ts::R1.S1 stdio：stdout/stderr 的读端关闭后，console 输出与进程警告不产生未捕获异常 | PASS |
| R1.S2 | apps/browser/e2e/stdio.spec.ts::R1.S2 stdio：stderr 的读端关闭后，alterd 写出的 256 KB 日志 3 秒内被读走 | PASS |

Gaps: none（上一份报告的三个缺口由 T6 与 requirement.md revision 2 关闭，评审第 3 轮 PASS；带时限的断言逐条核对过，都从触发之前计、完成后核对截止）。

Fix: verified
Overall: READY
