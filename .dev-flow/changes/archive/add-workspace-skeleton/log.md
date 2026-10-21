---
change: add-workspace-skeleton
---

# 执行日志：工程骨架：双工作区、核心存储与三进程启动

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

### T1 建立 Cargo workspace 与三个 crate 骨架
- Revision: 1
- VERIFY: `cargo fmt --check` → 无输出，退出 0
- VERIFY: `cargo clippy --all-targets -- -D warnings` → Checking alter-core / alter-cli / alterd v0.1.0，Finished `dev` profile，无 warning
- VERIFY: `cargo test` → alter-core 1 passed（tests::version_matches_workspace），alter 与 alterd 各 0 tests，全部 ok
- VERIFY: `cargo metadata --format-version 1 --no-deps | jq '.workspace_members | length'` → 3
- VERIFY: config.json `test` 回填为 `cargo test`；`dflow.py init --update-agents-md` → AGENTS.md action updated，dev-flow 块测试命令变为 `cargo test`

### T2 Store 打开即迁移：0001 建表、WAL 与外键
- Revision: 1
- RED: `cargo test -p alter-core --test store` → FAIL: r1_s1_fresh_open_creates_schema panicked：`首次打开应当成功: Unimplemented`（桩实现，打开行为缺失） (R1.S1)
- GREEN: `cargo test -p alter-core --test store` → PASS (1 test)，`Store::open` 建目录、开库、执行 0001 并写 schema_version (R1.S1)
- RED: `cargo test -p alter-core --test store` → FAIL: r1_s2_reopen_is_idempotent panicked：`再次打开应当成功: Sqlite(... msg: "table schema_version already exists" ...)` (R1.S2)
- GREEN: `cargo test -p alter-core --test store` → PASS (2 tests)，迁移运行器读取当前版本，只执行更高版本的迁移 (R1.S2)
- RED: `cargo test -p alter-core --test store` → FAIL: r1_s3_wal_and_foreign_keys：`assertion left == right failed, left: "delete"`（journal_mode 未切到 WAL） (R1.S3)
- GREEN: `cargo test -p alter-core --test store` → PASS (3 tests)，`Store::open` 在迁移前 `pragma_update` journal_mode=WAL、foreign_keys=ON (R1.S3)
- VERIFY: Widen `cargo fmt --check` 退出 0；`cargo clippy --all-targets -- -D warnings` 无 warning；`cargo test` → alter-core 单测 1 passed、tests/store.rs 3 passed，其余 0 tests，全部 ok

### T3 Bun workspace 与 TS 门禁，回填 config.json
- Revision: 1
- VERIFY: `bun install` → 依赖安装完成（typescript 6.0.3、eslint 10.13.0、@eslint/js 10.0.1、typescript-eslint 8.72.0、@types/bun 1.4.3）
- VERIFY: 第一次 `bun run lint` → 退出 2：`Error: typescript-eslint does not support TS 7.0.`（bun 默认装了 typescript 7.0.2）；`bun add -d typescript@~6.0` 后 `bun run lint` 退出 0
- VERIFY: `bunx eslint . --format json` 实际检查了 eslint.config.js、packages/protocol/src/index.ts、packages/sdk/src/index.ts、packages/sdk/tests/smoke.test.ts
- VERIFY: `bun run typecheck`（`tsc --noEmit -p tsconfig.json`）→ 退出 0；`--listFilesOnly` 列出 packages 下 3 个 .ts 与 sdk/package.json
- VERIFY: `bun test` → 1 pass，0 fail，Ran 1 test across 1 file（packages/sdk/tests/smoke.test.ts）
- VERIFY: config.json 回填 test=`cargo test && bun test`、lint=`cargo fmt --check && cargo clippy --all-targets -- -D warnings && bun run lint`、typecheck=`bun run typecheck`、build=`cargo build`；`dflow.py init --update-agents-md` → AGENTS.md updated；`cargo fetch` 退出 0
- VERIFY: Widen `cargo test && bun test` → cargo 全部 ok（alter-core 1 + store 3），bun 1 pass；lint 三段全部退出 0；typecheck 退出 0

### T4 协议类型 ErrorCode / HealthFile 与 export-ts
- Revision: 1
- RED: `cargo test -p alter-core --test export_ts` → FAIL: r2_s1_export_error_code_and_health_file panicked：`读取 .../ErrorCode.ts 失败：No such file or directory`（export-ts 为空桩，导出行为缺失） (R2.S1)
- GREEN: `cargo test -p alter-core --test export_ts` → PASS (1 test)，`protocol::export_ts` 用 ts-rs `export_all` 写出 ErrorCode.ts（14 个字面量）与 HealthFile.ts（version: string, pid: number, startedAt: number, dbPath: string） (R2.S1)
- VERIFY: `bun run protocol:export` 生成 packages/protocol/src/generated/{ErrorCode,HealthFile}.ts；`bun run typecheck` 退出 0
- VERIFY: 把生成文件时间戳改到 2020 年后再跑 `bun run --silent protocol:export` → 退出 0，文件被重写（mtime 更新），shasum 与之前一致，导出幂等
- VERIFY: `export-ts` 指向不存在的多级目录 → 退出 0，目录被创建并写出两个文件
- VERIFY: Widen `cargo fmt --check` 退出 0；clippy 无 warning；`cargo test` → alter-core 单测 1、export_ts 1、store 3 全部 ok；`bun test` 1 pass；`bun run lint`、`bun run typecheck` 退出 0；仓库内没有生成 `bindings/` 目录

### T5 alterd run：建库、健康文件、单实例、SIGTERM 清理退出
- Revision: 1
- RED: `cargo test -p alterd --test lifecycle` → FAIL: r3_s1_run_writes_health_file：`1 秒内没有出现 alterd.json`（alterd 为空 main，立即退出） (R3.S1)
- GREEN: `cargo test -p alterd --test lifecycle` → PASS (1 test)，`alterd run` 解析 ALTER_HOME、`Store::open`、原子写 alterd.json 后挂起 (R3.S1)
- RED: `cargo test -p alterd --test lifecycle` → FAIL: r3_s2_sigterm_exits_cleanly：`退出状态：ExitStatus(unix_wait_status(15)) left: None right: Some(0)`（没有信号处理，被 SIGTERM 直接杀死） (R3.S2)
- GREEN: `cargo test -p alterd --test lifecycle` → PASS (2 tests)，tokio 单线程运行时，写文件前注册 SIGTERM / SIGINT，收到后删除 alterd.json 并以 0 退出 (R3.S2)
- RED: `cargo test -p alterd --test lifecycle` → FAIL: r3_s3_second_instance_is_rejected：`第二个实例应当在 2 秒内退出`（第二个实例覆盖文件后一直运行） (R3.S3)
- GREEN: `cargo test -p alterd --test lifecycle` → PASS (3 tests)，健康文件存在时 stderr 输出 `alterd: already running (pid <n>)` 并以 3 退出 (R3.S3)
- RED: `cargo test -p alterd --test lifecycle` → FAIL: r3_s4_stale_health_file_is_taken_over：`1 秒内 alterd.json 没有改写为新进程的 pid，当前内容：{... "pid":999999 ...}`（只要文件存在就拒绝） (R3.S4)
- GREEN: `cargo test -p alterd --test lifecycle` → PASS (4 tests)，`health::is_alive` 用 `kill(pid, 0)`（EPERM 视为存活），只有记录的 pid 存活才拒绝 (R3.S4)
- REFACTOR: alterd 接入 tracing JSON 行日志（启动含 pid / version / db_path / schema_version，收到的信号，退出码）→ `cargo test -p alterd --test lifecycle` PASS (4 tests)；手动 `ALTER_HOME=<scratchpad> target/debug/alterd run` 后 SIGTERM，exit=0，stderr 三行 JSON 日志，健康文件已删除
- VERIFY: `cargo test -q -p alterd --test lifecycle` 连跑 5 遍，每遍 4 passed（1.41–1.63 s）
- VERIFY: Widen 第一次 `cargo fmt --check` 报 paths.rs 与 lifecycle.rs 有格式差异；`cargo fmt` 后 `cargo fmt --check` 通过；clippy 无 warning；`cargo test` 共 9 passed（alter-core 1、export_ts 1、store 3、lifecycle 4）；`bun test` 1 pass；`bun run lint`、`bun run typecheck` 退出 0

### T6 alter --version 与 alter status [--json]
- Revision: 1
- RED: `cargo test -p alter-cli --test cli` → FAIL: r4_s1_prints_version：`Unexpected stdout ... code=0 stdout=""`（空 main 不输出版本） (R4.S1)
- GREEN: `cargo test -p alter-cli --test cli` → PASS (1 test)，clap `#[command(name = "alter", version)]` 输出 `alter 0.1.0` (R4.S1)
- RED: `cargo test -p alter-cli --test cli` → FAIL: r5_s1_status_running：`Unexpected return code ... code=2`，stderr `error: unexpected argument 'status' found` (R5.S1)
- GREEN: `cargo test -p alter-cli --test cli` → PASS (2 tests)，`status [--json]` 读健康文件输出 `alterd running (pid <n>, version 0.1.0)` 与单行 JSON (R5.S1)
- RED: `cargo test -p alter-cli --test cli` → FAIL: r5_s2_status_without_health_file：`Unexpected return code, failed var == 1 ... code=101`，stderr `健康文件不存在`（缺文件时 panic） (R5.S2)
- GREEN: `cargo test -p alter-cli --test cli` → PASS (3 tests)，缺文件输出 `not running` / `{"running":false}`，退出 1 (R5.S2)
- RED: `cargo test -p alter-cli --test cli` → FAIL: r5_s3_status_with_stale_health_file：`code=0 stdout="alterd running (pid 999999, version 0.1.0)\n"`（未检查 pid 存活） (R5.S3)
- GREEN: `cargo test -p alter-cli --test cli` → PASS (4 tests)，复用 `health::is_alive`，残留输出 `not running (stale)` / `{"running":false,"stale":true}`，退出 1 (R5.S3)
- REFACTOR: status.rs 改为先算 `State`（Missing / Stale / Running）再统一输出文本或 JSON → `cargo test -p alter-cli --test cli` PASS (4 tests)
- VERIFY: Widen `cargo fmt --check` 退出 0；clippy 无 warning；`cargo test` → cli 4、alter-core 单测 1、export_ts 1、store 3、lifecycle 4 全部 ok；`bun test` 1 pass；`bun run lint`、`bun run typecheck` 退出 0

### T7 Electron + electron-vite + SolidJS 空窗口，Playwright e2e 运行器
- Revision: 1
- VERIFY: `bun add`（apps/browser）安装 solid-js 1.9.17、electron 44.9.1、electron-vite 6.0.0、electron-builder 26.15.3、vite 8.3.4、vite-plugin-solid 2.11.14、@playwright/test 1.64.1、@types/node；electron 包没有 install 脚本，`dist/` 不存在；手动 `node install.js` 用时约 25 s 从 GitHub 下载，`Electron --version` → v44.9.1
- VERIFY: 首次 `bun run e2e --grep R6` → `/bin/bash: electron-vite: command not found`（bun 隔离链接，二进制只在 apps/browser/node_modules/.bin）；脚本移到 apps/browser/package.json、根脚本用 `--cwd` 转发后可运行；再遇 `SyntaxError: Cannot use 'import.meta' outside a module`（apps/browser 为 CJS 包），改用 `__dirname`。这两次都是运行器搭建问题，不是 RED
- RED: `bun run e2e --grep R6` → FAIL: `TimeoutError: electronApplication.firstWindow: Timeout 10000ms exceeded while waiting for event "window"`（主进程只 whenReady，不建窗口） (R6.S1)
- GREEN: `bun run e2e --grep R6` → `✓ R6.S1 窗口打开：标题为 alter，页面可见文本含 alter (871ms)`，1 passed (R6.S1)
- VERIFY（变异）：把 App.tsx 的 `<h1>alter</h1>` 改为 `placeholder` 后 `bun run e2e --grep R6` → 1 failed，失败在 `expect(win.getByText("alter", { exact: true })).toBeVisible()`；恢复后 1 passed（变异未提交）
- VERIFY: 第一次 Widen `bun test` → 1 pass、1 fail、1 error（bun 扫到了 e2e/window.spec.ts）；新增 bunfig.toml `[test] root = "packages"` 后 `bun test` → 1 pass，Ran 1 test across 1 file
- VERIFY: Widen `cargo test -q` 共 13 passed、0 failed；`cargo fmt --check` 通过；clippy 无 warning；`bun run lint` 退出 0（eslint 检查 12 个文件，含 apps/browser 下 8 个，0 错误）；`bun run typecheck` 退出 0（第二段 tsc 覆盖 apps/browser 8 个文件）
- VERIFY: config.json build 改为 `cargo build && bun run build`，运行一次退出 0（electron-vite 三段 built）

### T8 Electron Main 托管 alterd
- Revision: 1
- RED: `bun run e2e --grep R7` → FAIL: R7.S1：`expect(received).toBe(expected) Expected: true Received: false`，3 秒内没有 alterd.json（主进程尚未拉起 alterd） (R7.S1)
- VERIFY（调试）：实现 spawn 后 R6.S1 与 R7.S1 都在 `electronApplication.close` 卡到 30 s 超时（`DEBUG=pw:api` 显示 close started 后无返回）。假设：alterd 继承了 Electron 的 stderr，退出时没被关掉，管道不关导致 Playwright 等不到 close。诊断实验把 stderr 改为 ignore：close 立即返回、R6.S1 1 passed，但 `pgrep` 看到残留的 `target/debug/alterd run`（pid 97414），假设成立；实验改动已还原，残留进程已清理
- GREEN: `bun run e2e` → R7.S1 ✓ (922ms)、R6.S1 ✓，2 passed；alterd 的 stderr 改为管道并由主进程转发，spawn 后每 100 ms 轮询健康文件，pid 匹配即推送 `daemon.status` running，渲染进程显示 `alterd 运行中` (R7.S1)
- RED: `bun run e2e --grep R7` → FAIL: R7.S2：`Expected: true Received: false`（daemon.spec.ts:45，关闭 app 后 2 秒内 alterd 仍存活） (R7.S2)
- GREEN: `bun run e2e` → 3 passed，before-quit 拦下第一次退出，`DaemonSupervisor.stop()` 发 SIGTERM，2 秒未退则 SIGKILL 并删除健康文件，之后再 `app.quit()`；`pgrep` 无残留 alterd (R7.S2)
- RED: `bun run e2e --grep R7.S3` → FAIL: `electronApplication.firstWindow failed +30s`（`ALTER_ALTERD_PATH=/nonexistent/alterd` 时 spawn 失败未处理，窗口事件都等不到） (R7.S3)
- GREEN: `bun run e2e` → 4 passed，`resolveAlterdPath` 设置了 ALTER_ALTERD_PATH 时只认它、不存在即返回 null，状态 missing，渲染进程显示 `alterd 未找到`；同时监听子进程 error 与提前 exit (R7.S3)
- VERIFY: `bun run e2e` 连跑 3 遍，每遍 4 passed（7.2–7.4 s），之后 `pgrep` 无残留 alterd 或 Electron 进程
- VERIFY: 按任务 verify 命令 `bun run e2e --grep R7` → R7.S1 ✓、R7.S2 ✓、R7.S3 ✓，3 passed
- VERIFY: Widen `bun run lint`、`bun run typecheck` 退出 0；`bun test` 1 pass；`cargo fmt --check` 通过；clippy 无 warning；`cargo test -q` 共 13 passed、0 failed

### T9 README、LICENSE、仓库卫生文件、CI 与 ci:check / version:check / dev 脚本
- Revision: 1
- VERIFY: `bun run ci:check` → 退出 0：fmt 通过、clippy 无 warning、cargo test 全部 ok（cli 4、alter-core 1、export_ts 1、store 3、lifecycle 4）、eslint 与两段 tsc 退出 0、bun test 1 pass、`版本号一致：0.1.0`
- VERIFY: 把 apps/browser/package.json 的 version 临时改成 0.1.1 → `bun run version:check` 退出 1，打印三处的值；改回后退出 0（改动未提交）
- VERIFY: `brew install shellcheck`（0.11.0）后 `shellcheck scripts/version-check.sh` 退出 0，无告警
- VERIFY: README 示例在临时 ALTER_HOME 下运行：`alter --version` → `alter 0.1.0`；无 alterd 时 `status --json` → `{"running":false}`、退出 1；alterd 运行时 `status --json` → `{"running":true,"pid":5818,"version":"0.1.0","dbPath":"…/alter.db"}`、退出 0，`status` → `alterd running (pid 5818, version 0.1.0)`；SIGINT 后 alterd 退出 0
- VERIFY: `ALTER_HOME=<临时目录> bun run dev` 后台启动 → electron-vite 输出 `dev server running for the electron renderer process`，alterd 写出健康文件（pid 6707）；对 electron-vite 发 SIGINT 后 3 s 内无残留进程，健康文件已删除，只剩 alter.db
- VERIFY: `.github/workflows/ci.yml` 只能在推送 PR 后由 GitHub Actions 运行，本地未执行；它跑的就是上面通过的 `bun run ci:check`

### T10 文档与代码对齐
- Revision: 1
- VERIFY: AGENTS.md 更新现状、命令表（运行 / 开发、构建、全部门禁）、单个测试与命令行入口、技术栈（Electron 44、electron-vite 6、TypeScript 6.0 的原因、bun 隔离链接）、目录树（标出待建立的目录，补 scripts/version-check.sh、.github/workflows、bunfig.toml）、验证与测试；共 104 行
- VERIFY: infra 文档：components.md 说明已有代码的组件与真实路径；overview.md 测试命令去掉 TBD；architecture.md 横切关注点补「进程存活：alterd.json」一条
- VERIFY: AGENTS.md 新列出的命令各跑一次：`cargo test -p alter-core --test store r1_s1` → 1 passed、2 filtered out；`bun test packages/sdk/tests/smoke.test.ts` → 1 pass；`bun install` → no changes；`cargo fetch` 退出 0（其余命令在 T7–T9 的证据里已跑过）
- VERIFY: `python3 -I /Users/bytedance/.claude/skills/dev-flow/scripts/dflow.py validate` → errors 0，agents_md 与 infra 无告警；唯一 warning 为 REV002（全部任务尚未评审）

### T11 alterd 用文件锁保证单实例（评审 CR1）
- Revision: 1
- RED: `cargo test -q -p alterd --test lifecycle r3_s3_concurrent` 连跑 3 遍 → 每遍 FAIL：`assertion left == right failed: 退出状态：ExitStatus(unix_wait_status(256))`；手动复现同时启动 4 个，其中一个 stderr 为 `写健康文件失败: No such file or directory`（撞上别人的临时文件）并退出 1，其余同时运行 (R3.S3)
- GREEN: `cargo test -q -p alterd --test lifecycle` → 5 passed（新增 r3_s3_concurrent_start_leaves_one_instance 与原有 R3.S1–R3.S4），再连跑 5 遍均 5 passed；alterd 在开库前对 `<ALTER_HOME>/alterd.lock` 调 `File::try_lock()` 并持有到退出，拿不到锁即输出 `alterd: already running (pid <n>)` 并退出 3 (R3.S3)
- VERIFY: Widen `cargo fmt --check` 通过；clippy 无 warning；`cargo test -q` 共 14 passed、0 failed；`bun run e2e` 4 passed，无残留 alterd

### T12 重新生成不绑定镜像地址的 bun.lock（评审 CR2）
- Revision: 1
- VERIFY: 修改前 `grep -c bnpm.byted.org bun.lock` → 465
- VERIFY: 第一次尝试删锁后用 `BUN_CONFIG_REGISTRY=https://registry.npmjs.org/ bun install` 重新解析：地址清空，但解析出 asn1js 3.1.0、pkijs 3.4.2 等公司镜像尚未同步的新版本；从零 `bun install --frozen-lockfile` 报 `failed to download pkijs@3.4.2: 404 Not Found`、`Failed to install 2 packages`，而退出码仍为 0。放弃这个做法
- VERIFY: 改为恢复原锁文件，只把每条记录的镜像 URL 替换为空串（版本与 integrity 不变）→ `grep -c bnpm.byted.org bun.lock` 为 0
- VERIFY: 删除 node_modules 后用用户级镜像 `bun install --frozen-lockfile` → 849 packages installed，无失败；在临时副本里用空缓存（`BUN_INSTALL_CACHE_DIR` 指向新目录）与公共源 `BUN_CONFIG_REGISTRY=https://registry.npmjs.org/` 做 `bun install --frozen-lockfile` → 849 packages installed [8.28s]，真实下载 223 MB，无失败
- VERIFY: `! grep -q bnpm.byted.org bun.lock && bun install --frozen-lockfile && bun run ci:check` → 退出 0（cargo 各测试 ok、bun test 1 pass、版本号一致）；`bun run e2e` → 4 passed

### T13 就绪轮询不设截止时间（评审 CR3）
- Revision: 1
- RED: `bun run e2e --grep 迟到就绪` → FAIL: `expect(locator).toBeVisible() failed ... Timeout: 8000ms`，页面快照为 `paragraph: alterd 启动中`（假 alterd 3.5 秒后才写健康文件，轮询 3 秒到期后状态卡死） (R7.S1)
- GREEN: `bun run e2e --grep R7` → 4 passed，含 `R7.S1 迟到就绪 ✓ (4.3s)`；轮询改为持续到看见健康文件或子进程退出 (R7.S1)
- VERIFY: 假 alterd `apps/browser/e2e/fixtures/slow-alterd.sh` 过 `shellcheck`；测试结束后 `pgrep` 无残留进程
- VERIFY: Widen `bun run lint`、`bun run typecheck` 退出 0；`bun test` 1 pass；`bun run e2e` 5 passed

### T14 alterd 错误退出只输出结构化日志（评审 CR5）
- Revision: 1
- VERIFY: 修改前 `cargo build -q -p alterd && ALTER_HOME=/dev/null target/debug/alterd run` → exit=1，stderr 两行：第 1 行 JSON（无 code 字段），第 2 行纯文本 `alterd: 创建数据目录失败: File exists (os error 17)`
- VERIFY: 修改后同一命令 → exit=1，stderr 只有 1 行，可解析为 JSON，`fields.code` 为 1
- VERIFY: Widen `cargo fmt --check` 通过；clippy 无 warning；`cargo test -q` 共 14 passed、0 failed（R3.S3 仍断言 already running 的纯文本行）

### T15 R7.S2 从发起关闭开始计时（评审 CR6）
- Revision: 1
- VERIFY（证明旧测试太弱）：变异 `stop()` 把 SIGTERM 推迟 3 秒（实际在 2 秒时走 SIGKILL 兜底），旧测试 `bun run e2e --grep R7.S2` 仍然 ✓ (2.8s)
- RED (mutation: 让 stop 推迟 3 秒才发 SIGTERM): `bun run e2e --grep R7.S2` → FAIL: `expect(...).toBeLessThanOrEqual(2000) Expected: <= 2000 Received: 2160`（新测试从发起关闭时计时） (R7.S2)
- GREEN: 恢复 daemon.ts（`grep -c 变异` 为 0，`git diff` 无改动）后 `bun run e2e --grep R7.S2` 连跑 3 遍 → 每遍 ✓（741–757 ms） (R7.S2)
- VERIFY: Widen `bun run e2e` 5 passed；`bun run lint`、`bun run typecheck` 退出 0；无残留进程

### T17 拒绝第二实例时输出带退出码的 JSON 日志（第 1 次验证差距）
- Revision: 1
- RED: `cargo test -q -p alterd --test lifecycle` → FAIL: r3_s3_second_instance_is_rejected panicked：`不是 JSON 行：alterd: already running (pid 45939)` (R3.S3)
- GREEN: `cargo test -q -p alterd --test lifecycle` → 5 passed，再连跑 3 遍均 5 passed；纯文本 `eprintln!` 改为 `tracing::warn!(code = 3, pid, "alterd: already running (pid <n>)")` (R3.S3)
- VERIFY: 手动在临时 ALTER_HOME 下启动两个 alterd，第二个 → exit=3，stderr 一行 `{"level":"WARN","fields":{"message":"alterd: already running (pid 46308)","code":3,"pid":"46308"},...}`
- VERIFY: Widen `cargo fmt --check` 通过；clippy 无 warning；`cargo test -q` 共 14 passed、0 failed；`bun run e2e` 5 passed

### T16 design.md 同步评审后的实现（第 1 次验证差距）
- Revision: 1
- VERIFY: design.md 更新组件表（export-ts 的导出方式、alterd 的实例锁、sdk 的 `SDK_NAME`）、架构一节的数据目录说明（新增 `alterd.lock`）、接口契约（单实例改为 `alterd.lock` 排他锁，stderr 只有 JSON 行，退出码 1 / 3 带 `code`，CLI 仍用健康文件加 `kill(pid, 0)`）、决策表（单实例与存活判断）、风险表（Electron 按需下载、bun.lock 不带地址、stderr 管道、就绪轮询不限时、R7.S2 从发起关闭计时）；seq 图改为取锁与持续轮询
- VERIFY: `grep -n "SDK_VERSION\|最多 3 s\|trustedDependencies: \[" design.md` 无结果；`dflow diagrams sync add-workspace-skeleton` → updated design.md、无错误；`python3 -I /Users/bytedance/.claude/skills/dev-flow/scripts/dflow.py validate add-workspace-skeleton` → errors 0（warnings 为 REV002 / REV003：T16、T17 尚未评审）
- VERIFY: requirement.md 未改，revision 仍为 1

## 决策与偏差
- 起始状态（2026-10-20，run 首次进入）：分支 `feature/add-workspace-skeleton`，HEAD `3ba82e1`，工作区干净；config.json 四个命令为空，仓库无代码，因此没有可运行的基线测试，不存在预先失败的用例。
- 本机 `cargo` 为 Homebrew 版 1.98.1，PATH 中排在 rustup 代理之前，`rust-toolchain.toml` 对本机不生效，只对 rustup 用户与 CI 生效；fmt 与 clippy 由 Homebrew 版提供（rustfmt 1.9.0、clippy 0.1.98）。
- T1 顺带新建 `.gitignore`（只有 `/target`），避免构建产物出现在工作区状态里；T9 负责补全其余条目。
- 依赖版本取自公司 crates 镜像索引的最新稳定版，集中写在根 `Cargo.toml` 的 `[workspace.dependencies]`；成员 crate 尚未引用的依赖不会被编译。
- T2：`0001_init.sql` 去掉了 data-model.md DDL 开头的两条 `PRAGMA`。WAL 不能在事务内切换、外键开关只对当前连接有效，按 design.md「迁移运行器规则」改由 `Store::open` 在迁移事务之外设置；建表语句原样保留。
- T2：任务子条目里的 `paths.rs`（`ALTER_HOME` 解析）推迟到 T5，由 alterd 的场景测试驱动；T2 的 seam `Store::open(&Path)` 直接接收目录，不读环境变量。
- 后续（follow-up）：data-model.md 提到升级前自动备份 `alter.db.bak-<version>`，本需求只有 0001、没有升级路径，留给第一个新增迁移的需求实现。
- T3：TypeScript 固定为 `~6.0`。typescript-eslint 8.72 的 peer 依赖是 `typescript >=4.8.4 <6.1.0`，bun 默认装的 7.0.2（Go 原生版）会让 eslint 直接报错退出。等 typescript-eslint 支持 TS 7 后再升级（follow-up）。
- T3：typecheck 脚本用 `tsc --noEmit -p tsconfig.json`，没有用任务子条目写的 `tsc -b`：project references 要求各包 `composite` 并产出声明文件，而这些包都不需要编译产物。apps/browser 的 main / renderer 运行环境不同，T7 给它单独的 tsconfig 并追加到 typecheck 脚本。
- T3：sdk 占位导出 `SDK_NAME`（冒烟测试比对 package.json 的 name），没有按子条目写 `SDK_VERSION = "0.1.0"`，以守住「版本号只出现在三处」的跨任务约束；packages/protocol 与 packages/sdk 的 package.json 也不写 version。
- T3：config.json 的 `build` 暂为 `cargo build`，`bun run build` 脚本由 T7 建立后再追加；`.gitignore` 追加 `node_modules/`。
- T4：类型上不加 `#[ts(export)]`，由 `export-ts` 显式调用 `export_all` 写到指定目录，`cargo test` 因而不会往仓库写 `bindings/`，任务子条目里「测试设 `TS_RS_EXPORT_DIR`」的做法不再需要。
- T4：ts-rs 默认把 `i64` 映射为 `bigint`，健康文件是 JSON 数字，`started_at` 标 `#[ts(type = "number")]`；R2.S1 的测试同时断言四个字段的 TS 类型（design.md「接口契约」的值），防止以后漂移成 bigint。
- T4：错误码用逐个 `#[serde(rename = "E_…")]`，没有用 `rename_all = "SCREAMING_SNAKE_CASE"`：后者生成不带 `E_` 前缀的字面量。
- T5：`alter_core::paths::alter_home()` 在这里落地（从 T2 推迟过来），由 R3.S1 的测试经 `ALTER_HOME` 驱动。`health` 模块（read / write / remove / is_alive）放在 alter-core，T6 的 CLI 复用。
- T5：健康文件内容无法解析时 `health::read` 返回 `None`，alterd 视同没有文件并覆盖。规格没有覆盖这种损坏文件的情况，这是最保守的处理：不会因为半截文件拒绝启动。
- T5：测试判断进程是否在运行用 `Child::try_wait`，判断健康文件里的 pid 用 JSON 解析结果直接比对，没有复用被测的 `health::read`，避免镜像断言。
- T6：测试用 `Command::new(env!("CARGO_BIN_EXE_alter"))` 构造 assert_cmd 命令，没有用子条目写的 `Command::cargo_bin`（新版 assert_cmd 已不推荐），也没有引入 `predicates`：stdout 全文比对与 JSON 解析比对已足够。
- T6：R5.S1 的文本输出断言整行 `alterd running (pid <n>, version 0.1.0)`（design.md「接口契约」），比规格里「包含 running」更严，避免 `not running` 之类的输出误判为通过。
- T7：没有加 `.npmrc` 的 `electron_mirror`，也去掉了 `trustedDependencies`。Electron 44 已没有 postinstall，改为首次解析 `electron` 路径时按需下载（electron/index.js），本机直连 GitHub 可用；网络受限时设环境变量 `ELECTRON_MIRROR=https://npmmirror.com/mirrors/electron/`（已验证该镜像有 44.9.1）。
- T7：bun 使用隔离链接，apps/browser 依赖的二进制只在它自己的 `node_modules/.bin`。`build`、`e2e` 脚本写在 apps/browser/package.json，根 package.json 用 `bun run --cwd apps/browser …` 转发；`bun run e2e --grep R6` 的参数会一路追加到 playwright 命令末尾。
- T7：apps/browser/package.json 不设 `"type": "module"`，让 electron-vite 产出 CommonJS 的 main 与 preload（`sandbox: true` 下 preload 只能是 CJS）；e2e 公共函数因此用 `__dirname`。
- T7：新增根 `bunfig.toml`，`bun test` 只扫 packages/，避免把 Playwright 的 `*.spec.ts` 当成 bun 测试；`typecheck` 脚本追加 `tsc --noEmit -p apps/browser/tsconfig.json`（DOM + node 类型、Solid JSX）。
- T7：macOS 26.6.2 上 Playwright 启动 Electron 不需要 `--no-sandbox`，`ALTER_E2E_ARGS` 保留为可选的追加参数。
- T7：窗口全部关闭即退出 app（`window-all-closed` → `app.quit()`），macOS 惯常的 Dock 常驻行为留给后续 UI 需求。`.gitignore` 追加 `out/`、`test-results/`、`playwright-report/`。
- T8：e2e 公共函数 `launchApp` 默认给每次启动一个新的临时 `ALTER_HOME`。从本任务起 app 会真正拉起 alterd，不这样做 R6.S1 会写到真实数据目录 `~/Library/Application Support/alter`。
- T8：alterd 的 stderr 用管道由主进程转发（`stdio: ["ignore","ignore","pipe"]`），没有直接继承。继承会让 alterd 持有主进程的 stdio，主进程退出被拖住，见 T8 的调试记录。
- T8：渲染进程先订阅 `daemon.status` 推送再 `invoke` 取当前值，同一通道名两种用法（ipcMain.handle 与 webContents.send），避免页面加载晚于状态推送时漏掉状态。主进程与渲染进程共用的类型放在 `apps/browser/src/shared/daemon.ts`。
- T8：`exited` 状态（alterd 提前退出，如第二实例退出码 3）按 design.md 接口契约实现，但没有场景覆盖；`stop()` 对 spawn 失败（没有 pid、不会有 exit 事件）直接返回，避免 app 无法退出。这两处都没有专门测试，交给评审确认。
- T8：apps/browser 的 `e2e` 脚本先 `cargo build -q -p alterd`，保证 `target/debug/alterd` 是最新的。
- T9：LICENSE 的版权人按任务子条目取 `git config user.name`；后按用户要求改为 ethanyulabs，仓库本地 git 身份同时设为 `ethanyulabs <ethanyulabs@gmail.com>`。
- T9：顺手从根 Cargo.toml 的 `[workspace.dependencies]` 删掉未使用的 `predicates`（T6 没有用到它）。
- T9：`dev` 脚本写在 apps/browser（`electron-vite dev`），根脚本先 `cargo build -q -p alterd` 再转发，原因同 T7 的 bun 隔离链接。
- 评审第 1 轮：codex 把 CR1 建议为 blocker，我定为 major。两个写连接下 SQLite 自身的锁保证不会损坏数据，实际后果是健康文件被误删与单写者约束被打破，属于很可能的并发 bug；无论哪个等级都通过收敛任务 T11 修复。CR4 codex 建议 major，我定为 minor 并接受，理由见评审表。
- 评审第 1 轮：codex 在只读沙箱里拿不到 Cargo 构建锁，没有运行 Rust 测试、clippy 与 e2e；这些检查在每个任务的 Widen 证据里已运行，验证阶段会再全量跑一遍。
- T11：单实例判定从 design.md「接口契约」写的「读到 alterd.json 且 kill(pid, 0) 成功即视为在运行」改为「alterd.lock 排他锁」。对外契约不变：退出码 3、stderr `already running`、残留健康文件被覆盖。数据目录里多了一个 `alterd.lock`，存档时同步到 infra 文档。`health::is_alive` 仍由 alter CLI 使用。
- T12：bun.lock 的条目不再带下载地址，安装时走各自配置的源（本机为 `~/.npmrc` 的公司镜像，CI 为公共 npm）。以后 `bun add` 会把镜像地址重新写进锁文件，提交前需要再清一次；后续可以加一条检查（follow-up）。
- T12：`bun install` 在部分包下载失败（404）时退出码仍为 0，只看退出码判断安装成功不可靠，需要同时看输出里有没有 `Failed to install`。
- 评审第 2 轮：codex 确认 CR1、CR2、CR3、CR5、CR6 已修复，无新发现。它提出两个疑问，核实后不作为发现，记为后续项（follow-up）：其一，lifecycle.rs 里 `drop(lock)` 早于 `Store` 析构，锁没覆盖连接关闭的最后一刻，此时旧实例已不再写库或碰健康文件，没有实际后果，可改为按声明逆序自然析构；其二，「迟到就绪」用例只靠 `app.close()` 收尾，没有单独兜底杀假 alterd，此前观察到 Playwright 关闭失败时会连同子进程一起清理，且每次运行后 `pgrep` 均无残留。
- 评审第 2 轮：codex 只读核对代码，没有重跑测试；全部检查在验证阶段重新运行。
- 评审第 3 轮：codex 确认 design.md 的 5 处差距与退出码 3 的 JSON 日志都已消除，无新发现。它指出 design.md 三处措辞可以更精确，记为后续项（follow-up）：「stderr 只输出 JSON 行」只覆盖 tracing 初始化之后（clap 参数错误仍是 clap 自己的纯文本）；「取不到锁即退出 3」应限定为锁被占用，其他取锁错误走退出码 1；seq 图没画「就绪前子进程退出 → exited」分支。
- 2026-10-20 存档前：按用户要求把 main 与本分支全部提交的作者改为 `ethanyulabs <ethanyulabs@gmail.com>`，内容不变，提交哈希随之变化。下文评审范围里的旧哈希对应：4bdffef→ffa8e9d、8406b7a→cafa3c0、486079b→c576b02、643a397→ec68548、98cd3c3→c6aec55、1efd086→9bcc0c9、3ba82e1→e5976ba。

## 代码评审
### Round 1
- Revision: 1
- Scope: alter `git diff 4bdffef...8406b7a`（排除 Cargo.lock、bun.lock、packages/protocol/src/generated 与 .dev-flow/；bun.lock 另行检查）; reviewer: codex
- Tasks: T1-T10
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR1 | major | correctness | crates/alterd/src/lifecycle.rs:25 | 单实例检查与写健康文件之间有竞态：两个 `alterd run` 同时启动都读不到健康文件，会同时运行并各自持有 SQLite 写连接，任一退出还会删掉另一个的健康文件，使 CLI 误报未运行（R3.S3） | fixed in R2 |
| CR2 | major | spec | bun.lock:45 | 465 个包的下载地址写死为公司镜像 bnpm.byted.org，GitHub Actions 与外部贡献者无法按锁文件安装，违反「不把公司镜像配置提交进仓库」的跨任务约束 | fixed in R2 |
| CR3 | major | correctness | apps/browser/src/main/daemon.ts:83 | 就绪轮询 3 秒到期只清定时器不更新状态，alterd 启动超过 3 秒后即使健康文件出现，界面也永远停在「alterd 启动中」（R7.S1） | fixed in R2 |
| CR4 | minor | correctness | apps/browser/src/main/daemon.ts:93 | alterd 写出健康文件后崩溃，关闭 app 时 `stop()` 直接返回，留下残留的 alterd.json | accepted: 残留健康文件是规格覆盖的状态，alterd 下次启动会接管（R3.S4），alter status 报 stale（R5.S3），不影响功能 |
| CR5 | minor | spec | crates/alterd/src/main.rs:33 | 错误退出时先打一行 JSON 日志，又用 eprintln 打一行纯文本，JSON 行日志被混入非 JSON 行，且错误日志没有退出码字段（可观测性 NFR） | fixed in R2 |
| CR6 | major | tests | apps/browser/e2e/daemon.spec.ts:43 | R7.S2 的 2 秒计时从 `app.close()` 返回后才开始，而 close 本身会等待 `stop()`，关闭过程耗时超过 2 秒测试仍会通过，没有证明「2 秒内」（R7.S2） | fixed in R2 |
| CR7 | nit | maintainability | packages/protocol/src/generated/HealthFile.ts:6 | ts-rs 生成的文件有尾随空格 | accepted: 生成文件禁止手改，eslint 已忽略该目录 |

Review: CHANGES REQUESTED

### Round 2
- Revision: 1
- Scope: alter `git diff 486079b...643a397`（第 1 轮之后的改动，排除 .dev-flow/；bun.lock 全文核对）; reviewer: codex
- Tasks: T11-T15
Findings: none.

Review: PASS

### Round 3
- Revision: 1
- Scope: alter `git diff 98cd3c3...1efd086`（第 1 次验证之后的改动，含 design.md 与 seq 图）; reviewer: codex
- Tasks: T16-T17
Findings: none.

Review: PASS
<!-- 由评审步骤填写，每次评审一轮：
### Round <n>
- Revision: <当前 revision>
- Scope: <repo> `git diff <base>...HEAD`（含未提交的改动）
- Tasks: T1, T2, …（这一轮读过代码的任务；收敛任务和重新打开的任务需要新的一轮）
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
最后恰好一行 "Review: PASS" 或 "Review: CHANGES REQUESTED"。语法见 references/formats.md -->

## 验证报告
<!-- 每次验证追加一份新报告，放在最近一轮评审之后。以 "- Revision: <当前 revision>" 和 "- Review: Round <最近一轮>" 开头。由验证步骤填写：本仓库的测试结果，每个场景一行 "| R<n>.S<m> | <path::test> | PASS |"，成功标准对照表，收敛缺口，fix 类型还要写 "Fix: verified | partial | failed"，最后恰好一行 "Overall: READY" 或 "Overall: NOT READY"。 -->

### 第 1 次验证
- Revision: 1
- Review: Round 2
- Verifier: codex

| Repo | Command | Result |
|---|---|---|
| alter | `cargo test && bun test` | PASS（codex 运行：cargo 14 passed、0 failed；bun 1 pass） |
| alter | `cargo fmt --check && cargo clippy --all-targets -- -D warnings && bun run lint` | PASS（codex 运行：无 warning / error） |
| alter | `bun run typecheck` | PASS（codex 运行：两段 tsc 无错误） |
| alter | `cargo build && bun run build` | PASS（codex 运行：dev profile 与 electron-vite 三段构建成功） |
| alter | `bun run e2e` | PASS（本机重跑 5 passed；codex 沙箱内 5 个用例都报 `Process failed to launch!`，崩溃栈停在 `NSApplication sharedApplication`，属沙箱不允许开窗口） |

| Scenario | Test | Result |
|---|---|---|
| R1.S1 | crates/alter-core/tests/store.rs::r1_s1_fresh_open_creates_schema | PASS |
| R1.S2 | crates/alter-core/tests/store.rs::r1_s2_reopen_is_idempotent | PASS |
| R1.S3 | crates/alter-core/tests/store.rs::r1_s3_wal_and_foreign_keys | PASS |
| R2.S1 | crates/alter-core/tests/export_ts.rs::r2_s1_export_error_code_and_health_file | PASS |
| R3.S1 | crates/alterd/tests/lifecycle.rs::r3_s1_run_writes_health_file | PASS |
| R3.S2 | crates/alterd/tests/lifecycle.rs::r3_s2_sigterm_exits_cleanly | PASS |
| R3.S3 | crates/alterd/tests/lifecycle.rs::r3_s3_second_instance_is_rejected + r3_s3_concurrent_start_leaves_one_instance | PASS |
| R3.S4 | crates/alterd/tests/lifecycle.rs::r3_s4_stale_health_file_is_taken_over | PASS |
| R4.S1 | crates/alter-cli/tests/cli.rs::r4_s1_prints_version | PASS |
| R5.S1 | crates/alter-cli/tests/cli.rs::r5_s1_status_running | PASS |
| R5.S2 | crates/alter-cli/tests/cli.rs::r5_s2_status_without_health_file | PASS |
| R5.S3 | crates/alter-cli/tests/cli.rs::r5_s3_status_with_stale_health_file | PASS |
| R6.S1 | apps/browser/e2e/window.spec.ts::R6.S1 窗口打开 | PASS |
| R7.S1 | apps/browser/e2e/daemon.spec.ts::R7.S1 启动拉起 alterd + R7.S1 迟到就绪 | PASS |
| R7.S2 | apps/browser/e2e/daemon.spec.ts::R7.S2 退出关闭 alterd | PASS |
| R7.S3 | apps/browser/e2e/daemon.spec.ts::R7.S3 alterd 二进制缺失 | PASS |

| SC | Check | Result |
|---|---|---|
| SC1 | `bun run ci:check` → 退出 0 | PASS |
| SC2 | `bun run protocol:export && git diff --exit-code packages/protocol/src/generated` → 退出 0，diff 为空 | PASS |
| SC3 | `hyperfine --warmup 3 'target/release/alter --version'` → mean 4.8 ms ± 1.2 ms（444 runs） | PASS（< 20 ms） |
| SC4 | 场景表 16 行 | PASS（16 / 16） |

Gaps:
- contradicts：design.md 与评审后的实现不一致，共 5 处：单实例判定（design.md:88 写健康文件 + pid，实现为 alterd.lock 排他锁）、就绪轮询（design.md:131 写最多 3 s，实现持续轮询）、SDK 占位导出（design.md:69 写 SDK_VERSION，实现为 SDK_NAME）、协议导出机制（design.md:61 写 `#[ts(export)]`，实现显式 export_all）、Electron 下载（design.md:159 写 .npmrc 镜像与 trustedDependencies，实现均未设置）。偏差都已记在「决策与偏差」，但设计正文未同步 → T16
- partial：可观测性 NFR 要求退出码写进 JSON 行日志，alterd 拒绝第二实例（退出码 3）时只输出纯文本一行（crates/alterd/src/lifecycle.rs:36），没有 JSON 日志与 code 字段 → T17

Overall: NOT READY

### 第 2 次验证
- Revision: 1
- Review: Round 3
- Verifier: codex

| Repo | Command | Result |
|---|---|---|
| alter | `cargo test && bun test` | PASS （codex 运行：cargo 14 passed、0 failed；bun 1 pass） |
| alter | `cargo fmt --check && cargo clippy --all-targets -- -D warnings && bun run lint` | PASS （codex 运行：无 warning / error） |
| alter | `bun run typecheck` | PASS （codex 运行：两段 tsc 无错误） |
| alter | `cargo build && bun run build` | PASS （codex 运行：dev profile 与 electron-vite 三段构建成功） |
| alter | `bun run e2e` | PASS （本机运行 5 passed；codex 沙箱不能开窗口，按约定跳过） |
| alter | `python3 -I /Users/bytedance/.claude/skills/dev-flow/scripts/dflow.py validate add-workspace-skeleton` | PASS （errors 0、warnings 0） |

| Scenario | Test | Result |
|---|---|---|
| R1.S1 | crates/alter-core/tests/store.rs::r1_s1_fresh_open_creates_schema | PASS |
| R1.S2 | crates/alter-core/tests/store.rs::r1_s2_reopen_is_idempotent | PASS |
| R1.S3 | crates/alter-core/tests/store.rs::r1_s3_wal_and_foreign_keys | PASS |
| R2.S1 | crates/alter-core/tests/export_ts.rs::r2_s1_export_error_code_and_health_file | PASS |
| R3.S1 | crates/alterd/tests/lifecycle.rs::r3_s1_run_writes_health_file | PASS |
| R3.S2 | crates/alterd/tests/lifecycle.rs::r3_s2_sigterm_exits_cleanly | PASS |
| R3.S3 | crates/alterd/tests/lifecycle.rs::r3_s3_second_instance_is_rejected + r3_s3_concurrent_start_leaves_one_instance | PASS |
| R3.S4 | crates/alterd/tests/lifecycle.rs::r3_s4_stale_health_file_is_taken_over | PASS |
| R4.S1 | crates/alter-cli/tests/cli.rs::r4_s1_prints_version | PASS |
| R5.S1 | crates/alter-cli/tests/cli.rs::r5_s1_status_running | PASS |
| R5.S2 | crates/alter-cli/tests/cli.rs::r5_s2_status_without_health_file | PASS |
| R5.S3 | crates/alter-cli/tests/cli.rs::r5_s3_status_with_stale_health_file | PASS |
| R6.S1 | apps/browser/e2e/window.spec.ts::R6.S1 窗口打开 | PASS |
| R7.S1 | apps/browser/e2e/daemon.spec.ts::R7.S1 启动拉起 alterd + R7.S1 迟到就绪 | PASS |
| R7.S2 | apps/browser/e2e/daemon.spec.ts::R7.S2 退出关闭 alterd | PASS |
| R7.S3 | apps/browser/e2e/daemon.spec.ts::R7.S3 alterd 二进制缺失 | PASS |

| SC | Check | Result |
|---|---|---|
| SC1 | `bun run ci:check` → 退出 0 | PASS |
| SC2 | `bun run protocol:export && git diff --exit-code packages/protocol/src/generated` → 退出 0，diff 为空 | PASS |
| SC3 | `hyperfine --warmup 3 'target/release/alter --version'` → mean 5.6 ms ± 1.3 ms（478 runs） | PASS （< 20 ms） |
| SC4 | 场景表 16 行 | PASS （16 / 16） |

宪章与 NFR：规格先行、测试先行、简单优先、边界契约、证据优先均满足；性能 5.6 ms；alterd 运行时 `lsof -nP -a -p <pid> -i` 无网络套接字，健康文件无密钥；stderr 每行均为 JSON，退出码 0 / 1 / 3 都有 `code` 字段；兼容性在 macOS 26.6.2 arm64 验证，Intel 与更早版本不在范围。GitHub Actions 需要推送后在托管 runner 上运行，本地已跑过它执行的 `bun run ci:check`。

Gaps: none.

Overall: READY
