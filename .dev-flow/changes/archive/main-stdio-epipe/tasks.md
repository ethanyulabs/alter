---
change: main-stdio-epipe
---

# 执行文档：stdout/stderr 断开后主进程写日志不再报错

<!-- 执行文档。每个任务是一个薄的垂直切片，能由一个测试单独证明。
行语法：- [ ] T<n> [covers:R1.S1] [after:T<k>] [P] <做什么> — seam: <测试驱动的公开入口> — verify: <命令>
有变更的场景用 RED/GREEN，已有行为用 BASELINE，没有 covers 的任务用 GREEN/VERIFY。
子条目要把执行时会卡住的点预先写掉，因为执行从不中途停下来问：涉及的文件、边界用例及钉住它的场景、
前置条件（服务、凭证、fixture、迁移、数据）和缺失时的兜底、代码会逼出的选择及其答案、
不得单独跨过的单向门，以及跨任务约束和并行时各自占用的文件和资源。
[P] 在执行前要先做独立性检查；共享的证据和任务勾选只由协调者记录。
对应的证据写进 log.md 之后才能勾选 [x]。执行时做不完的任务标 [blocked:<原因>]，问题写进 log.md。
完整语法见 references/formats.md -->

## Repo: alter

- [x] T1 [covers:R1.S1] 主进程的 stdout / stderr 加 `error` 防护 — seam: Electron e2e（`apps/browser/e2e/stdio.spec.ts`，Playwright 关闭 app 的 stdout / stderr 读端） — verify: `bun run e2e --grep "R1\.S1 stdio"`
  - 新文件 `apps/browser/src/main/stdio.ts` 导出 `guardStdio()`，`src/main/index.ts` 的第一条语句调用它（早于 `app.setPath` 等任何可能写日志的代码）。
  - 回归用例已在反馈循环里写好：用 `uncaughtExceptionMonitor` 同步写文件记录未捕获异常（弹窗会卡住主进程）；`closeOrKill` 保证失败时也不留下孤儿 app。
  - 变异：只给 stderr 加防护（stdout 仍会 EPIPE），用例必须失败。
- [x] T2 [covers:R1.S2] [after:T1] alterd 的 stderr 改为持续读取，写不出去时丢弃 — seam: Electron e2e（同上，`ALTER_ALTERD_PATH` 指向 `apps/browser/e2e/fixtures/chatty-alterd.sh`） — verify: `bun run e2e --grep "R1\.S2 stdio"`
  - `stdio.ts` 加 `forwardToStderr(stream)`；`daemon.ts:82` 的 `pipe` 改用它，更新那里「stderr 用管道转发」的注释。
  - RED 在 T1 完成之后取：此时 R1.S1 已通过，R1.S2 仍因 `pipe` 解除而失败。
  - 3 秒时限从建出 `start-logging` 之前用单调时钟（`performance.now()`）取，满足后断言没有超过截止。
  - 变异：改回 `pipe`，用例必须失败。

## Convergence

- [x] T3 [after:T2] alterd 日志的转发恢复背压：`process.stderr.write` 返回 false 时暂停读取、`drain` 后恢复；stderr 出错后不再暂停，照读照丢（CR1） — seam: Electron e2e（`apps/browser/e2e/stdio.spec.ts`，测试暂停读 app 的 stderr） — verify: `bun run e2e --grep "stdio"`
  - 新用例「stdio 背压」：测试进程暂停读 app 的 stderr，假 alterd 写 256 KB——2 秒内不应写完（被背压挡住，主进程不替它缓存）；恢复读取后 3 秒内写完。不新增场景（这是 `pipe` 原有的行为，R1.S2 不变），证据记 GREEN/VERIFY 与变异。
  - 变异：去掉暂停（回到无背压），背压用例必须失败；去掉「出错后恢复」，R1.S2 必须失败。
- [x] T4 [after:T2] 假 alterd 随 stdin 关闭退出，与真 alterd 一致（CR2） — seam: Electron e2e（同上） — verify: `bun run e2e --grep "stdio"`
  - `chatty-alterd.sh` 后台读 stdin 到 EOF 后给自己发 TERM（trap 删健康文件后退出）；过 `shellcheck`。
  - 核对：SIGKILL app 后 2 秒内假 alterd 退出（临时脚本核对一次，记 VERIFY）。
- [x] T5 [after:T2] R1.S1 断言 console 输出那一步的限时 `evaluate` 没有超时（CR3） — seam: Electron e2e（同上） — verify: `bun run e2e --grep "R1\.S1 stdio"`
- [x] T6 [after:T5] 带时限的断言从触发之前取截止点：R1.S1 两处「2 秒内响应」、「stdio 背压」恢复读取后的 3 秒（验证缺口） — seam: Electron e2e（`apps/browser/e2e/stdio.spec.ts`） — verify: `bun run e2e --grep "stdio"`
  - 截止点用 `performance.now()` 在调用 `app.evaluate` / `resume()` 之前取，等待的时限用剩余时间，满足后断言 `performance.now() <= deadline`。
  - 变异：让写日志那一步晚于 2 秒响应（stderr `error` 监听里忙等）、让恢复后写完晚于 3 秒（假 alterd 写完前 `sleep 4`），用例必须失败。
