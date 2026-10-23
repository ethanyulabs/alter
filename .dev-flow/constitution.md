# alter 宪章

**版本**：1.1.0 | **批准日期**：2026-10-20 | **最近修订**：2026-10-22

这些原则约束每一次变更：需求讨论阶段对照它们检查设计，验证阶段也一样。新增原则时升 MINOR 版本，删除或重新定义原则时升 MAJOR 版本，措辞修正升 PATCH 版本。

## 核心原则

### 一、规格先行
行为变更从 `.dev-flow/changes/` 里一份已批准的需求开始。代码与规格不一致，说明两者之一有缺陷：先改规格，再改代码。

### 二、测试先行（不可协商）
每个场景都由一个测试证明，而且在实现存在之前，亲眼看到过这个测试因预期的原因失败。测试从约定的 seam 驱动公开接口。mock 只用在系统边界上。

### 三、简单优先
只构建满足场景所需的最小实现。新增依赖、模块或抽象，必须由设计说明更简单的方案为什么不行。

### 四、边界契约
跨模块或跨仓库共用的接口、事件和表结构要写下来（design.md 或 infra/api.md），并在使用方依赖它们之前由契约测试覆盖。

### 五、证据优先
「完成」指 log.md 里有证据：实际运行过的命令和它们的真实输出。没有观察到的结果不要记录。

## 质量门禁
- 批准之前和存档之前，`dflow.py validate` 通过。
- 进入 `verified` 之前，本仓库的全部测试通过（多仓库需求的每个仓库各自验证自己的分片）。
- Rust：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` 全绿。
- TypeScript：`bun run lint`（eslint）、`bun run typecheck`（tsc --noEmit）、`bun test` 全绿。
- 涉及 Renderer、Electron Main 或网关协议的需求，必须跑过 Electron 端到端（Playwright，`bun run e2e`）。
- `scripts/install.sh` 改动必须过 `shellcheck`，并用 `--dry-run` 验证。
- 性能预算（见 infra/architecture.md 横切关注点）回归即算失败：CLI 启动、单次 page 动作的网关开销。
- 覆盖率：本次需求改动的行覆盖率不低于 80%（`config.json` 的 `coverage_threshold`）；Rust 用 `cargo llvm-cov`，TypeScript 用 `bun test --coverage`。
- 以上命令以 `.dev-flow/config.json` 填入的为准，都是当下真实可跑的。

## 执行策略
构建一个需求时可以自行决定的事，这样执行从不需要中途停下来问；每个需求的任务备注在这些默认值上细化。
- 外部服务、凭证或 fixture：项目不依赖外部服务。页面用 `e2e/fixtures/server.ts` 的本地 HTTP 服务器，不访问外网；Chrome / Arc 导入用测试里构造的 profile 数据，不读本机真实 profile；macOS Keychain 在系统边界 mock，测试不触发授权弹窗。状态一律放临时 `ALTER_HOME`。本机缺工具（如 `cargo llvm-cov`、`shellcheck`）时把对应任务标为 blocked，其余继续。
- 依赖界限：dev 依赖可以直接加，钉精确版本并同步更新锁文件（`bun.lock` 走仓库 `.npmrc` 的公共 registry）；运行时依赖（Cargo `[dependencies]`、包的 `dependencies`）要在 design.md 里说明为什么更简单的方案不行。`packages/sdk` 不得引入 electron 依赖；tsc 不升到 7。
- 构建时不得单独跨过的单向门：SQLite 表结构与迁移；`crates/alter-core` 的协议类型与 `ErrorCode` 集合；Gateway、Shell Control、CDP 转发端点的协议字段；CLI 的退出码与命令输出格式；`gateway.json`、`alterd.json` 等落盘文件的格式与权限；单实例机制；删除或改写用户数据；新增运行时依赖；`E_USER_CONTROL` 语义；增删改场景。
- 重试上限：同一问题修三次失败后，把任务标为 blocked，其余继续。

## 治理
本宪章优先于与之冲突的习惯。修订它要走一次正常的需求（类型为 `chore` 或 `refactor`），并更新版本号和最近修订日期。
