---
change: fix-orphan-alterd
---

# 执行文档：app 异常结束后 alterd 残留，重启显示「已退出」

## Repo: alter

- [ ] T1 [covers:R1.S1,R1.S2] 先用失败的回归测试复现，再给 `alterd run` 加 `--exit-on-stdin-close` — seam: `alterd` 二进制（`std::process::Command` + 临时 `ALTER_HOME`，管道 stdin） — verify: `cargo test -p alterd --test lifecycle exit_on_stdin`
  - 文件：`crates/alterd/src/main.rs`（clap `Run` 变体加 `#[arg(long)] exit_on_stdin_close: bool`，传给 `lifecycle::run`）、`crates/alterd/src/lifecycle.rs`（在 `tokio::select!` 里加第三个分支）、`crates/alterd/tests/lifecycle.rs`（新增 `exit_on_stdin_r1_s1_…`、`exit_on_stdin_r1_s2_…`，沿用文件里的 `Daemon` 守卫与等待健康文件的写法）。
  - 读 stdin 必须用独立的 `std::thread`：它阻塞读到 EOF，再经 `tokio::sync::oneshot`（需给根 Cargo.toml 的 tokio 加 `sync` feature）通知主循环。不要用 `tokio::io::stdin`：它在阻塞线程池里读，无法取消，会让 SIGTERM 路径上的运行时关闭一直挂住。
  - 退出路径和 SIGTERM 共用：删除健康文件，写 `code=0` 的 JSON 日志，`signal` 字段写 `stdin-eof`。
  - R1.S2 要用 `Stdio::null()` 证明不带参数时立即 EOF 也不退出。顺序：先写 R1.S1 并看它失败（参数不存在，clap 退出码 2），再写实现；R1.S2 在实现之前就该通过，记为 BASELINE。
  - 现有 `r3_*` 用例的 stdin 为默认继承，不带新参数，不受影响；跑完整个 `--test lifecycle` 确认。
- [ ] T2 [covers:R2.S1] [after:T1] app 以管道 stdin 和 `--exit-on-stdin-close` 拉起 alterd — seam: Playwright `_electron.launch`（临时 `ALTER_HOME`，`app.process().kill("SIGKILL")`） — verify: `bun run e2e --grep "R2\\.S1"`
  - 文件：`apps/browser/src/main/daemon.ts`（`spawn(path, ["run", "--exit-on-stdin-close"], { stdio: ["pipe", "ignore", "pipe"] })`，并保留 `child.stdin` 的引用直到退出；不要对它调用 `end()`）、新文件 `apps/browser/e2e/orphan.spec.ts`（用例标题以场景 ID 开头，沿用 `e2e/app.ts` 的 `launchApp`、`waitUntil`、`isAlive`）。
  - SIGKILL 之后 Playwright 的 `app.close()` 会报错，用例里不要再调用它；用 `waitUntil(2000, () => !isAlive(pid))` 断言。
  - `e2e/fixtures/slow-alterd.sh` 会收到新参数，它不解析参数，不需要改；跑完整个 e2e 确认「R7.S1 迟到就绪」仍通过。
- [ ] T3 [covers:R3.S1] [after:T2] 退出码 3 时复用已在运行的实例 — seam: Playwright `_electron.launch`（同一临时 `ALTER_HOME` 里先用 `child_process.spawn` 拉起 `target/debug/alterd run`） — verify: `bun run e2e --grep "R3\\.S1"`
  - 文件：`apps/browser/src/main/daemon.ts`（`exit` 处理：`code === 3` 时用现有的 `healthPid(home)` 读 `alterd.json` 里的 pid，pid 存活则 `setStatus({ state: "running", pid, … })` 并标记为外部实例；`stop()` 对外部实例直接返回，不发信号、不删健康文件）、`apps/browser/e2e/orphan.spec.ts`。
  - pid 存活判断用 `process.kill(pid, 0)`，`EPERM` 也算存活，与 alter-core 的 `health::is_alive` 和 `e2e/app.ts` 的 `isAlive` 口径一致；`healthPid` 已经处理了文件缺失和 JSON 损坏，不要另写一份读取逻辑。
  - 用例结束时要自己杀掉外部拉起的 alterd，避免在临时目录外留下进程。
  - 跑完整个 e2e 与 `bun run ci:check`，确认 R7.S1–S3 仍通过。
