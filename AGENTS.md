# alter

<!-- 给编码 agent 和贡献者的指南。事实以代码为准；基础文档在 .dev-flow/infra/，这里链接过去，不要照抄。整个文件控制在 300 行以内。 -->

## 项目简介
alter 是一个 macOS 浏览器，给人和 AI agent 共用：对人提供 Arc 风格的 Space 隔离与侧边栏交互，并可从 Chrome / Arc 导入 profile；对 agent 提供 `alter` CLI 与 `alter nodejs` 脚本运行时（仿 ego-browser 的 TaskSpace / Page API），agent 在用户已登录的 Space 里干活，用户可随时接管再交还。本仓库是单仓多包：Rust 守护进程 alterd（热路径）、Electron + SolidJS 壳、Bun 运行的 SDK、CLI、agent skill、主页与安装脚本。

**现状：核心功能已可用（add-core-features）。** 多个隔离的 Space、固定标签与拖拽、快捷键；Cmd+T 命令栏、浏览历史、分屏、Peek、自动归档；agent 网关（Gateway、CDP 多路复用与带 token 的转发端点）、`alter nodejs` 与 SDK 的 TaskSpace / Page API、agent 叠层与接管交还；从 Chrome / Arc 导入书签与历史（向导与 `alter import`）。还没有：Cookie 导入（Keychain 解密），以及 agent skill、主页、DMG 打包、安装脚本与 `alter upgrade`（发布）——由后续 change 落地，下文标「待建立」的还没有代码。`.dev-flow/infra/` 描述现状与目标架构，二者有别处已注明。

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

单个测试：Rust `cargo test -p alterd --test lifecycle`、`cargo test -p alterd --test shell n4_`、`cargo test -p alterd --test gateway n3_`、`cargo test -p alterd --test import`、`cargo test -p alterd --bin alterd cdp::`（单元测试）、`cargo test -p alter-cli --test import`（CLI 测试只在 alterd 二进制不存在时构建它，先 `cargo build -p alterd`）；基准（`#[ignore]`）：Shell 通道 `cargo test -p alterd --release --test shell_perf -- --ignored --nocapture`、Gateway `cargo test -p alterd --release --test gateway_perf -- --ignored --nocapture`、CLI 冷启动 `cargo test -p alter-cli --release --test perf -- --ignored --nocapture`；TS `bun test packages/sdk/tests/smoke.test.ts`；Electron 端到端 `bun run e2e --grep R7`（按场景 ID 过滤，会先构建 alterd 与 app）。端到端需要本机能启动 Electron 窗口，每次启动自动用临时 `ALTER_HOME`，`ALTER_E2E_ARGS` 可追加 Electron 参数；CLI 与 alterd 测试同样用临时 `ALTER_HOME` 隔离状态。无需外部服务。

命令行入口（调试时可直接跑）：`cargo run -q -p alter-cli -- --version`、`ALTER_HOME=<临时目录> cargo run -q -p alter-cli -- status [--json]`（退出码 0 运行中、1 未运行或残留）、`cargo run -q -p alter-cli -- spaces list` / `import --from chrome --profile <dir> --space <id>`（需要 alterd 在运行）、`ALTER_HOME=<临时目录> cargo run -q -p alterd -- run`（Ctrl+C 退出；已有实例时退出码 3；app 托管时带 `--exit-on-stdin-close`，stdin 关闭即清理退出）、`bun run protocol:export`（重新生成 TS 协议类型）。

## 技术栈
- Rust（stable，edition 2024）：tokio（alterd 用单线程运行时，接管的保证依赖它）、tokio-tungstenite / futures-util（Gateway 与自写的最小 CDP 客户端，不用 chromiumoxide）、tungstenite（CLI 的同步 WS）、rusqlite（bundled SQLite，WAL，`backup` / `functions` 特性）、toml、serde、ts-rs（导出 TS 类型）、clap、assert_cmd；security-framework（Keychain）随 Cookie 导入的后续 change 引入。
- TypeScript（Bun 作为运行时与包管理器，版本锁在 `.bun-version`）：Electron 44、electron-vite 6、SolidJS、Vite、eslint + typescript-eslint、tsc 6.0（typescript-eslint 尚不支持 TS 7，勿升级）、Playwright（Electron 端到端）。仓库根的 `.npmrc` 固定公共 registry（个人 `~/.npmrc` 的镜像地址写进 `bun.lock` 会让 CI 下载失败），bun 用隔离链接，apps/browser 的命令行工具只在它自己的 `node_modules/.bin`，根脚本经 `bun run --cwd apps/browser` 转发。
- 存储：SQLite 单文件 `~/Library/Application Support/alter/alter.db`；Cookie / storage 由 Chromium partition 管理。
- 打包与分发（待建立）：electron-builder 出 DMG（内含 alterd、alter、Bun），GitHub Releases + GitHub Pages。

## 架构
详见 [.dev-flow/infra/architecture.md](.dev-flow/infra/architecture.md)、[core-flows.md](.dev-flow/infra/core-flows.md)、[components.md](.dev-flow/infra/components.md)、[api.md](.dev-flow/infra/api.md)、[data-model.md](.dev-flow/infra/data-model.md)。

- 三个进程：alter.app（Electron Main + SolidJS Renderer + Chromium）、alterd（Rust 守护进程，随 app 启动）、alter CLI（Rust；`alter nodejs` 启动 Bun 加载 SDK）。
- 一次 agent 动作：脚本 → SDK → Gateway（loopback WS JSON-RPC，token）→ 该 TaskSpace 的串行队列，TabService 校验 ownership → CdpMux 直接向 Chromium 发 CDP → receipt 回传；agent 光标与动作文字的叠层也由 alterd 经 CDP 注入页面的隔离 world。Electron Main 不在这条路径上。
- 标签生命周期：alterd 是 Space / Tab / TaskSpace 状态的唯一来源，经 Shell Control Channel（Unix socket）的全量快照让 Electron 创建 / 销毁 WebContentsView 并指定 partition；Electron 把用户事件回传，并上报每个视图的 CDP targetId。
- 接管：TabService 把 ownership 写成 user 的同一步递增接管信号（`takeover.rs`），正在执行、排队与入队未写出的 agent 命令都以 `E_USER_CONTROL` 结束，CDP 转发端点拒绝 / 断开。
- 只有 alterd 写 SQLite、只有 alterd 发 CDP；导入服务在 alterd 内读 Chrome / Arc 数据，经 TabService 写库（Cookie 的 Keychain 解密待后续 change）。
- 外部依赖：Chromium（经 Electron）、macOS Keychain、GitHub Releases / Pages。

## 目录结构
```text
alter/
├── crates/
│   ├── alter-core/      # 协议类型、错误码、SQLite 迁移与仓储；ts-rs 导出 TS 类型
│   ├── alterd/          # 守护进程：lifecycle（run）、shell/（shell.sock 服务端）、tabs/（TabService：Space / 标签 / TaskSpace 状态唯一来源、ownership、搜索）、gateway/（Gateway、page.*、CDP 转发）、cdp/（CdpMux、叠层）、import/、archive.rs、takeover.rs
│   └── alter-cli/       # `alter` 命令：--version / status / nodejs / spaces list / import；upgrade 待建立
├── apps/
│   └── browser/         # Electron 壳：src/main/（窗口、托管 alterd、shell-link.ts、tab-views.ts、peek.ts、menu.ts、stdio.ts）、src/preload/、src/renderer/（SolidJS：侧边栏、SpaceBar、命令栏、Peek、接管横幅、导入向导）、src/shared/、e2e/
├── packages/
│   ├── protocol/        # 由 alter-core 生成的 TS 类型，不手写
│   └── sdk/             # alter-sdk：taskSpace() / Page API，Bun 内运行（`alter nodejs` 预加载 runtime.ts）
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
- 规格先行：`.dev-flow/changes/` 里没有已批准的需求不改行为；测试先行，证据写进 log.md（见下方「需求开发」与 `.dev-flow/constitution.md`）。
- 只有 alterd 写 SQLite、只有 alterd 发 CDP；Electron Main 不碰数据库与 CDP，Renderer 只经 IPC 与 Main 通信。
- `packages/sdk` 零 electron 依赖；`crates/alter-cli` 除 `nodejs` 子命令外不启动 Bun。
- 协议类型的源头在 `crates/alter-core`，`packages/protocol` 由 `cargo run -p alter-core --bin export-ts` 生成；禁止手改生成文件。
- `.dev-flow/specs/protocol/spec.md` 写死了 `ErrorCode` 的字面量集合与导出类型：需求要新增错误码或导出类型时，写需求文档时就加 protocol 能力的 MODIFIED，不要等到实现时再走 update。
- 所有网络接口只监听 127.0.0.1；`gateway.json` 与 `shell.sock` 权限 0600；Keychain 密钥只在内存，不写日志、不落盘。
- 性能预算是门禁：CLI 冷启动 < 20 ms，网关单动作内部开销 < 5 ms（基准数值在建网关需求时校准，见 architecture.md）。基准回归即失败。
- `E_USER_CONTROL` 语义不可绕过：ownership=user 时 agent 命令必须被拒绝，SDK 不得自动重试。
- 新依赖、新 crate / 包、新抽象要在该需求的 design.md 里说明为什么更简单的方案不行。
- 修复评审发现时改动了接口契约（退出码、文件格式、IPC、命令输出、单实例机制等），在同一个任务里同步改 design.md 的「接口契约」与相关图，不留到验证阶段。
- 实现偏离 design.md 时（模块落点、新依赖、时序、IPC / 协议字段、已定的决策与风险结论），在同一个任务里改 design.md 与 `assets/*.mmd`：add-core-features 的整体验证为此反复了 5 轮。
- 只在 dev-flow 的 git 步骤或用户明确要求时 commit / push；提交说明用中文，不加任何 AI 署名行。
- 代码注释与文档用中文；标识符、命令、路径不翻译。

## 验证与测试
- Rust 单测与集成测试在各 crate 的 `src/` 与 `tests/`；网关协议与 Shell Control 协议各有契约测试（假客户端 / 假 Electron 端）。
- TS 单测在 `packages/*/tests/`（`bun test`，由 bunfig.toml 限定范围），SDK 对假 Gateway 断言请求体与错误映射。
- Shell Control 的契约测试在 `crates/alterd/tests/shell.rs`（共用辅助在 `tests/common/mod.rs`），基准在 `tests/shell_perf.rs`（`#[ignore]`）。
- Electron 端到端在 `apps/browser/e2e/`（Playwright `_electron.launch`），用例标题以场景 ID 开头；覆盖主窗口、alterd 托管、侧边栏与标签。页面用 `e2e/fixtures/server.ts` 的本地 HTTP 服务器，不访问外网；内容区 URL 在主进程里读（`webContents.getAllWebContents()`、窗口挂载的 `WebContentsView`），不依赖 `app.windows()`；用例失败时 `dumpLogsOnFailure` 打印 app 主进程与 alterd 的输出。已存档需求的用例保留各自的场景 ID，`--grep` 可能同时选中新旧同名 ID。
- 评审与验证交给 codex 时，它的沙箱启动不了 Electron（SIGABRT），也不允许监听 Unix socket（alterd 的 lifecycle / shell 测试会以 `Operation not permitted` 失败）：`bun run e2e` 与 `cargo test` 都由主会话在本机跑，把结果贴进验证报告并注明运行环境。
- 写完 e2e 或契约测试后，至少想一个能让它误判为通过的错误实现，并用变异确认测试会失败；计时类成功标准从被测系统自己留下的时间点（如 `alterd.json` 的 mtime）起算，只允许高估。
- 带时限的 e2e 断言（「N 秒内」）用 `apps/browser/e2e/ui.ts` 的 `expectAllWithin(t0, ms, …)`：`t0 = now()` 在触发动作之前取；要同时成立的几个条件放在一次 `win.evaluate` 里同步读取（分几次异步读可能读到切换前后的两个状态）；可见性按计算样式与尺寸判断。
- 主进程可能被未捕获异常的原生弹窗卡住的 e2e（stdio、异常路径）：每次 `app.evaluate` 都限时，`finally` 里 `app.close()` 限时、关不掉就 SIGKILL，不留孤儿 app；用 `stream.destroy()` 关闭 app 的管道读端后要等 `close` 事件再写；直接写 `process.stdout` 与经 `console.*` 写要分开测（Node 的 Console 会给没有监听器的流补空 `error` 监听，顺带兜住同一时刻其他写入的错误）。
- CLI 用 `assert_cmd` 跑真实二进制，`ALTER_HOME` 指向临时目录。
- alterd 退出路径（信号、stdin 关闭）的测试要覆盖宿主已死、stdin / stderr 管道都已断开的情况：此时写日志会失败，清理不能因此中断。只靠 e2e 才测得出的问题，codex 的沙箱跑不到。
- `scripts/install.sh` 过 `shellcheck`，并用 `--dry-run` 断言步骤。
- 一次变更完成的标准：宪章质量门禁全部通过（fmt、clippy、cargo test、lint、typecheck、bun test，涉及 UI / 网关时加 e2e，改动的行覆盖率不低于 80%），`dflow.py validate` 通过，log.md 有 RED → GREEN 证据。

## 需求开发

本仓库的需求文档放在 `.dev-flow/`，用 dev-flow 技能做需求开发：任何 chore、fix、feature 或 refactor 都走 `/dev-flow new` → `/dev-flow run` → `/dev-flow archive`（Codex：`$dev-flow …`）。
- `.dev-flow/changes/` 下没有已批准的需求时，不改动行为。测试先行（red → green → refactor），证据写进该需求的 `log.md`。
- 本仓库的测试命令：`cargo test && bun test`
