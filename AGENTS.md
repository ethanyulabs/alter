# alter

<!-- 给编码 agent 和贡献者的指南。事实以代码为准；基础文档在 .dev-flow/infra/，这里链接过去，不要照抄。整个文件控制在 300 行以内。 -->

## 项目简介
alter 是一个 macOS 浏览器，给人和 AI agent 共用：对人提供 Arc 风格的 Space 隔离与侧边栏交互，并可从 Chrome / Arc 导入 profile；对 agent 提供 `alter` CLI 与 `alter nodejs` 脚本运行时（仿 ego-browser 的 TaskSpace / Page API），agent 在用户已登录的 Space 里干活，用户可随时接管再交还。本仓库是单仓多包：Rust 守护进程 alterd（热路径）、Electron + SolidJS 壳、Bun 运行的 SDK、CLI、agent skill、主页与安装脚本。

**现状：工程骨架已就绪。** 三个进程能启动并互相配合：app 拉起 alterd，alterd 建库并写健康文件 `alterd.json`，`alter status` 读它报告状态；还没有网关、CDP 与任何浏览器 UI。`.dev-flow/infra/` 描述的是目标架构，功能按 `.dev-flow/changes/` 里的需求逐个落地；下文标「待建立」的还没有代码。

## 功能
- Space：每个 Space 一套独立的 Cookie / storage（Chromium partition），侧边栏底部切换。
- 侧边栏标签：垂直标签列表、固定标签、拖拽排序、不活跃标签自动归档并可恢复。
- 命令栏（Cmd+T）：一个输入框完成打开 URL、搜索、切换标签 / Space。
- 分屏（Split View）与 Peek 悬停预览。
- 从 Chrome / Arc 导入书签、历史、Cookie（Cookie 经 Keychain 授权解密）到指定 Space。
- `alter nodejs` 脚本运行时：agent 用 TaskSpace / Page API 打开页面、快照、点击、填表、截图、下载。
- 人机协作：agent 光标与动作文字叠层；用户随时接管，agent `handOff()` 等用户做完再 `takeOverTaskSpace()` 接回。
- Playwright 可经 alterd 转发的 CDP 端点 `connectOverCDP` 直连。
- `scripts/install.sh` 一键安装 app 与 CLI；`alter upgrade` 自更新；主页与 agent skill 随仓库发布。

## 命令
| 任务 | 命令 |
|---|---|
| 安装 | `bun install && cargo fetch` |
| 运行 / 开发 | `bun run dev`（先构建 alterd，再以开发模式启动 Electron，由 app 拉起 alterd） |
| 测试 | `cargo test && bun test` |
| Lint | `cargo fmt --check && cargo clippy --all-targets -- -D warnings && bun run lint` |
| 类型检查 | `bun run typecheck` |
| 构建 | `cargo build && bun run build`（release 构建与 DMG 打包待建立） |
| 全部门禁 | `bun run ci:check`（CI 跑的就是它） |

单个测试：Rust `cargo test -p alterd --test lifecycle`（或 `cargo test -p alter-core --test store r1_s1`）；TS `bun test packages/sdk/tests/smoke.test.ts`；Electron 端到端 `bun run e2e --grep R7`（按场景 ID 过滤，会先构建 alterd 与 app）。端到端需要本机能启动 Electron 窗口，每次启动自动用临时 `ALTER_HOME`，`ALTER_E2E_ARGS` 可追加 Electron 参数；CLI 与 alterd 测试同样用临时 `ALTER_HOME` 隔离状态。无需外部服务。

命令行入口（调试时可直接跑）：`cargo run -q -p alter-cli -- --version`、`ALTER_HOME=<临时目录> cargo run -q -p alter-cli -- status [--json]`（退出码 0 运行中、1 未运行或残留）、`ALTER_HOME=<临时目录> cargo run -q -p alterd -- run`（Ctrl+C 退出；已有实例时退出码 3）、`bun run protocol:export`（重新生成 TS 协议类型）。

## 技术栈
- Rust（stable，edition 2024）：tokio、chromiumoxide（CDP）、rusqlite（bundled SQLite，WAL）、security-framework（Keychain）、serde、ts-rs（导出 TS 类型）、clap、assert_cmd。
- TypeScript（Bun 作为运行时与包管理器，版本锁在 `.bun-version`）：Electron 44、electron-vite 6、SolidJS、Vite、eslint + typescript-eslint、tsc 6.0（typescript-eslint 尚不支持 TS 7，勿升级）、Playwright（Electron 端到端）。bun 用隔离链接，apps/browser 的命令行工具只在它自己的 `node_modules/.bin`，根脚本经 `bun run --cwd apps/browser` 转发。
- 存储：SQLite 单文件 `~/Library/Application Support/alter/alter.db`；Cookie / storage 由 Chromium partition 管理。
- 打包与分发：electron-builder 出 DMG（内含 alterd、alter、Bun），GitHub Releases + GitHub Pages。

## 架构
详见 [.dev-flow/infra/architecture.md](.dev-flow/infra/architecture.md)、[core-flows.md](.dev-flow/infra/core-flows.md)、[components.md](.dev-flow/infra/components.md)、[api.md](.dev-flow/infra/api.md)、[data-model.md](.dev-flow/infra/data-model.md)。

- 三个进程：alter.app（Electron Main + SolidJS Renderer + Chromium）、alterd（Rust 守护进程，随 app 启动）、alter CLI（Rust；`alter nodejs` 启动 Bun 加载 SDK）。
- 一次 agent 动作：脚本 → SDK → Gateway（loopback WS JSON-RPC，token）→ TaskSpaceManager 校验 ownership → CdpMux 直接向 Chromium 发 CDP → receipt 回传，同时向 Renderer 的 AgentOverlay 推送光标与动作文字。Electron Main 不在这条路径上。
- 标签生命周期：alterd 是 Space / Tab / TaskSpace 状态的唯一来源，经 Shell Control Channel（Unix socket）让 Electron 创建 / 销毁 BrowserView 并指定 partition；Electron 把用户事件回传。
- 只有 alterd 写 SQLite、只有 alterd 发 CDP；导入服务在 alterd 内读 Chrome / Arc 数据并经 Keychain 解密 Cookie。
- 外部依赖：Chromium（经 Electron）、macOS Keychain、GitHub Releases / Pages。

## 目录结构
```text
alter/
├── crates/
│   ├── alter-core/      # 协议类型、错误码、SQLite 迁移与仓储；ts-rs 导出 TS 类型
│   ├── alterd/          # 守护进程：目前只有 lifecycle（run）；gateway/ task_space/ cdp/ shell/ import/ 待建立
│   └── alter-cli/       # `alter` 命令：目前 --version / status；nodejs / spaces / import / upgrade 待建立
├── apps/
│   └── browser/         # Electron 壳：src/main/（窗口、托管 alterd）、src/preload/、src/renderer/（SolidJS）、src/shared/、e2e/
├── packages/
│   ├── protocol/        # 由 alter-core 生成的 TS 类型，不手写
│   └── sdk/             # alter-sdk：taskSpace() / Page API，Bun 内运行（目前是占位）
├── skills/
│   └── alter-browser/   # 给 agent 的 SKILL.md 与 references/（待建立）
├── site/                # 主页静态站，发 GitHub Pages（待建立）
├── scripts/
│   ├── version-check.sh # 三处版本号一致性检查（bun run version:check）
│   └── install.sh       # 下载 DMG、安装 app、链接 CLI 到 ~/.local/bin（待建立）
├── .github/workflows/   # CI：macOS 上跑 bun run ci:check
├── .dev-flow/           # 需求文档、基础文档、宪章（dev-flow 工作区）
├── Cargo.toml           # Cargo workspace（版本号与依赖版本集中在这里）
├── package.json         # Bun workspace 与全部脚本
└── bunfig.toml          # bun test 只扫 packages/
```

## 规则与约束
- 规格先行：`.dev-flow/changes/` 里没有已批准的需求不改行为；测试先行，证据写进 log.md（见下方 dev-flow 块与 `.dev-flow/constitution.md`）。
- 只有 alterd 写 SQLite、只有 alterd 发 CDP；Electron Main 不碰数据库与 CDP，Renderer 只经 IPC 与 Main 通信。
- `packages/sdk` 零 electron 依赖；`crates/alter-cli` 除 `nodejs` 子命令外不启动 Bun。
- 协议类型的源头在 `crates/alter-core`，`packages/protocol` 由 `cargo run -p alter-core --bin export-ts` 生成；禁止手改生成文件。
- 所有网络接口只监听 127.0.0.1；`gateway.json` 与 `shell.sock` 权限 0600；Keychain 密钥只在内存，不写日志、不落盘。
- 性能预算是门禁：CLI 冷启动 < 20 ms，网关单动作内部开销 < 5 ms（基准数值在建网关需求时校准，见 architecture.md）。基准回归即失败。
- `E_USER_CONTROL` 语义不可绕过：ownership=user 时 agent 命令必须被拒绝，SDK 不得自动重试。
- 新依赖、新 crate / 包、新抽象要在该需求的 design.md 里说明为什么更简单的方案不行。
- 修复评审发现时改动了接口契约（退出码、文件格式、IPC、命令输出、单实例机制等），在同一个任务里同步改 design.md 的「接口契约」与相关图，不留到验证阶段。
- 只在 dev-flow 的 git 步骤或用户明确要求时 commit / push；提交说明用中文，不加任何 AI 署名行。
- 代码注释与文档用中文；标识符、命令、路径不翻译。

## 验证与测试
- Rust 单测与集成测试在各 crate 的 `src/` 与 `tests/`；网关协议与 Shell Control 协议各有契约测试（假客户端 / 假 Electron 端）。
- TS 单测在 `packages/*/tests/`（`bun test`，由 bunfig.toml 限定范围），SDK 对假 Gateway 断言请求体与错误映射。
- Electron 端到端在 `apps/browser/e2e/`（Playwright `_electron.launch`），用例标题以场景 ID 开头；目前覆盖主窗口与 alterd 托管，后续加侧边栏、命令栏、叠层与接管。
- 评审与验证交给 codex 时，它的沙箱启动不了 Electron（SIGABRT），e2e 由主会话在本机跑 `bun run e2e`，把结果贴进验证报告并注明运行环境。
- CLI 用 `assert_cmd` 跑真实二进制，`ALTER_HOME` 指向临时目录。
- `scripts/install.sh` 过 `shellcheck`，并用 `--dry-run` 断言步骤。
- 一次变更完成的标准：宪章质量门禁全部通过（fmt、clippy、cargo test、lint、typecheck、bun test，涉及 UI / 网关时加 e2e），`dflow.py validate` 通过，log.md 有 RED → GREEN 证据。

<!-- dev-flow:start -->
## 需求开发（dev-flow）

本仓库的需求文档放在 `.dev-flow/`。这个块由 `dflow.py init` 生成，请在块外编辑。

- 任何 chore、fix、feature（新模块或新服务）或 refactor 都用 dev-flow 技能：`/dev-flow new`、`/dev-flow update`（修改已批准的规格）、`/dev-flow run`（也负责评审、验证和进度）、`/dev-flow archive`；让 `/dev-flow new` 更新 AGENTS.md 会刷新本文件，`/dev-flow link <path>` / `/dev-flow unlink <path>` 管理多仓库项目里的其他仓库（Codex：`$dev-flow new ...`）。
- `.dev-flow/changes/` 下没有已批准的需求时，不要改动行为。测试先行（red → green → refactor），证据写进该需求的 `log.md`。
- 本仓库的测试命令：`cargo test && bun test`
<!-- dev-flow:end -->
