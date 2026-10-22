---
change: add-core-features
---

# 执行文档：agent 叠层与接管交还

<!-- 行语法：- [ ] T<n> [covers:R1.S1] [after:T<k>] [P] <做什么> — seam: <测试驱动的公开入口> — verify: <命令>
对应的证据写进 log.md 之后才能勾选 [x]。完整语法见 references/formats.md -->

## Repo: alter

- [x] T1 [covers:R9.S1,R9.S2,R9.S3] 协议：`ownershipReturned` / `userTookControl` 事件、`OwnershipChanged` 通知类型 — seam: `export-ts`（`crates/alter-core/tests/export_ts.rs`） — verify: `cargo test -p alter-core --test export_ts`
  - `crates/alter-core/src/protocol/shell.rs` 加两个 `ShellEvent` 变体；`gateway.rs` 加 `OwnershipChanged {id, ownership, reason}` 与 `TakeOverResult`。跑 `bun run protocol:export`。
- [x] T2 [covers:R1.S1,R2.S1,R2.S2,R3.S1,R4.S1,R4.S2,R5.S1,R6.S1,R6.S2] [after:T1] ownership 状态机、强制执行、取消、超时 — seam: Gateway WS + Shell 假客户端（`crates/alterd/tests/gateway.rs`） — verify: `cargo test -p alterd --test gateway n3_`
  - 新文件 `crates/alterd/src/task_space/ownership.rs`。检查点在队列执行器取出命令之后、执行之前（design.md「架构」）。
  - R5.S1 用 `page.waitFor` 测取消：没有 Chromium 时 `waitFor` 也要先等到 target，所以本任务的测试用一个假 CDP 端（复用 N2 T14 的假 CDP WS 服务，放在 `crates/alterd/tests/common/fake_cdp.rs`），让选择器永远查不到。
  - R6 的阈值：`ALTER_HANDOFF_TIMEOUT_MS` 优先，其次 config.toml 的 `handoff_timeout_minutes`，默认 30 分钟；检查周期取 `min(阈值 / 3, 60 s)`。
  - R2.S1 的回执日志断言：从 `Daemon` 收集的 stderr 里找 `method=page.click`、`error_code=E_USER_CONTROL` 的行。
- [x] T3 [covers:R7.S1,R7.S2,R7.S3] [after:T1] [P] SDK：`handOff`、`waitForControl`、`takeOverTaskSpace` — seam: SDK 公开 API 对假 Gateway（`packages/sdk/tests/handoff.test.ts`） — verify: `bun test packages/sdk/tests/handoff.test.ts`
  - 复用 N2 T6 的假 Gateway（抽到 `packages/sdk/tests/fake-gateway.ts`）；新增「主动推送通知」的能力。
  - R7.S3 与 N2 的 R20.S2 是同一条规则在 `goto` 上的体现，测试要断言请求次数恰好为 1。
  - 独立性：只写 `packages/sdk/`。
- [x] T4 [covers:R1.S2,R3.S2,R5.S2] [after:T2,T3] 接管横幅、交还按钮、停止 Agent — seam: Electron e2e（`apps/browser/e2e/handoff.spec.ts`，经 `alter nodejs` 驱动） — verify: `bun run e2e --grep "N3 R[135]\."`
  - Renderer：`HandoffBanner.tsx`（`data-testid="handoff-banner"`、按钮 `handoff-return`）、Sidebar 顶部 `stop-agent`；Main：IPC `taskSpace.return` / `taskSpace.stop` 转为 Shell 事件。
  - 横幅只显示当前 Space 内 ownership 为 `user` 的 TaskSpace；多个时逐条显示。
- [x] T5 [covers:R8.S1,R8.S2,R8.S3] [after:T2] agent 叠层 — seam: Electron e2e，经 CDP 转发端点读页面 DOM（`apps/browser/e2e/handoff.spec.ts`） — verify: `bun run e2e --grep "N3 R8\."`
  - 新文件 `crates/alterd/src/cdp/overlay.rs` 与叠层脚本 `crates/alterd/src/cdp/overlay.js`（`include_str!`）。
  - 测试读 shadow DOM：用 Playwright `connectOverCDP` 拿到页面后 `page.evaluate(() => document.querySelector("alter-agent-overlay").shadowRoot.querySelector("[data-role=action]").textContent)`。
  - R8.S2：经 CDP 转发端点对按钮中心 `page.mouse.click(x, y)`，断言页面正文出现 `submitted:`。
  - 截图前隐藏叠层（design.md「风险」），对 N2 的截图测试 R15.S1 再跑一遍，确认仍然通过。

## Convergence

- [x] T6 [after:T5] `taskSpace.cdp` 在用户控制期间回 `E_USER_CONTROL`，并随转为用户控制被取消（CR1） — seam: Gateway WS + 假 CDP 端（`crates/alterd/tests/gateway.rs`） — verify: `cargo test -p alterd --test gateway n3_cr1_`
- [x] T7 [after:T5] CDP 转发端点：有 TaskSpace 处于用户控制时拒绝新连接（423），转为用户控制时断开已有的转发连接（CR2） — seam: Gateway WS + 假 CDP 端，经转发端点的原始 TCP（`crates/alterd/tests/gateway.rs`） — verify: `cargo test -p alterd --test gateway n3_cr2_`
- [x] T8 [after:T5] `taskSpace.release` / `finish` 在用户控制期间回 `E_USER_CONTROL`，页面与状态不变（CR3） — seam: Gateway WS（`crates/alterd/tests/gateway.rs`） — verify: `cargo test -p alterd --test gateway n3_cr3_`
- [x] T9 [after:T5] 契约写明：激活标签被别的 TaskSpace 托管时 `takeOver` 的 `userPage` 为 null；补双 TaskSpace 的测试（CR4） — seam: Gateway WS + Shell 假客户端（`crates/alterd/tests/gateway.rs`） — verify: `cargo test -p alterd --test gateway n3_cr4_`
- [x] T10 [after:T6] 原始 CDP 透传（`taskSpace.cdp`、`page.cdp`）在有任何 TaskSpace 处于用户控制时回 `E_USER_CONTROL`（CR5） — seam: Gateway WS + 假 CDP 端（`crates/alterd/tests/gateway.rs`） — verify: `cargo test -p alterd --test gateway n3_cr5_`
- [x] T11 [after:T7] TabService 写 ownership=user 时同步发出转发切断信号；转发连接在切断信号优先的 `select!` 里写请求头与转发（CR6） — seam: Gateway WS + 假 CDP 端，经转发端点的原始 TCP（`crates/alterd/tests/gateway.rs`） — verify: `cargo test -p alterd --test gateway n3_cr`
- [x] T12 [after:T10] 原始 CDP 透传在检查之前订阅全局切断信号，发送以切断优先：检查通过后、发送之前发生的接管也让它回 `E_USER_CONTROL`（CR7） — seam: Gateway WS + 假 CDP 端 + Shell 假客户端（`crates/alterd/tests/gateway.rs`） — verify: `cargo test -p alterd --test gateway n3_cr7_`
  - 构造交错：A 的 `newPage` 得到 p2（还没上报 targetId），A 对 p2 发 `page.cdp`（在 `session()` 里等 target），此时 B handOff，再替 p2 上报 targetId：`page.cdp` 回 `E_USER_CONTROL`，假 CDP 端没有收到它的命令。
- [x] T13 [after:T11] 把「alterd 运行在单线程运行时」写成断言与注释（TabService 与转发任务依赖它），design.md 写明接管时刻的论证（CR8） — seam: alterd 单测（`crates/alterd/src/gateway/forward.rs`） — verify: `cargo test -p alterd --bin alterd forward::`
- [x] T14 [after:T12] CdpMux 写出队列的原始命令带接管信号，写出任务发送前检查，接管后丢弃（CR9） — seam: alterd 单测（`crates/alterd/src/cdp/mod.rs`，假 CDP 端） — verify: `cargo test -p alterd --bin alterd cdp::`
- [x] T15 [after:T14] design.md 写明接管边界的定义：开始写出即发出，接管前已开始写出的算接管前发出（CR10，设计修正） — seam: 无行为改动 — verify: `cargo build -p alterd`
- [x] T16 [after:T15] agent 调用产生的全部 CDP 命令都带接管信号（task-local，`CdpMux` 未显式给出时取它）；TaskSpace 的取消信号与全局接管信号都由 TabService 在写 ownership 的同一步递增（CR11） — seam: alterd 单测（`crates/alterd/src/cdp/mod.rs`）；Gateway WS + 假 CDP 端（`crates/alterd/tests/gateway.rs`） — verify: `cargo test -p alterd --bin alterd cdp:: && cargo test -p alterd --test gateway n3_`
- [x] T17 [after:T16] R1.S2、R8.S1、R8.S3 的时限从发起动作（启动脚本之前）开始计，脚本与界面检查同时进行、满足时刻不晚于截止时间；R8.S1 另断言叠层 `display` 为 block、`visibility` 为 visible（验证缺口） — seam: Electron e2e（`apps/browser/e2e/handoff.spec.ts`） — verify: `bun run e2e --grep "N3 R(1\.S2|8\.)"`
- [x] T18 [after:T17] R3.S2、R5.S2 的 1 秒从点击之前开始计（单调时钟），横幅状态满足后断言不晚于截止时间；顺带核对 handoff.spec.ts 里其余带时限的断言（验证缺口） — seam: Electron e2e（`apps/browser/e2e/handoff.spec.ts`） — verify: `bun run e2e --grep "N3 R[135]\.S2"`
- [x] T19 [after:T18] 接管信号与写库在同一个同步步骤里发出：单元测试 + design.md 去掉旧描述（CR12） — seam: `crates/alterd/src/tabs/ownership.rs` 的单元测试（进程内的 TabService） — verify: `cargo test -p alterd --bin alterd tabs::ownership`
  - 进程内建 TabService（临时目录的 Store、`CdpMux::new`、空的通知与请求通道），建一个 TaskSpace，订阅全局与该 TaskSpace 的接管信号后调用 `task_space_hand_off`；返回时两个信号都已变化（`has_changed()` 为 true，中间没有 await）。
  - design.md「实现细节（T2）」的取消一条改成：TabService 写 ownership=user 的同一步递增该 TaskSpace 与全局的接管信号。
  - 变异：把 `self.takeover.fire(row.id)` 挪到 Gateway 转发 `ownershipChanged` 通知之前（异步分发里），用例必须失败。
- [x] T20 [after:T18] 已入队、因背压还没写出的守卫命令，在接管后被写出任务丢弃（CR13） — seam: `crates/alterd/src/cdp/mod.rs` 的单元测试 — verify: `cargo test -p alterd --bin alterd cdp::`
  - 假 CDP 端握手后先不读；发一条 12 MB 的普通命令把写出任务堵在 `sink.send`；之后 `send_guarded` 一条小命令（入队时信号未变）；再触发接管；放开读取：只收到大命令与之后的标记命令，守卫命令不出现，调用方得到 `Interrupted`。写出任务若没被堵住，守卫命令会在接管前写出、用例失败，所以只会保守地失败。
  - 变异：写出任务不检查接管信号（检查挪到入队之前），用例必须失败。
- [x] T21 [after:T20] 背压测试显式确认守卫命令已入队再触发接管（CR14） — seam: `crates/alterd/src/cdp/mod.rs` 的单元测试 — verify: `cargo test -p alterd --bin alterd cdp::`
  - 等 `conn.pending` 里有两条再触发接管，替换固定的 `sleep`。
  - 变异：接管检查挪到入队之前、写出任务不再检查，用例必须失败。

