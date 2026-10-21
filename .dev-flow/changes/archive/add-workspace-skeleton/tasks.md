---
change: add-workspace-skeleton
---

# 执行文档：工程骨架：双工作区、核心存储与三进程启动

<!-- 执行文档。每个任务是一个薄的垂直切片，能由一个测试单独证明。
行语法：- [ ] T<n> [covers:R1.S1] [after:T<k>] [P] <做什么> — seam: <测试驱动的公开入口> — verify: <命令>
有变更的场景用 RED/GREEN，已有行为用 BASELINE，没有 covers 的任务用 GREEN/VERIFY。
子条目写涉及的文件、跨任务必须遵守的约束、风险与检查项、并行时各自占用的文件和资源。
[P] 在执行前要先做独立性检查；共享的证据和任务勾选只由协调者记录。
对应的证据写进 log.md 之后才能勾选 [x]。完整语法见 references/formats.md -->

跨任务约束（每个任务都要遵守）：
- 版本号 `0.1.0` 只出现在 `Cargo.toml` 的 `[workspace.package]`、根 `package.json`、`apps/browser/package.json` 三处；Rust 代码用 `env!("CARGO_PKG_VERSION")`，不写死。
- 错误码列表以 `.dev-flow/infra/api.md` 的 14 个为准，不增不减。
- 只有 alterd 打开 `alter.db`；CLI 与 Electron Main 只读 `alterd.json`。
- 所有写 `ALTER_HOME` 的测试都用 `tempfile::tempdir()` 或 Playwright 的临时目录，不碰 `~/Library/Application Support/alter`。
- 注释与文档用中文；标识符、命令、路径不翻译。
- 不把公司镜像配置（cargo、npm）提交进仓库。

## Repo: alter

- [x] T1 建立 Cargo workspace 与三个 crate 骨架，Rust 门禁可跑 — verify: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
  - 文件：`Cargo.toml`（`[workspace] members = ["crates/*"]`，`resolver = "3"`，`[workspace.package] version = "0.1.0", edition = "2024", license = "MIT"`，`[workspace.dependencies]` 集中声明依赖版本）；`rust-toolchain.toml`（`channel = "stable"`，components rustfmt、clippy）；`rustfmt.toml`（`edition = "2024"`）；`crates/alter-core/{Cargo.toml,src/lib.rs}`、`crates/alterd/{Cargo.toml,src/main.rs}`、`crates/alter-cli/{Cargo.toml,src/main.rs}`（`[[bin]] name = "alter"`）。
  - `alter-core/src/lib.rs` 放一个 `pub fn version() -> &'static str { env!("CARGO_PKG_VERSION") }` 与一个单元测试断言它等于 `"0.1.0"`，让 `cargo test` 有东西可跑（VERIFY 证据：三条命令的真实输出）。
  - 回填 `.dev-flow/config.json` 的 `test` 为 `cargo test`（临时，T3 再改成双侧），并运行 `python3 -I /Users/bytedance/.claude/skills/dev-flow/scripts/dflow.py init --update-agents-md` 刷新 AGENTS.md 的 dev-flow 块。
  - 检查：`cargo metadata --format-version 1 | jq '.workspace_members | length'` 为 3。

- [x] T2 [covers:R1.S1,R1.S2,R1.S3] [after:T1] Store 打开即迁移：`ALTER_HOME` 解析、0001 建表、WAL 与外键 — seam: `alter_core::store::Store::open(&Path)` — verify: `cargo test -p alter-core --test store`
  - 文件：`crates/alter-core/src/paths.rs`（`pub fn alter_home() -> PathBuf`：`ALTER_HOME` 或 `~/Library/Application Support/alter`）；`crates/alter-core/src/store/mod.rs`（`pub struct Store { conn: rusqlite::Connection }`，`Store::open(home: &Path) -> Result<Store, StoreError>`，`pub fn db_path(home) -> PathBuf`，`pub fn conn(&self) -> &Connection` 供测试查 PRAGMA）；`crates/alter-core/src/store/migrate.rs`（`const MIGRATIONS: &[(u32, &str)]`，`include_str!("../../migrations/0001_init.sql")`）；`crates/alter-core/migrations/0001_init.sql`（从 `.dev-flow/infra/data-model.md` 的 ```sql 块原样复制，含索引）；测试 `crates/alter-core/tests/store.rs`，三个测试函数名含场景 ID（`r1_s1_fresh_open_creates_schema` 等）。
  - 依赖：`rusqlite = { version = "0.3x", features = ["bundled"] }`、`thiserror`、`tempfile`（dev）。
  - 顺序：先写三个测试（`Store::open` 不存在 → 编译失败不算 RED；先放一个返回 `Err(StoreError::Unimplemented)` 的桩，让 RED 是断言失败），再实现。
  - 坑：`PRAGMA foreign_keys` 是每连接设置，必须在 `open` 里执行；`journal_mode=WAL` 要在迁移前设；`schema_version` 表本身由 0001 创建，迁移运行器要容忍表不存在（版本 0）；DDL 里的 `CREATE INDEX` 与 `CREATE TABLE` 在一个事务里。
  - 目录不存在时 `create_dir_all`。

- [x] T3 [after:T1] 建立 Bun workspace 与 TS 门禁，回填 config.json 四个命令并刷新 AGENTS.md — verify: `bun install && bun run lint && bun run typecheck && bun test`
  - 文件：根 `package.json`（`"name": "alter-monorepo", "private": true, "version": "0.1.0", "workspaces": ["apps/*", "packages/*"]`，脚本 `lint: eslint .`、`typecheck: tsc -b`、`test: bun test`，其余脚本由 T7 / T9 补）；`tsconfig.base.json`（strict、moduleResolution bundler、target ES2022）；根 `tsconfig.json`（references 指向各包）；`eslint.config.js`（typescript-eslint 推荐规则，忽略 `packages/protocol/src/generated`、`out/`、`dist/`）；`.bun-version`（`1.3.14`）；`packages/protocol/{package.json,tsconfig.json,src/index.ts}`（名字 `@alter/protocol`，`src/index.ts` 先为空导出，T4 填）；`packages/sdk/{package.json,tsconfig.json,src/index.ts,tests/smoke.test.ts}`（名字 `@alter/sdk`，导出 `SDK_VERSION = "0.1.0"`，测试断言它）。
  - 回填 `.dev-flow/config.json`：`test` = `cargo test && bun test`；`lint` = `cargo fmt --check && cargo clippy --all-targets -- -D warnings && bun run lint`；`typecheck` = `bun run typecheck`；`build` = `cargo build && bun run build`（`build` 脚本由 T7 建，此处先写上）。然后运行 `python3 -I /Users/bytedance/.claude/skills/dev-flow/scripts/dflow.py init --update-agents-md`，并把 AGENTS.md「命令」表里的「待建立」去掉（安装、测试、Lint、类型检查三行；运行 / 开发与构建两行留到 T7 / T9）。
  - 坑：npm registry 为公司镜像（用户级 `.npmrc`），仓库里的 `.npmrc` 只放 `electron_mirror`（T7）；`bun test` 默认只找 `*.test.ts`，把测试放 `tests/` 下。
  - VERIFY 证据：四条命令的真实输出，`bun test` 至少 1 个测试通过。

- [x] T4 [covers:R2.S1] [after:T2,T3] 协议类型 `ErrorCode` / `HealthFile` 与 `export-ts` 导出到 packages/protocol — seam: `cargo run -p alter-core --bin export-ts -- <dir>` — verify: `cargo test -p alter-core --test export_ts && bun run typecheck`
  - 文件：`crates/alter-core/src/protocol/mod.rs`、`error.rs`（`#[derive(Serialize, Deserialize, TS)] #[serde(rename_all = "SCREAMING_SNAKE_CASE")] #[ts(export)] pub enum ErrorCode { Unauthorized, ProtocolVersion, SpaceNotFound, TaskSpaceNotFound, PageNotFound, PageBudget, UserControl, AmbiguousSelector, NoMatch, Timeout, DialogOpen, ImportSourceNotFound, KeychainDenied, Internal }`）、`health.rs`（`HealthFile`，`#[serde(rename_all = "camelCase")] #[ts(rename_all = "camelCase")]`）；`crates/alter-core/src/bin/export_ts.rs`（参数 `<dir>`，对每个类型调用 `T::export_all_to(dir)`）；测试 `crates/alter-core/tests/export_ts.rs`（`Command::new(env!("CARGO_BIN_EXE_export-ts"))` 指向 tempdir，读回两个文件断言字面量与字段）；`packages/protocol/src/generated/{ErrorCode.ts,HealthFile.ts}`（生成后提交）、`packages/protocol/src/index.ts`（`export type { ErrorCode } from "./generated/ErrorCode"; export type { HealthFile } from "./generated/HealthFile";`）；根 `package.json` 加脚本 `protocol:export: cargo run -p alter-core --bin export-ts -- packages/protocol/src/generated`。
  - 依赖：`ts-rs = { version = "10", features = ["serde-compat"] }`、`serde`、`serde_json`。
  - 坑：ts-rs 的 `#[ts(export)]` 在 `cargo test` 时也会按 `TS_RS_EXPORT_DIR` 导出，测试里显式设 `TS_RS_EXPORT_DIR` 到 tempdir 以免污染仓库；生成文件头有 ts-rs 的注释行，SC2 比对的是整个目录，`protocol:export` 必须幂等。
  - RED 判定：断言 `ErrorCode.ts` 包含 14 个字面量，先只写 13 个枚举值看它失败（或先不写 `#[ts(export)]` 看文件不存在）。

- [x] T5 [covers:R3.S1,R3.S2,R3.S3,R3.S4] [after:T2,T4] `alterd run`：建库、写健康文件、单实例、SIGTERM 清理退出 — seam: `alterd` 二进制（assert_cmd + 临时 `ALTER_HOME`） — verify: `cargo test -p alterd --test lifecycle`
  - 文件：`crates/alterd/src/main.rs`（clap：子命令 `run`；tracing-subscriber JSON 到 stderr）；`crates/alterd/src/lifecycle.rs`（`pub async fn run(home: PathBuf) -> Result<ExitCode>`：读旧 `alterd.json` → `alter_core::health::is_alive(pid)` 为真则 stderr `alterd: already running (pid <n>)` 返回 3；`Store::open`；写 `HealthFile`（临时文件 + rename）；`tokio::signal::unix` 等 SIGTERM / SIGINT；删除文件；返回 0）；`crates/alter-core/src/health.rs`（`write(home, &HealthFile)`、`read(home) -> Option<HealthFile>`、`is_alive(pid) -> bool` 用 `libc::kill(pid, 0)`，`EPERM` 也算存活）；测试 `crates/alterd/tests/lifecycle.rs`：四个测试，用 `std::process::Command::new(env!("CARGO_BIN_EXE_alterd")).arg("run").env("ALTER_HOME", dir)`，轮询文件（每 50 ms，上限 1 s），`libc::kill(pid, SIGTERM)` 后 `wait_timeout`（用 `wait-timeout` crate 或自旋 `try_wait`）。
  - 依赖：`tokio = { features = ["rt", "macros", "signal", "time"] }`、`clap = { features = ["derive"] }`、`tracing`、`tracing-subscriber = { features = ["json"] }`、`libc`、`anyhow`；dev：`tempfile`、`wait-timeout`。
  - 坑：测试结束必须杀掉子进程（`Drop` 守卫 SIGKILL），否则残留 alterd 会让后续测试串扰；R3.S3 的第一实例由测试自己拉起；R3.S4 的残留文件用 pid 999999 手写。
  - 不监听任何端口；不写 `gateway.json`。

- [x] T6 [covers:R4.S1,R5.S1,R5.S2,R5.S3] [after:T5] `alter --version` 与 `alter status [--json]` — seam: `alter` 二进制（assert_cmd + 临时 `ALTER_HOME`） — verify: `cargo test -p alter-cli --test cli`
  - 文件：`crates/alter-cli/src/main.rs`（clap `#[command(name = "alter", version)]`，子命令 `status { --json }`）；`crates/alter-cli/src/status.rs`（调 `alter_core::health::read` 与 `is_alive`，三态输出与退出码见 design.md「接口契约」）；测试 `crates/alter-cli/tests/cli.rs`：`assert_cmd::Command::cargo_bin("alter")`；R5.S1 用 `std::process::id()` 作为存活 pid 手写 `alterd.json`（理由见 design.md 决策表）；R5.S2 空目录；R5.S3 pid 999999。
  - 依赖：`clap`、`serde_json`、`alter-core`；dev：`assert_cmd`、`predicates`、`tempfile`。
  - 坑：clap 的 `--version` 默认输出 `<name> <version>`，`name` 必须显式设为 `alter`（crate 名是 `alter-cli`）；`--json` 输出用 `serde_json::to_string`（单行）；R5.S2 / S3 退出码 1 用 `std::process::exit(1)` 而不是 clap 的错误码 2。
  - 性能：不要在 `main` 里初始化 tokio 运行时或 tracing；SC3 在验证时用 `cargo build --release` 产物测。

- [x] T7 [covers:R6.S1] [after:T3] Electron + electron-vite + SolidJS 空窗口，Playwright e2e 运行器 — seam: Playwright `_electron.launch` — verify: `bun run e2e --grep R6`
  - 文件：`.npmrc`（`electron_mirror=https://npmmirror.com/mirrors/electron/`）；根 `package.json` 加 `trustedDependencies: ["electron"]` 与脚本 `build: electron-vite build -c apps/browser/electron.vite.config.ts`、`e2e: bun run build && playwright test -c apps/browser/e2e/playwright.config.ts`；`apps/browser/package.json`（名字 `@alter/browser`，`version 0.1.0`，`main: out/main/index.js`）；`apps/browser/electron.vite.config.ts`（main / preload / renderer 三段，renderer 用 `vite-plugin-solid`）；`apps/browser/src/main/index.ts`（`BrowserWindow` 标题 `alter`，加载 renderer）；`apps/browser/src/preload/index.ts`（`contextBridge.exposeInMainWorld("alter", {...})`，T8 填内容）；`apps/browser/src/renderer/index.html`（`<title>alter</title>`）、`src/renderer/src/index.tsx`、`App.tsx`（渲染文本 `alter`）；`apps/browser/e2e/playwright.config.ts`（`testDir: "."`，`workers: 1`）、`apps/browser/e2e/window.spec.ts`（测试名 `R6.S1 窗口打开`：`_electron.launch({ args: ["apps/browser/out/main/index.js", ...extraArgs] })`，`firstWindow().title()` 为 `alter`，`getByText("alter")` 可见）。
  - 依赖：`electron`（稳定版）、`electron-vite`、`electron-builder`（本需求只装不配）、`solid-js`、`vite-plugin-solid`、`@playwright/test`。
  - 先验证：`bun install` 后 `ls node_modules/electron/dist/Electron.app` 存在；不存在先查 `.npmrc` 镜像与 `trustedDependencies`。
  - 坑：macOS 26 启动可能需要额外参数，launch 时追加 `process.env.ALTER_E2E_ARGS?.split(" ")`；e2e 与 CI 无关（宪章：涉及 Renderer 的需求本机跑 e2e）。
  - RED：先让 `App.tsx` 渲染 `placeholder`、`<title>` 为 `placeholder`，看断言失败。

- [x] T8 [covers:R7.S1,R7.S2,R7.S3] [after:T5,T7] Electron Main 托管 alterd：解析路径、spawn、轮询健康文件、退出 SIGTERM、缺失提示 — seam: Playwright `_electron.launch`（临时 `ALTER_HOME`、`ALTER_ALTERD_PATH`） — verify: `bun run e2e --grep R7`
  - 文件：`apps/browser/src/main/daemon.ts`（`resolveAlterdPath()`：`ALTER_ALTERD_PATH` → `join(process.resourcesPath, "bin/alterd")` → `join(repoRoot, "target/debug/alterd")`，不存在返回 `null`；`startDaemon(win)`：spawn `alterd run`（继承 `ALTER_HOME`），每 100 ms 读 `alterd.json` 最多 3 s，向 `win.webContents.send("daemon.status", {...})`；`stopDaemon()`：SIGTERM，2 s 后未退则 SIGKILL；`app.on("before-quit")` 调用它）；`src/preload/index.ts` 暴露 `onDaemonStatus(cb)`；`App.tsx` 按状态渲染 `alterd 启动中 / 运行中 / 未找到 / 已退出`；`apps/browser/e2e/daemon.spec.ts`：三个测试名以 `R7.S1` / `R7.S2` / `R7.S3` 开头；每个测试 `fs.mkdtemp` 作为 `ALTER_HOME` 传 `env`；R7.S2 用 `app.close()` 后轮询 `process.kill(pid, 0)` 抛 `ESRCH`。
  - 前置：`cargo build -p alterd` 产出 `target/debug/alterd`（e2e 脚本里先跑；R7.S3 用 `ALTER_ALTERD_PATH=/nonexistent/alterd` 覆盖）。
  - 坑：Electron Main 的 `process.env.ALTER_HOME` 必须透传给子进程；`before-quit` 是同步的，SIGKILL 兜底用 `setTimeout` 并 `event.preventDefault()` 等待后再 `app.exit()`；Renderer 文本断言用 `getByText("alterd 运行中")`。
  - 不在 Main 里读数据库。

- [x] T9 [after:T1,T3] [P] README、LICENSE（MIT）、仓库卫生文件、GitHub Actions CI、`ci:check` / `version:check` / `dev` 脚本 — verify: `bun run ci:check`
  - 文件：`README.md`（项目一句话、架构三进程、开发命令、目录）；`LICENSE`（MIT，版权人取 `git config user.name`）；`.gitignore`（`target/`、`node_modules/`、`out/`、`dist/`、`.DS_Store`、`*.log`）；`.editorconfig`；`.github/workflows/ci.yml`（`runs-on: macos-latest`，`dtolnay/rust-toolchain@stable` + `oven-sh/setup-bun@v2`，缓存 cargo，步骤 `bun install --frozen-lockfile`、`bun run ci:check`）；`scripts/version-check.sh`（比对 Cargo workspace version 与两个 package.json 的 version，不一致退出 1）；根 `package.json` 脚本 `version:check: bash scripts/version-check.sh`、`ci:check: cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && bun run lint && bun run typecheck && bun test && bun run version:check`、`dev: cargo build -p alterd && electron-vite dev -c apps/browser/electron.vite.config.ts`。
  - 并行所有权：只写上面列出的文件和根 `package.json` 的 `scripts` 三个键；不改 T7 / T8 拥有的 `apps/browser/**` 与 T4 的 `packages/**`。与 T7 共享根 `package.json`：T7 先合入后再加键，或由协调者合并。
  - 检查：`shellcheck scripts/version-check.sh`（本机未装则 `brew install shellcheck`，装不上则在 log.md 记明跳过）；`act` 不用，CI 工作流以首次 PR 运行为准。
  - VERIFY 证据：`bun run ci:check` 真实输出。

- [x] T10 [after:T6,T8,T9] 文档与代码对齐：AGENTS.md 命令表与目录树、components.md 的 TBD、`dflow validate` 全绿 — verify: `python3 -I /Users/bytedance/.claude/skills/dev-flow/scripts/dflow.py validate`
  - AGENTS.md：「命令」表六行全部去掉「待建立」并与根 `package.json` / Cargo 一致；「单个测试」示例改成真实路径（`cargo test -p alterd --test lifecycle`、`bun test packages/sdk/tests/smoke.test.ts`、`bun run e2e --grep R6`）；目录树核对实际目录；保持 300 行内。
  - `.dev-flow/infra/components.md`：把 alter-core / alterd / alter CLI / Electron Main / Renderer / packages 行里的 TBD 路径换成真实路径（只改路径与 seam，不改职责）。
  - `.dev-flow/infra/architecture.md` 横切关注点：补一句「健康文件 `alterd.json`」。
  - VERIFY 证据：`dflow validate` 输出 0 error、无 TPL001 / AGT001。

## 收敛任务

- [x] T11 [covers:R3.S3] [after:T10] alterd 用生命周期内一直持有的文件锁保证单实例，取代「读健康文件 + 检查 pid」 — seam: `alterd` 二进制（assert_cmd + 临时 `ALTER_HOME`） — verify: `cargo test -p alterd --test lifecycle`
  - 来源：评审 CR1。先写并发启动测试（同时 spawn 4 个 `alterd run`，断言恰好 1 个仍在运行、其余 2 秒内退出码 3 且 stderr 含 `already running`、健康文件 pid 是存活的那个），在现有代码上观察 RED。
  - 实现：`<ALTER_HOME>/alterd.lock` 用 `std::fs::File::try_lock()`（Rust 1.89 起稳定）在 `Store::open` 之前获取，持有到进程退出；拿不到锁即退出 3，stderr 仍为 `alterd: already running (pid <n>)`（pid 取自健康文件，读不到时写 `unknown`）。锁拿到后健康文件一律覆盖，R3.S4 自然成立；`health::is_alive` 仍留给 CLI 使用。
  - 回归：R3.S1–R3.S4 四个原有测试保持通过。
- [x] T12 [after:T10] 重新生成不绑定镜像地址的 bun.lock — verify: `! grep -q bnpm.byted.org bun.lock && bun install --frozen-lockfile && bun run ci:check`
  - 来源：评审 CR2。用公共源 `https://registry.npmjs.org/`（`BUN_CONFIG_REGISTRY`）重新解析锁文件，使条目不带镜像 URL，本机安装仍走用户级 `~/.npmrc` 的镜像。
  - 检查：依赖的主版本不变（typescript 仍为 ~6.0），`bun run ci:check` 与 `bun run e2e` 通过。
- [x] T13 [covers:R7.S1] [after:T10] 主进程就绪轮询不设截止时间，直到看到健康文件或子进程退出 — seam: Playwright `_electron.launch`（`ALTER_ALTERD_PATH` 指向测试用的慢启动脚本） — verify: `bun run e2e --grep R7`
  - 来源：评审 CR3。新增 e2e：假 alterd 脚本先睡 3.5 秒再写健康文件，断言界面最终显示 `alterd 运行中`；在现有代码上观察 RED。
  - 假脚本放 `apps/browser/e2e/fixtures/`，收到 SIGTERM 时删除健康文件并退出 0，测试结束兜底杀进程。
- [x] T14 [after:T10] alterd 错误退出只输出结构化日志并带退出码 — verify: `cargo build -q -p alterd && ALTER_HOME=/dev/null target/debug/alterd run`
  - 来源：评审 CR5。去掉 main.rs 错误分支的 `eprintln!`，tracing 错误日志加 `code = 1`；`already running` 的纯文本行是 design.md 的契约，保留。
  - 证据：stderr 每一行都能被 JSON 解析，退出码 1。
- [x] T15 [covers:R7.S2] [after:T10] R7.S2 的 2 秒从发起关闭时开始计时 — seam: Playwright `_electron.launch` — verify: `bun run e2e --grep R7.S2`
  - 来源：评审 CR6。在 `app.close()` 之前记 t0，pid 消失与健康文件删除都要在 t0 后 2 秒内完成。
  - 证据：先用变异（让 `stop()` 推迟 3 秒才发 SIGTERM）观察测试失败，恢复后通过。
- [x] T16 [after:T15] design.md 同步评审后的实现 — verify: `python3 -I /Users/bytedance/.claude/skills/dev-flow/scripts/dflow.py validate add-workspace-skeleton`
  - 来源：第 1 次验证的 contradicts 差距。改 design.md 的接口契约、组件、决策、风险与核心流程图：单实例改为 `alterd.lock` 排他锁；就绪轮询持续到看见健康文件或子进程退出；SDK 占位导出 `SDK_NAME`；`export-ts` 显式调用 `export_all`、类型不加 `#[ts(export)]`；Electron 44 按需下载，不配 `.npmrc` 与 `trustedDependencies`；alterd 的 stderr 经主进程管道转发；数据目录新增 `alterd.lock`。
  - 只改 design.md 与 seq 图；requirement.md 的场景与 revision 不变。
- [x] T17 [covers:R3.S3] [after:T15] alterd 拒绝第二实例时输出带退出码 3 的 JSON 日志 — seam: `alterd` 二进制（assert_cmd + 临时 `ALTER_HOME`） — verify: `cargo test -p alterd --test lifecycle`
  - 来源：第 1 次验证的 partial 差距（可观测性 NFR）。R3.S3 的测试追加断言：第二个实例 stderr 每一行都是 JSON，有一行 `fields.code` 为 3 且 message 含 `already running`；在现有代码上观察 RED。
  - 实现：去掉纯文本 `eprintln!`，改为 `tracing::warn!(code = 3, pid, "alterd: already running (pid <n>)")`，stderr 只有 JSON 行（与 T14 一致）。design.md 的契约由 T16 一并更新。
