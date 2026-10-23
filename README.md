# alter

alter 是一个给人和 AI agent 共用的 macOS 浏览器：对人提供 Arc 风格的 Space 隔离与侧边栏，并能从 Chrome / Arc 导入书签、历史与 Cookie；对 agent 提供 `alter` 命令行与 `alter nodejs` 脚本运行时，agent 在用户已登录的 Space 里干活，用户随时可以接管再交还。

主页：<https://ethanyulabs.github.io/alter/>

## 安装

在终端运行（只支持 Apple Silicon、macOS 13 及以上）：

```sh
curl -fsSL https://github.com/ethanyulabs/alter/releases/latest/download/install.sh | bash
```

它会下载最新版本并校验 SHA256，把 `alter.app` 装进 `/Applications`（不可写时装进 `~/Applications`），把命令行链接到 `~/.local/bin/alter`。之后：

```sh
alter upgrade --check        # 有没有新版本
alter upgrade                # 升级
alter skill install          # 把 agent skill 装进 ~/.claude/skills、~/.codex/skills（已存在的那些）
alter skill install --target <dir>   # 装进其他 agent 的 skills 目录
```

也可以从 [Releases](https://github.com/ethanyulabs/alter/releases/latest) 下载 DMG。app 没有经过公证，在浏览器里下载的 DMG 第一次打开时会被拦下，到「系统设置 → 隐私与安全性 → 仍要打开」放行即可；用上面的命令安装不会遇到这个问题。

卸载：先运行 `alter skill uninstall`（装过 skill 的话），再删除 `/Applications`（或 `~/Applications`）里的 `alter.app` 和 `~/.local/bin/alter`。数据在 `~/Library/Application Support/alter`。

## 现状

已有的功能：
- 多个隔离的 Space，固定标签与拖拽，快捷键。
- Cmd+T 命令栏、浏览历史、分屏、Peek、自动归档。
- 从 Chrome / Arc 导入书签、历史与 Cookie，有向导，也有 `alter import`。
- agent 网关、`alter nodejs` 与 SDK 的 TaskSpace / Page API，agent 叠层与接管交还，Playwright `connectOverCDP`。
- 打包与发布：DMG、推 tag 自动发布、install.sh、`alter upgrade`、新版本提示。
- agent skill（`skills/alter-browser`），主页在 `site/`。

## 架构

三个进程，共享一个数据目录（默认 `~/Library/Application Support/alter`，环境变量 `ALTER_HOME` 覆盖）：

| 进程 | 位置 | 职责 |
|---|---|---|
| alter.app | `apps/browser`（Electron + SolidJS） | 窗口与界面；启动时拉起 alterd，退出时关掉它 |
| alterd | `crates/alterd`（Rust） | 守护进程：Space / 标签状态、agent 网关、CDP、导入，唯一持有 SQLite 写连接 |
| alter | `crates/alter-cli`（Rust） | 命令行：`status`、`nodejs`、`import`、`spaces`、`upgrade`、`skill` |

协议类型以 Rust（`crates/alter-core`）为源头，生成 TypeScript 声明到 `packages/protocol`。目标架构与设计决策见 [.dev-flow/infra/](.dev-flow/infra/)。

## 开发

需要：macOS（arm64）、Rust stable、Bun（版本见 `.bun-version`）、shellcheck。

```sh
bun install && cargo fetch   # 安装依赖
bun run dev                  # 构建 alterd 并以开发模式启动 app
bun run ci:check             # 全部门禁：fmt、clippy、cargo test、eslint、tsc、bun test、版本号一致、shellcheck
bun run e2e                  # Electron 端到端（需要本机能开窗口，不在 CI 里跑）
bun run dist                 # 打包 dist/alter-<版本>-arm64.dmg（要联网取 Bun）；dist:dir 只出 .app
bun run protocol:export      # 重新生成 packages/protocol/src/generated
```

命令行：

```sh
cargo run -q -p alter-cli -- --version          # alter 0.1.0
cargo run -q -p alterd -- run                   # 前台运行守护进程，Ctrl+C 退出
cargo run -q -p alter-cli -- status --json      # {"running":true,"pid":…,"version":"0.1.0","dbPath":"…"}
```

测试一律把 `ALTER_HOME` 指向临时目录，不会碰真实数据目录。

发布：把三处版本号（`Cargo.toml`、`package.json`、`apps/browser/package.json`）改成新版本，推送 `v<版本>` tag，GitHub Actions 会打包并创建 Release。

## 目录

```text
crates/alter-core   协议类型、SQLite 迁移与存储、健康文件
crates/alterd       守护进程
crates/alter-cli    alter 命令行
apps/browser        Electron 壳与端到端测试
packages/protocol   生成的 TypeScript 协议类型
packages/sdk        agent SDK（TaskSpace / Page API）
skills/alter-browser 给 agent 的 skill
site                主页（GitHub Pages）
scripts             打包、安装与发布脚本
.dev-flow           需求文档、基础文档与宪章
```

## 许可证

[MIT](LICENSE)
