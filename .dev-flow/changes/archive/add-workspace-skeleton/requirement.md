---
id: add-workspace-skeleton
title: 工程骨架：双工作区、核心存储与三进程启动
type: feature
status: archived
revision: 1
approved_revision: 1
repos: [alter]
created: 2026-10-20
skip_specs: false
---

# 工程骨架：双工作区、核心存储与三进程启动

<!-- 需求文档。正文用中文写；保留字保持英文："Spec Delta:"、"ADDED/MODIFIED/REMOVED/RENAMED Requirement:"、"Scenario:"、WHEN/THEN/AND、SHALL/MUST。语法见 references/formats.md -->

## 背景
仓库目前没有任何代码：`.dev-flow/infra/` 描述的是目标架构（Electron 壳 + Rust 守护进程 alterd + Rust CLI + Bun 运行的 SDK），`AGENTS.md` 里的命令全部标着「待建立」，`.dev-flow/config.json` 的 test / lint / typecheck / build 四个命令为空。后续每个功能需求（网关、Space 侧边栏、TaskSpace 协作、导入、安装脚本……）都要在同一套工作区、同一套门禁、同一个 SQLite 存储和同样的三个进程之上做测试先行的开发。没有这层地基，第一个业务需求就得顺手把工具链也搭了，范围会失控。

依据：`dflow status` 输出 `changes: []`、config.json 四个命令为空；AGENTS.md「命令」表每行标「待建立」；`.dev-flow/infra/architecture.md` 的架构决策表与 `data-model.md` 的 DDL 已在 init 阶段由用户确认。

## 变更内容
- 新增 Cargo workspace（`crates/alter-core`、`crates/alterd`、`crates/alter-cli`）与 Bun workspace（`apps/browser`、`packages/protocol`、`packages/sdk`），fmt / clippy / cargo test / eslint / tsc / bun test 门禁一条命令可跑。
- 新增 SQLite 存储层：首次打开按 `data-model.md` 的 DDL 建表（迁移 0001），WAL 与外键生效，重复打开幂等。
- 新增协议类型的 Rust 源头与 `export-ts` 导出：错误码枚举与健康文件结构生成到 `packages/protocol/src/generated/`。
- 新增 `alterd run`：建库、写健康文件 `alterd.json`、单实例、SIGTERM 清理退出。本需求不开网关、不连 Chromium。
- 新增 `alter` CLI 的 `--version` 与 `status [--json]` 两个入口。
- 新增 Electron + SolidJS 空窗口：启动时拉起 alterd、退出时关闭它、二进制缺失时在窗口里提示；Playwright 端到端运行器可用。
- 新增 README、MIT LICENSE、最小 GitHub Actions CI。

## 范围
- 范围内：上面七条；`config.json` 四个命令回填并刷新 AGENTS.md 的 dev-flow 块；性能预算中 CLI 冷启动一项的基准方法。
- 范围外：Gateway（WebSocket / JSON-RPC / token / `gateway.json`）、CdpMux 与任何 CDP 调用、Shell Control Channel、Space / 标签 / 命令栏等任何业务 UI、导入服务、SDK 的真实实现、DMG 打包与 `install.sh`、`alter nodejs` / `spaces` / `import` / `upgrade` 子命令、主页与 skill。

## 用户故事
1. 作为仓库开发者，我希望一条命令跑完全部门禁（Rust 与 TypeScript 两侧），以便每个需求的验证步骤有确定的通过标准。（P1）
2. 作为给 agent 写集成的开发者，我希望启动 app 后三个进程都起来，并能用 `alter status` 看到 alterd 的健康状态，以便后续网关需求有可运行的宿主。（P1）
3. 作为新贡献者，我希望 clone 之后 `bun install && bun run dev` 就能看到一个窗口，以便确认本机环境可用。（P2）

## Spec Delta: storage
SQLite 单文件存储，alterd 是唯一写者；本需求只建表与迁移，不含业务读写。

### ADDED Requirement: R1 数据库初始化与迁移
系统 SHALL 在打开数据目录时创建 SQLite 数据库文件并应用全部尚未应用的迁移，使表结构与 `data-model.md` 一致。

#### Scenario: R1.S1 空目录首次打开
- WHEN `ALTER_HOME` 指向一个空的临时目录，并打开存储
- THEN `<ALTER_HOME>/alter.db` 文件存在
- AND `sqlite_master` 中存在 `schema_version`、`spaces`、`tabs`、`bookmarks`、`history`、`task_spaces`、`task_pages`、`imports` 共 8 张表
- AND `SELECT MAX(version) FROM schema_version` 返回 1

#### Scenario: R1.S2 重复打开不重复迁移
- WHEN 同一个 `ALTER_HOME` 下的存储已经打开并迁移过一次，再次打开
- THEN 打开成功，没有错误
- AND `SELECT COUNT(*) FROM schema_version` 返回 1

#### Scenario: R1.S3 WAL 与外键生效
- WHEN 存储打开后执行 `PRAGMA journal_mode` 与 `PRAGMA foreign_keys`
- THEN 两者分别返回 `wal` 与 `1`
- AND 向 `tabs` 插入一行 `space_id` 为 `no-such-space` 的记录被拒绝，错误信息包含 `FOREIGN KEY constraint failed`

## Spec Delta: protocol
协议类型以 Rust 为源头，生成 TypeScript 声明供 SDK 与壳使用；禁止手改生成文件。

### ADDED Requirement: R2 协议类型导出
系统 SHALL 把 Rust 里标记为导出的协议类型生成为 TypeScript 声明文件写入指定目录。

#### Scenario: R2.S1 导出错误码与健康文件类型
- WHEN 执行 `cargo run -p alter-core --bin export-ts -- <空的临时目录>`
- THEN 退出码为 0
- AND 目录中存在 `ErrorCode.ts`，其类型是字符串字面量联合，恰好包含 `E_UNAUTHORIZED`、`E_PROTOCOL_VERSION`、`E_SPACE_NOT_FOUND`、`E_TASK_SPACE_NOT_FOUND`、`E_PAGE_NOT_FOUND`、`E_PAGE_BUDGET`、`E_USER_CONTROL`、`E_AMBIGUOUS_SELECTOR`、`E_NO_MATCH`、`E_TIMEOUT`、`E_DIALOG_OPEN`、`E_IMPORT_SOURCE_NOT_FOUND`、`E_KEYCHAIN_DENIED`、`E_INTERNAL` 这 14 个字面量
- AND 目录中存在 `HealthFile.ts`，其字段恰好为 `version`、`pid`、`startedAt`、`dbPath`

## Spec Delta: alterd-lifecycle
守护进程的启动、单实例与退出；健康文件是壳与 CLI 判断它是否在运行的唯一依据。

### ADDED Requirement: R3 守护进程启动与退出
系统 SHALL 在 `alterd run` 启动后把进程信息写入数据目录的健康文件 `alterd.json`，并在收到终止信号后删除它再退出。

#### Scenario: R3.S1 启动后写出健康文件
- WHEN `ALTER_HOME` 指向一个空的临时目录，启动 `alterd run`
- THEN 1 秒内出现 `<ALTER_HOME>/alterd.json`
- AND 文件是 JSON，`pid` 等于该进程的 pid，`version` 为 `0.1.0`，`dbPath` 为 `<ALTER_HOME>/alter.db`，`startedAt` 是毫秒级 Unix 时间戳整数
- AND `<ALTER_HOME>/alter.db` 存在，且 `SELECT MAX(version) FROM schema_version` 返回 1

#### Scenario: R3.S2 SIGTERM 后清理退出
- WHEN 一个已写出健康文件的 `alterd run` 收到 SIGTERM
- THEN 进程在 2 秒内退出，退出码为 0
- AND `<ALTER_HOME>/alterd.json` 已被删除

#### Scenario: R3.S3 拒绝第二个实例
- WHEN 同一个 `ALTER_HOME` 下已有一个 `alterd run` 在运行，再启动一个 `alterd run`
- THEN 第二个进程在 2 秒内退出，退出码为 3
- AND 第二个进程的 stderr 包含 `already running`
- AND 第一个进程仍在运行，`alterd.json` 里的 `pid` 仍是第一个进程的 pid

#### Scenario: R3.S4 接管残留的健康文件
- WHEN `<ALTER_HOME>/alterd.json` 已存在且其中 `pid` 为 999999（不存在的进程），启动 `alterd run`
- THEN 1 秒内 `alterd.json` 被改写，`pid` 等于新进程的 pid
- AND 启动后 1 秒时新进程仍在运行

## Spec Delta: cli
`alter` 命令行，agent 与用户共用；本需求只做版本与状态两个入口。

### ADDED Requirement: R4 版本输出
系统 SHALL 在 `alter --version` 时打印 CLI 的名字与版本号。

#### Scenario: R4.S1 打印版本
- WHEN 执行 `alter --version`
- THEN stdout 恰好为一行 `alter 0.1.0`
- AND 退出码为 0

### ADDED Requirement: R5 状态查询
系统 SHALL 在 `alter status` 时根据数据目录里的健康文件以及其中 pid 对应的进程是否存活来报告守护进程状态。

#### Scenario: R5.S1 守护进程运行中
- WHEN `<ALTER_HOME>/alterd.json` 存在，其 `pid` 对应一个存活的进程（例如正在运行的 `alterd run`），`version` 为 `0.1.0`，执行 `alter status`
- THEN stdout 包含 `running`、该 pid 和 `0.1.0`
- AND 退出码为 0
- AND 改为执行 `alter status --json` 时，stdout 为一行 JSON `{"running": true, "pid": <该 pid>, "version": "0.1.0", "dbPath": "<ALTER_HOME>/alter.db"}`

#### Scenario: R5.S2 没有健康文件
- WHEN `ALTER_HOME` 指向一个没有 `alterd.json` 的目录，执行 `alter status`
- THEN stdout 为一行 `not running`
- AND 退出码为 1
- AND 改为执行 `alter status --json` 时，stdout 为一行 JSON `{"running": false}`

#### Scenario: R5.S3 健康文件残留
- WHEN `<ALTER_HOME>/alterd.json` 存在且其中 `pid` 为 999999（不存在的进程），执行 `alter status`
- THEN stdout 为一行 `not running (stale)`
- AND 退出码为 1
- AND 改为执行 `alter status --json` 时，stdout 为一行 JSON `{"running": false, "stale": true}`

## Spec Delta: app-shell
Electron 壳：窗口、托管 alterd、给 Renderer 的 IPC；本需求只做空窗口与守护进程托管。

### ADDED Requirement: R6 主窗口启动
系统 SHALL 在 app 启动后打开一个标题为 `alter` 的主窗口。

#### Scenario: R6.S1 窗口打开
- WHEN 用 Playwright 的 `_electron.launch` 启动打包前的 app
- THEN 第一个窗口的标题为 `alter`
- AND 窗口页面的可见文本包含 `alter`

### ADDED Requirement: R7 守护进程托管
系统 SHALL 在 app 启动时拉起 alterd，并在 app 退出时关闭它。

#### Scenario: R7.S1 启动拉起 alterd
- WHEN `ALTER_HOME` 指向一个空的临时目录，启动 app
- THEN 3 秒内出现 `<ALTER_HOME>/alterd.json`
- AND 文件中的 `pid` 对应的进程存活
- AND 窗口页面的可见文本包含 `alterd 运行中`

#### Scenario: R7.S2 退出关闭 alterd
- WHEN app 已启动且 `<ALTER_HOME>/alterd.json` 已出现，关闭 app
- THEN 2 秒内该文件原先记录的 `pid` 不再存活
- AND `<ALTER_HOME>/alterd.json` 已被删除

#### Scenario: R7.S3 alterd 二进制缺失
- WHEN 环境变量 `ALTER_ALTERD_PATH` 为 `/nonexistent/alterd`，启动 app
- THEN 主窗口仍然打开
- AND 窗口页面的可见文本包含 `alterd 未找到`
- AND 启动后 5 秒时 app 进程仍在运行

## 非功能需求
- 质量门禁：宪章里的 Rust 与 TypeScript 门禁全部由 `bun run ci:check` 一条命令覆盖（检查：SC1）。
- 性能：`alter --version` 冷启动均值 < 20 ms（architecture.md 的性能预算；检查：SC3）。
- 安全：alterd 本需求不监听任何端口；`alterd.json` 不含密钥。
- 可观测性：alterd 用 `tracing` 把启动、迁移版本、收到的信号、退出码打到 stderr（JSON 行）；日志文件与滚动留给网关需求。
- 兼容性：macOS arm64，本机 macOS 26 验证；Intel 机器与更早版本不在本次验证范围。

## 成功标准
- SC1: 全部门禁一条命令通过 — measure: `bun run ci:check` 退出码为 0 — check: command
- SC2: 生成的协议类型与 Rust 源头一致 — measure: `bun run protocol:export` 后 `git diff --exit-code packages/protocol/src/generated` 退出码为 0 — check: command
- SC3: CLI 冷启动满足预算 — measure: `hyperfine --warmup 3 'target/release/alter --version'` 的均值 < 20 ms — check: command
- SC4: 每个场景有自动化测试证明 — measure: 验证报告的场景表 16 行全部 PASS — check: test

## 假设
- Rust edition 2024：本机 rustc 1.98 支持，新仓库没有理由用旧 edition。
- `ALTER_HOME` 默认 `~/Library/Application Support/alter`，环境变量覆盖：与 architecture.md 一致，测试靠它隔离。
- 健康文件叫 `alterd.json`，不是 `gateway.json`：后者在 api.md 里定义为含 port 与 token 的网关文件，本需求没有网关，混用会让 CLI 以后误判。
- alterd 二进制查找顺序 `ALTER_ALTERD_PATH` → `process.resourcesPath/bin/alterd` → `<repo>/target/debug/alterd`：开发态不需要打包，测试能指向不存在的路径。
- `export-ts` 生成的文件提交进仓库：SDK 与壳在没有 Rust 工具链的环境（CI 的 TS 任务、编辑器）也能做类型检查；SC2 守住一致性。
- Electron 端到端不进 CI：GitHub macOS runner 启 Electron 窗口不稳定，e2e 在本机作为验证门禁跑。
- 迁移 0001 直接采用 `data-model.md` 的全部 DDL：表结构已在 init 阶段确认，分批建表只会让后续需求各自改迁移。
- 三处版本号（Cargo workspace、根 package.json、`apps/browser/package.json`）同为 `0.1.0`，由 `bun run version:check` 守住一致：`alter --version` 与健康文件的 `version` 都来自它。
- SQLite 用 rusqlite 的 `bundled` 特性：不依赖系统 SQLite 版本，WAL 与外键行为确定。
- `alter status` 判断 pid 存活用 `kill(pid, 0)`：pid 复用导致的误判概率低，记为后续项，不在本需求处理。

## 待确认问题
- 无。

## 澄清记录
<!-- 每轮讨论追加一个 "### Session YYYY-MM-DD"，下面写 "- Q: ... → A: ..."。 -->
### Session 2026-10-20
- Q: 骨架的范围取「只搭工具链」、「工具链 + 核心存储」还是「工具链 + 核心存储 + 三个进程能起来」？ → A: 工具链 + 核心存储 + 三个进程能起来。
- Q: Electron 工具链用 electron-vite + electron-builder，还是 Electron Forge？ → A: electron-vite + electron-builder。
- Q: 本需求是否加 CI？ → A: 加，最小工作流（fmt、clippy、cargo test、lint、typecheck、bun test；e2e 不进 CI）。
- Q: 许可证？ → A: MIT，同时加 README。

## 变更记录
- 2026-10-20: 创建。
