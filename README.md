# alter

alter 是一个给人和 AI agent 共用的 macOS 浏览器：对人提供 Arc 风格的 Space 隔离与侧边栏，并能从 Chrome / Arc 导入 profile；对 agent 提供 `alter` 命令行与 `alter nodejs` 脚本运行时，agent 在用户已登录的 Space 里干活，用户随时可以接管再交还。

> 现状：工程骨架阶段。三个进程已经能启动并互相配合，浏览器功能按 `.dev-flow/changes/` 里的需求逐个落地。

## 架构

三个进程，共享一个数据目录（默认 `~/Library/Application Support/alter`，环境变量 `ALTER_HOME` 覆盖）：

| 进程 | 位置 | 职责 |
|---|---|---|
| alter.app | `apps/browser`（Electron + SolidJS） | 窗口与界面；启动时拉起 alterd，退出时关掉它 |
| alterd | `crates/alterd`（Rust） | 守护进程，唯一持有 SQLite 写连接；启动后写健康文件 `alterd.json` |
| alter | `crates/alter-cli`（Rust） | 命令行，例如 `alter status` 读健康文件报告 alterd 状态 |

协议类型以 Rust（`crates/alter-core`）为源头，生成 TypeScript 声明到 `packages/protocol`。目标架构与设计决策见 [.dev-flow/infra/](.dev-flow/infra/)。

## 开发

需要：macOS（arm64）、Rust stable、Bun（版本见 `.bun-version`）。

```sh
bun install && cargo fetch   # 安装依赖
bun run dev                  # 构建 alterd 并以开发模式启动 app
bun run ci:check             # 全部门禁：fmt、clippy、cargo test、eslint、tsc、bun test、版本号一致
bun run e2e                  # Electron 端到端（需要本机能开窗口，不在 CI 里跑）
bun run protocol:export      # 重新生成 packages/protocol/src/generated
```

命令行：

```sh
cargo run -q -p alter-cli -- --version          # alter 0.1.0
cargo run -q -p alterd -- run                   # 前台运行守护进程，Ctrl+C 退出
cargo run -q -p alter-cli -- status --json      # {"running":true,"pid":…,"version":"0.1.0","dbPath":"…"}
```

测试一律把 `ALTER_HOME` 指向临时目录，不会碰真实数据目录。

## 目录

```text
crates/alter-core   协议类型、SQLite 迁移与存储、健康文件
crates/alterd       守护进程
crates/alter-cli    alter 命令行
apps/browser        Electron 壳与端到端测试
packages/protocol   生成的 TypeScript 协议类型
packages/sdk        agent SDK（占位）
.dev-flow           需求文档、基础文档与宪章
```

## 许可证

[MIT](LICENSE)
