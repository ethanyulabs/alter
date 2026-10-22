---
id: main-stdio-epipe
title: stdout/stderr 断开后主进程写日志不再报错
type: fix
status: archived
revision: 2
approved_revision: 2
repos: [alter]
created: 2026-10-21
skip_specs: false
---

# stdout/stderr 断开后主进程写日志不再报错

<!-- fix 类型的需求文档，保持简短。正文用中文写，保留字保持英文（references/formats.md）。 -->

## 现象
- 用户在界面上点击时，弹出「A JavaScript error occurred in the main process」，内容为 `Uncaught Exception: Error: write EPIPE`，调用栈止于 `console.error` ← `writeOut (node:internal/process/warning)`。app 没有退出，只是弹窗。2026-10-21 报告。
- 出现的条件：主进程的 stdout / stderr 是一根读端已关闭的管道。用户点到的窗口是 e2e 运行遗留的孤儿 app，Playwright 早已退出，管道没人读了；从终端经管道启动后关掉读的一端（如 `bun run dev | head`）也会这样。
- 严重程度：中。每次写日志都会弹窗；更隐蔽的是 alterd 的 stderr 随之没人读，写满管道后 alterd 卡住，Gateway 与 Shell 都不再响应。`main` 上同样存在（基础代码）。

## 复现
1. 启动 app，等窗口显示「alterd 运行中」。
2. 关闭 app 的 stdout 与 stderr 管道的读端。
3. 主进程写一行 `console.error`，或发出一条进程警告。
- 期望：不报错，主进程照常响应；alterd 之后写的日志被照常读走（丢弃）。
- 实际：两次 `Error: write EPIPE` 成为未捕获异常并弹窗，弹窗卡住主进程；alterd 往 stderr 写满 64 KB 后阻塞。

## 反馈循环
`bun run e2e --grep "stdio"`（`apps/browser/e2e/stdio.spec.ts`，假 alterd `e2e/fixtures/chatty-alterd.sh`），修复前：

```
✘ R1.S1 stdio … Error: 主进程出现了未捕获异常  Received: "Error: write EPIPE\nError: write EPIPE"
✘ R1.S2 stdio … Error: alterd 的 stderr 没人读，写满管道后卡住  Expected: true  Received: false
```

## 根因
- 假设（按可能性排序，每条给出预测）：
  1. `process.stdout` / `process.stderr` 没有 `error` 监听：写失败时流发出的 `error` 事件没人处理，成为未捕获异常。预测：给两个流加 `error` 监听后 R1.S1 通过；只给 stderr 加，stdout 的 EPIPE 仍会出现。
  2. `daemon.ts` 用 `child.stderr.pipe(process.stderr)` 转发 alterd 日志：目标流出错时 `pipe` 自动解除，之后没人读 alterd 的 stderr。预测：只做假设 1 的修复时 R1.S2 仍失败；改成持续读取并在写不出去时丢弃后通过。
  3. 点击时调用了本仓库代码里的某个弃用 API，产生了进程警告。预测：在 `main` 上点击界面会记录到 `warning` 事件。实测（临时探针，挂 `process.on("warning")` 点击各控件）：没有任何警告，所以警告只是触发写入的那一次，不是缺陷本身。孤儿实例所在分支上的那条警告已无从查证。
  4. Electron 的原生代码往断开的管道写。预测：异常栈里没有 JS 帧。实际栈全是 JS（`console.error`），排除。
- 已确认：
  - `apps/browser/src/main/index.ts` 与整个 `src/main` 都没有处理 `process.stdout` / `process.stderr` 的 `error` 事件（假设 1，由反馈循环的 R1.S1 确认）。
  - `apps/browser/src/main/daemon.ts:82` 的 `child.stderr?.pipe(process.stderr)`（假设 2，由 T2 的 RED 在 T1 修复之后确认）。

## 修复范围
- 新增 `apps/browser/src/main/stdio.ts`：
  - `guardStdio()` 给 `process.stdout`、`process.stderr` 加 `error` 监听并忽略。读端已关闭时日志本来就无处可写，也没有别处可报告。
  - `forwardToStderr(stream)` 以 `data` 事件读取并转发：stderr 正常时保留背压（写入返回 false 就暂停读取，`drain` 后恢复）；stderr 断开后（`guardStdio` 的 `error` 监听记下）恢复读取，数据直接丢弃。
- `src/main/index.ts` 在最前面调用 `guardStdio()`；`daemon.ts` 用 `forwardToStderr` 代替 `pipe`。
- 刻意不动：alterd 自己的日志（`main.rs` 已用 `log_internal_errors(false)` 容忍 stderr 断开）；不加日志文件；不处理「e2e 留下孤儿 app」（另记为风险）。

## Spec Delta: app-shell

### ADDED Requirement: R1 stdout / stderr 断开时照常运行
系统 SHALL 在 app 主进程的 stdout / stderr 管道读端关闭后照常运行：主进程写日志或发出进程警告不产生未捕获异常，并继续读走 alterd 写到 stderr 的日志。

#### Scenario: R1.S1 写日志不产生未捕获异常
- WHEN app 已启动且窗口显示 `alterd 运行中`，关闭 app 的 stdout 与 stderr 管道的读端，之后主进程执行 `console.log`、`console.error` 并发出一条进程警告（`process.emitWarning`）
- THEN 1 秒内主进程没有未捕获异常（`uncaughtExceptionMonitor` 没有记录）
- AND 主进程在 2 秒内响应一次 `app.evaluate`

#### Scenario: R1.S2 继续读走 alterd 的 stderr
- WHEN `ALTER_ALTERD_PATH` 指向一个假 alterd，app 显示 `alterd 运行中` 后关闭 app 的 stderr 管道的读端，假 alterd 随后向 stderr 写出 256 KB
- THEN 从让它开始写起 3 秒内写完（它在写完后建出标记文件）

## 风险与回滚
- 影响范围只在 Electron 主进程的日志输出：stdout / stderr 正常时行为不变（照样输出，alterd 日志的转发与原来的 `pipe` 一样有背压，读得慢时 alterd 等待而不是在主进程里堆积）；只有 stderr 断开后才改为读走丢弃。回滚即还原 `stdio.ts`、`index.ts`、`daemon.ts` 三处。
- 孤儿 app：被测试或终端遗弃的 app 仍会留在桌面上（只是不再弹窗）。如需根治，另开 chore 让 e2e 在用例结束时强制回收。

## 假设
- 写失败直接忽略、不另行记录：读端关闭时日志已无处可写，额外的日志文件超出这次修复的范围。
- 回归测试放在 Electron e2e（`apps/browser/e2e/stdio.spec.ts`）：只有真实的 Electron 主进程与管道才能复现，没有更小的 seam。
- 用例标题以场景 ID 加 `stdio` 开头（`R1.S1 stdio：…`），`--grep` 不会选中已存档需求里同名的 `R1.S1`。

## 待确认问题
- 无。

## 澄清记录
### Session 2026-10-21
- Q: 修复放在 add-core-features 里，还是单独的 fix change？ → A: 单独的 fix change，从 main 拉分支。
- Q: 出错时 app 是怎么启动的？ → A: app 未退出，只是弹窗报错（未说明启动方式；诊断发现是 e2e 遗留的孤儿实例）。

## 变更记录
- 2026-10-21: 创建。
- 2026-10-21: amended: 「修复范围」与「风险与回滚」改为与实现一致——alterd 日志的转发在 stderr 正常时保留背压、断开后才读走丢弃（评审 CR1 引入背压，验证指出文档仍写「不再有背压」）；场景与任务不变; revision: 2; re-approved by the user
