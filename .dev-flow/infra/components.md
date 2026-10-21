# 组件

| 组件 | 仓库 / 路径 | 职责 | 依赖 | 公开接口 |
|---|---|---|---|---|
| alter-core | alter:crates/alter-core/ | 协议类型（Gateway 方法 / 事件 / 错误码、Shell Control 消息）、SQLite 迁移与仓储层、配置文件解析；`ts-rs` 导出 TS 类型 | rusqlite、serde | Rust API（`Store`、`protocol::*`）；`migrations/NNNN_*.sql` |
| Gateway | alter:crates/alterd/src/gateway/ | 监听 loopback WebSocket，token 鉴权，JSON-RPC 分发到 TaskSpaceManager / ImportService，推送事件 | alter-core、tokio-tungstenite | api.md 的 Gateway 协议（测试用 WS 客户端直接驱动） |
| TaskSpaceManager | alter:crates/alterd/src/task_space/ | TaskSpace 与 Page 的生命周期、ownership 状态机、page budget、每个 TaskSpace 的串行命令队列 | Store、CdpMux、ShellLink | Rust API；经 Gateway 暴露为 `taskSpace.*` / `page.*` |
| CdpMux | alter:crates/alterd/src/cdp/ | 连接 Chromium remote-debugging 端口，按 target 多路复用 CDP 会话，实现 goto / snapshot / click / fill 等原语与等待；对外转发 CDP 端点 | chromiumoxide | Rust API；`http://127.0.0.1:<cdpPort>/json/version` 转发端点 |
| ShellServer | alter:crates/alterd/src/shell/ | Shell Control Channel 的 alterd 端：监听 `shell.sock`（0600），连接即发全量快照，之后逐份广播；按行解析 Electron 事件交给 TabService，错误与 `navigate` 只回发送方 | alter-core、tokio | api.md 的 Shell Control 协议（`crates/alterd/tests/shell.rs` 用假 Electron 端驱动） |
| TabService | alter:crates/alterd/src/tabs/ | Space / 标签状态的唯一来源：默认 Space、地址规范化、新建 / 导航 / 激活 / 关闭 / 回写标签，生成快照；独占 `Store` | Store | Rust API；经 Shell Control 事件驱动 |
| ImportService | alter:crates/alterd/src/import/ | 拷贝并解析 Chrome / Arc 的 Bookmarks / History / Cookies，Keychain 取密钥解密，经 CDP 写 Cookie | Store、CdpMux、security-framework | Gateway `import.run`；fixture 目录驱动的 Rust 单测 |
| Store | alter:crates/alter-core/src/store/ | SQLite 读写（单写连接 + WAL），表见 data-model.md | rusqlite | Rust API；表结构本身是契约 |
| alter CLI | alter:crates/alter-cli/ | 子命令 `nodejs` / `spaces` / `import` / `status` / `upgrade`；读 gateway.json（`status` 目前读健康文件 `alterd.json`）；`nodejs` 启动打包的 Bun 并注入环境变量 | alter-core | 命令行（测试用 `assert_cmd` 驱动） |
| alter-sdk | alter:packages/sdk/ | `taskSpace()` / `takeOverTaskSpace()` / `listTaskSpaces()` 与 Page API 的 TS 实现，WS 客户端，错误映射 | @alter/protocol | TS 公开 API（`bun test` 对假 Gateway） |
| @alter/protocol | alter:packages/protocol/ | 由 alter-core 生成的 TS 类型，不手写 | 无 | 类型包 |
| Electron Main | alter:apps/browser/src/main/ | 窗口、托管 alterd（`daemon.ts`）、Shell Control 客户端（`shell-link.ts`）、按快照管理 WebContentsView 与布局、网页弹窗转新标签（`tab-views.ts`）、partition、IPC 给 Renderer | electron | IPC 通道（Playwright Electron e2e 驱动） |
| Renderer | alter:apps/browser/src/renderer/ | SolidJS UI：Sidebar（Space / 标签 / 固定 / 归档）、CommandBar、SplitView 布局、AgentOverlay（agent 光标与动作文字）、导入向导、Peek | solid-js | DOM（Playwright e2e 驱动） |
| alter-browser skill | alter:skills/alter-browser/ | 教 agent 用 CLI 与 SDK 的 SKILL.md 与 references（API、安装、清理状态） | 无 | 文档；`bun run skills:check` 校验示例可跑 |
| site | alter:site/ | 主页静态站（功能介绍、安装命令、skill 入口），发 GitHub Pages | 无 | 静态文件（构建产物可访问性检查） |
| install.sh | alter:scripts/install.sh | 下载 DMG、装到 /Applications、去 quarantine、链接 `alter` 到 `~/.local/bin`，`--dry-run` 只打印 | curl、hdiutil | shell（`shellcheck` + `--dry-run` 测试） |

## 说明
归属与分层规则（谁可以依赖谁）：
- 只有 alterd 写 SQLite，只有 alterd 发 CDP。Electron Main 不碰数据库、不碰 CDP；它按 alterd 的快照与指令创建、挂载或销毁 WebContentsView。
- Renderer 只经 Electron IPC 与 Main 通信，不直接连 Gateway，也不 require electron 主进程模块。
- alter-sdk 零 electron 依赖，只依赖 @alter/protocol，可在任何 Bun / Node 环境运行。
- alter CLI 只依赖 alter-core 与 Gateway 协议；`alter nodejs` 之外的子命令不启动 Bun。
- 协议类型的源头在 alter-core（Rust），@alter/protocol 由 `cargo run -p alter-core --bin export-ts` 生成；手改生成文件算违规。
- 已有代码的组件（add-workspace-skeleton、add-spaces-sidebar 建立）：alter-core（`src/store/` 含 `spaces.rs`、`tabs.rs`，`src/protocol/` 含 `shell.rs`，`src/health.rs`、`src/paths.rs`、`src/bin/export_ts.rs`）、alterd（`src/lifecycle.rs`、`src/shell/`、`src/tabs/`）、alter CLI（`src/status.rs`，仅 `--version` / `status`）、@alter/protocol（`src/generated/`）、alter-sdk（占位）、Electron Main（`src/main/index.ts`、`daemon.ts`、`shell-link.ts`、`tab-views.ts`）、Renderer（`src/renderer/src/` 的 `App`、`Sidebar`、`AddressBar`、`NavButtons`）。其余行的路径仍是目标路径，由对应需求落地。

测试接入的 seam（需求的 tasks.md 从这些入口驱动测试）：
1. Gateway 协议：WS 客户端对 alterd 发 JSON-RPC，断言响应与事件（契约测试，宪章第四条）。
2. Shell Control Channel 协议：假 Electron 端连 socket，断言 alterd 下发的快照、错误与指令（`crates/alterd/tests/shell.rs`）。
3. SDK 公开 API：`bun test` 对假 Gateway，断言请求体与错误映射。
4. CLI 子命令：`assert_cmd` 跑真实二进制，用临时 `ALTER_HOME` 隔离状态。
5. SQLite 表：迁移后的 schema 与仓储层查询（Rust 单测，内存库）。
6. Electron IPC 与 DOM：Playwright 的 `_electron.launch` 跑真实壳，断言侧边栏 / 命令栏 / 叠层。
7. `install.sh --dry-run`：打印将执行的步骤，bats 或 shell 断言。
