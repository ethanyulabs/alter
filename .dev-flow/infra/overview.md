# alter 概览

<!-- 基础文档：每个需求都以它为起点的系统级背景。每份文档保持简短、与现状一致。需求让它们过时时，由存档步骤更新。 -->

## 用途
alter 是一个 macOS 浏览器，给人和 AI agent 共用。对人，它提供 Arc 风格的交互：侧边栏标签、彼此隔离的 Space、固定标签、命令栏、分屏、自动归档和 Peek 预览，并能从 Chrome / Arc 导入书签、历史和 Cookie。对 agent，它提供 `alter` CLI 和 `alter nodejs` 脚本运行时（仿 ego-browser 的 TaskSpace / Page API），agent 在用户已登录的 Space 里完成任务，用户随时可以接管，再交还给 agent。热路径（agent 网关、CDP 多路复用、存储、导入）由 Rust 守护进程 `alterd` 承担，Electron 只负责窗口与 UI，脚本运行时用 Bun，目标是低延迟。

现状：工程骨架、浏览器基础 UI，以及 add-core-features 交付的核心功能已实现——多个隔离的 Space、固定标签与拖拽、快捷键；Gateway、CDP 多路复用与转发、`alter nodejs` 与 SDK 的 TaskSpace / Page API；agent 叠层与接管交还；命令栏、浏览历史、分屏、Peek、自动归档；从 Chrome / Arc 导入书签、历史与 Cookie（向导与 `alter import`；Cookie 经 Keychain 授权解密）。add-release-and-cookies 补齐了发布：Apple Silicon 的 DMG、推 tag 自动发布、`install.sh`、`alter upgrade`、新版本提示，以及主页（`site/`，GitHub Pages）与 agent skill（`skills/alter-browser`，`alter skill install` 安装）。

## 仓库
| 仓库 | 路径 | 角色 | 技术栈 | 测试命令 |
|---|---|---|---|---|
| alter | . | 单仓多包：Rust 核心（alterd、CLI、core）、Electron 壳、SDK、协议类型、agent skill、主页、安装脚本 | Rust（tokio 单线程运行时、tokio-tungstenite、自写 CDP 客户端、rusqlite）、TypeScript（Bun、SolidJS、Vite、Electron）、SQLite | `cargo test && bun test`（全部门禁 `bun run ci:check`） |

## 术语表
| 术语 | 含义 |
|---|---|
| Space | 用户侧的工作区：一组标签、书签、历史，以及独立的 Chromium partition（Cookie / storage 互相隔离）。对应 Arc 的 Space。 |
| Partition | Chromium 的会话分区，命名 `persist:space-<spaceId>`，一个 Space 一个，由 Electron 创建 WebContentsView 时指定；数据目录为 `ALTER_HOME/chromium/Partitions/space-<id 小写>`。 |
| Tab | Space 内的一个页面，侧边栏里一行；对应一个 WebContentsView（首次激活时才创建；agent 托管的标签总有离屏视图）和 Chromium target；关闭即归档（`tabs.archived_at`），固定标签关闭只卸载页面。 |
| Pinned Tab | 固定标签：常驻侧边栏顶部，不参与自动归档。 |
| Split View | 分屏：同一 `split_group` 的标签在窗口内并排显示。 |
| Peek | Shift+点击页面里的链接时打开的浮层预览（内容区的 80%），Esc 或点浮层外关闭，可「在新标签打开」；不切换当前标签，不存库。 |
| 自动归档 | 非固定标签超过不活跃阈值后移出侧边栏（`tabs.archived_at`），可在归档列表恢复。 |
| alterd | Rust 守护进程，随 alter.app 启动，唯一写 SQLite、唯一发 CDP 的进程。 |
| ALTER_HOME | 数据目录，默认 `~/Library/Application Support/alter`，环境变量覆盖；测试一律指向临时目录。 |
| 健康文件 | `ALTER_HOME/alterd.json`：alterd 运行时写入的 `{version, pid, startedAt, dbPath}`，CLI 与 app 据此判断 alterd 是否在运行。 |
| Gateway | alterd 对外的本机 WebSocket JSON-RPC 接口，token 鉴权；SDK 和 CLI 都走它。 |
| Shell Control Channel | alterd 与 Electron Main 之间的本地 Unix socket（`ALTER_HOME/shell.sock`，0600），JSON 行协议：alterd 下发全量状态快照与少量指令，Electron 上报用户事件。 |
| 全量快照 | `shell.stateChanged`：全部未归档的 Space 与标签及各 Space 的激活标签。连接建立时发一份，之后每次状态变化再发一份；Electron 据此对齐侧边栏与页面视图。 |
| CDP | Chrome DevTools Protocol。Chromium 开 remote-debugging 端口，alterd 的 CdpMux 直连，Playwright 也可经 alterd 转发的端点直连。 |
| TaskSpace | agent 的任务空间：挂在某个 Space 下，共享其登录态，拥有若干带永久标签的 Page；一个用户目标只用一个 TaskSpace。 |
| Page 标签 | TaskSpace 内页面的永久名字 `p1`、`p2`…，跨脚本调用稳定；用户页面为 `userPage`。 |
| Ownership | TaskSpace 当前由谁控制：`agent` 或 `user`。`user` 期间 agent 命令被拒（`E_USER_CONTROL`）。 |
| HandOff / TakeOver | agent 把控制权交给用户（handOff）；用户做完后 agent 再接回（takeOverTaskSpace）。用户也可主动接管。 |
| Receipt | 每个 agent 动作的回执：成功与否、耗时、立即观察到的 popup / dialog。不代表页面最终状态。 |
| Snapshot | 页面的无障碍树文本，带 `@ref` 引用，agent 据此选目标元素。 |
| Import Source | 导入来源：`chrome` 或 `arc`（Arc 也是 Chromium 数据格式）。 |
| Page Budget | 一个 TaskSpace 允许同时打开的 Page 数上限。 |
| Safe Storage 口令 | Chrome / Arc 存在 Keychain 里的口令（「Chrome Safe Storage」「Arc Safe Storage」），由它派生解密 Cookie 的密钥；只在导入任务的内存里存在。 |
| 部分完成（partial） | 导入的一种结果：书签与历史照常导入，Cookie 没有导入，`warning` 说明原因（`keychain_denied` 用户拒绝授权，`shell_unavailable` 没有持有 Chromium 的 app 连接或它没有及时回报）。 |
| 更新提示 | alterd 发现 GitHub 上有更新的版本时，经 Gateway 通知 `app.updateAvailable` 与 Shell 快照的 `updateAvailable` 告诉侧边栏、SDK；只提示，升级由用户运行 `alter upgrade`。 |
| alter-browser skill | 给 agent 的使用说明（`SKILL.md` 与 `references/`），随 app 包发布，`alter skill install` 复制到 `~/.claude/skills`、`~/.codex/skills` 或 `--target` 目录，目录里的 `.alter-skill` 标记记录版本。 |

## 基础文档
- [架构](architecture.md) · [核心流程](core-flows.md) · [组件](components.md) · [API](api.md) · [数据模型](data-model.md)
