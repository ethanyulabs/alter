# alter 宪章

**版本**：1.0.0 | **批准日期**：2026-10-20 | **最近修订**：2026-10-20

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
- 以上命令在 `.dev-flow/config.json` 填入的是当下真实可跑的；仓库尚无代码时为空，由第一个需求的 T1 建立并回填。

## 治理
本宪章优先于与之冲突的习惯。修订它要走一次正常的需求（类型为 `chore` 或 `refactor`），并更新版本号和最近修订日期。
