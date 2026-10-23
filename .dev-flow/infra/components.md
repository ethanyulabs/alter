# 组件

| 组件 | 仓库 / 路径 | 职责 | 依赖 | 公开接口 |
|---|---|---|---|---|
| alter-core | alter:crates/alter-core/ | 协议类型（Gateway 方法 / 事件 / 错误码、Shell Control 消息）、SQLite 迁移与仓储层、配置文件解析；`ts-rs` 导出 TS 类型 | rusqlite、serde | Rust API（`Store`、`protocol::*`）；`migrations/NNNN_*.sql` |
| Gateway | alter:crates/alterd/src/gateway/ | 监听 loopback WebSocket（`mod.rs`），token 鉴权，JSON-RPC 分发（`rpc.rs`）到 TabService / ImportService，`page.*` 原语（`page.rs`、`actions.rs`），按 TaskSpace 推送事件（`events.rs`）；每个 TaskSpace 一把 `tokio::sync::Mutex` 的串行队列；CDP 转发端点（`forward.rs`，TCP 透明代理） | alter-core、tokio-tungstenite、futures-util | api.md 的 Gateway 协议（`crates/alterd/tests/gateway.rs` 用 WS 客户端驱动） |
| TaskSpace（逻辑组件） | alter:crates/alterd/src/tabs/task_space.rs、tabs/ownership.rs、takeover.rs | TaskSpace 与 Page 的生命周期、page budget、ownership 状态机、接管信号（每个 TaskSpace 一个、全局一个 `watch`，与写 ownership 同一步递增）、`inactive` 计时；状态在 TabService 里，串行队列在 Gateway | Store、CdpMux | 经 Gateway 暴露为 `taskSpace.*` / `page.*` |
| CdpMux | alter:crates/alterd/src/cdp/ | 自写的最小 CDP 客户端：一个 browser 级 WebSocket（flatten session），按 targetId（Main 经 `getOrCreateDevToolsTargetId` 上报）按需 attach，分发响应与事件；写出任务在写出前检查接管信号；agent 叠层（`overlay.rs` / `overlay.js`） | tokio-tungstenite | Rust API（单元测试用假 CDP 端） |
| ShellServer | alter:crates/alterd/src/shell/ | Shell Control Channel 的 alterd 端：监听 `shell.sock`（0600），连接即发全量快照，之后逐份广播；按行解析 Electron 事件交给 TabService，错误与 `navigate` 只回发送方 | alter-core、tokio | api.md 的 Shell Control 协议（`crates/alterd/tests/shell.rs` 用假 Electron 端驱动） |
| TabService | alter:crates/alterd/src/tabs/ | Space / 标签 / TaskSpace 状态的唯一来源，独占 `Store`（数据库唯一写者）：Space 管理、固定 / 移动 / 分屏 / 归档 / 恢复、浏览历史、命令栏搜索（`search.rs`）、生成快照；Gateway 与导入经 `Request::Gateway` 把操作交给它的循环执行 | Store | Rust API；经 Shell Control 事件与 Gateway 驱动 |
| ImportService | alter:crates/alterd/src/import/ | 读取 Chrome / Arc 的 Bookmarks（拷贝）与 History（SQLite backup API 取一致快照，被锁时退回拷贝），书签树与最近 90 天历史经 TabService 批量写库；进度推给发起方；启动时收尾中断的导入。Cookie（`cookies.rs`）：Cookies 库经 backup 读进内存，`keychain.rs` 取 Safe Storage 口令（debug 构建可用 `ALTER_KEYCHAIN_FAKE` 替代），PBKDF2 + AES-128-CBC 解密，经 TabService 的 `send_cookies` 分批交给持有 Chromium 的 Shell 连接，等 `cookiesImported` 回报；拒绝授权或没有连接时 `partial` | TabService、rusqlite、security-framework、RustCrypto | Gateway `import.run` / `import.status`，Shell `importRequested`（`crates/alterd/tests/import.rs`） |
| Store | alter:crates/alter-core/src/store/ | SQLite 读写（单写连接 + WAL，注册 `alter_fold` 做 Unicode 小写折叠搜索），表见 data-model.md | rusqlite | Rust API；表结构本身是契约 |
| 更新检查 | alter:crates/alterd/src/update.rs | 启动后（`ALTER_UPDATE_CHECK_DELAY_MS`，默认 5 秒）与之后每 24 小时用 `curl -q` 取 Release API 的 latest；比自身新时经 Gateway 推 `app.updateAvailable`（当时的连接与之后每个新连接各一次）并写进 Shell 快照；`update_check = false` 或 debug 构建没设 `ALTER_RELEASE_API` 时不检查 | 系统 curl、Gateway、TabService | Gateway 通知与 `ShellState.updateAvailable`（`crates/alterd/tests/gateway.rs` 的 `rc_n2_*` 用本地假 Release 服务器） |
| Archiver | alter:crates/alterd/src/archive.rs | 定时（阈值 / 3，限制在 10 ms 到 60 s）请 TabService 归档超过阈值的非固定标签 | TabService | 经 Shell 快照的 `archived` 体现 |
| alter CLI | alter:crates/alter-cli/ | 子命令 `status` / `nodejs` / `spaces list` / `import` / `upgrade` / `skill install` / `skill uninstall`；读 gateway.json 经同步 WS（tungstenite）连 Gateway；`nodejs` 启动 Bun（包内的 `bin/bun` 与 `sdk/runtime.js`，开发时 `ALTER_BUN_PATH` 或 PATH 里的 bun）并注入环境变量；`exe.rs` 的 `real_exe()` 解开符号链接找同包的资源；`upgrade.rs` 用系统 curl 取 latest 后运行该版本的 install.sh；`skill.rs` 经暂存目录原子替换 | alter-core、tungstenite、系统 curl | 命令行（测试用 `assert_cmd` 驱动） |
| alter-sdk | alter:packages/sdk/ | `taskSpace()` / `takeOverTaskSpace()` / `listTaskSpaces()` 与 Page API 的 TS 实现，WS 客户端，错误映射；`runtime.ts` 收到 `app.updateAvailable` 时在 stderr 打一次 `[alter:notice]`；打包时 `bun build` 成 `sdk/runtime.js` | @alter/protocol | TS 公开 API（`bun test` 对假 Gateway） |
| @alter/protocol | alter:packages/protocol/ | 由 alter-core 生成的 TS 类型，不手写 | 无 | 类型包 |
| Electron Main | alter:apps/browser/src/main/ | 窗口、托管 alterd（`daemon.ts`）、Shell Control 客户端（`shell-link.ts`）、按快照管理 WebContentsView、分屏布局与上报 targetId（`tab-views.ts`）、把导入的 Cookie 写进 partition 并回报（`cookie-import.ts`）、Peek（`peek.ts`）、应用菜单与快捷键（`menu.ts`、`actions.ts`）、stdout / stderr 断开防护（`stdio.ts`）、IPC 给 Renderer | electron | IPC 通道（Playwright Electron e2e 驱动） |
| Renderer | alter:apps/browser/src/renderer/ | SolidJS UI：Sidebar（标签 / 固定 / 拖拽 / 右键分屏 / 底部菜单）、SpaceBar 与 SpacePanel、ArchivedList、CommandBar、PeekChrome、HandoffBanner、ImportWizard（含 Cookie 复选框与部分完成的原因）、UpdateBanner（新版本提示条，点开 Release 页）（agent 光标与动作文字的叠层由 alterd 经 CDP 注入页面，不在 Renderer） | solid-js | DOM（Playwright e2e 驱动） |
| alter-browser skill | alter:skills/alter-browser/ | 教 agent 用 CLI 与 SDK 的 SKILL.md 与 references（`api.md`、`handoff.md`、`playwright.md`、`install.md`）；随 app 包发布，`alter skill install` 安装 | 无 | 文档；`packages/sdk/tests/distribution.test.ts` 校验 front matter、API 与 SDK 双向一致，`apps/browser/e2e/skill.spec.ts` 真实跑示例 |
| site | alter:site/ | 主页静态站（英文 `index.html`、中文 `zh/index.html`，功能介绍、安装命令、skill 入口），`.github/workflows/pages.yml` 发 GitHub Pages | 无 | 静态文件；`distribution.test.ts` 校验引用都在 `site/` 内、不加载外部资源，安装命令与 README、install.sh 逐字一致 |
| install.sh | alter:scripts/install.sh | 平台检查，取 latest（或 `--version`），下载 DMG 与 `.sha256` 并校验，挂载后 ditto 到 `<prefix>/alter.app.new` 再原子替换（失败时 trap 恢复旧包），正在运行的同一个包先 `kill -TERM` 装完再 `open`，链接 `alter` 到 `--bin-dir`（默认 `~/.local/bin`），`--with-skill` 调 `alter skill install`，`--dry-run` 只打印 | curl、shasum、hdiutil、ditto、lsappinfo | shell（`shellcheck -S style` + `crates/alter-cli/tests/install_script.rs`） |
| 打包与发布脚本 | alter:scripts/{dist.sh,fetch-bun.sh,check-bundle.sh,version-check.sh}、apps/browser/electron-builder.yml、.github/workflows/release.yml | `dist.sh` 构建 release 二进制、按 `.bun-version` 取 Bun 并校验、`bun build` SDK 运行时、electron-builder 出 DMG 与 `.sha256`；`check-bundle.sh` 断言包内布局、版本、签名与 DMG 校验和；`version-check.sh --tag` 让三处版本号与 tag 一致才发布 | electron-builder、curl | `crates/alter-cli/tests/release_scripts.rs`、`apps/browser/e2e/packaged.spec.ts`（打包后的 app） |

## 说明
归属与分层规则（谁可以依赖谁）：
- 只有 alterd 写 SQLite，只有 alterd 发 CDP。Electron Main 不碰数据库、不碰 CDP；它按 alterd 的快照与指令创建、挂载或销毁 WebContentsView。
- Renderer 只经 Electron IPC 与 Main 通信，不直接连 Gateway，也不 require electron 主进程模块。
- alter-sdk 零 electron 依赖，只依赖 @alter/protocol，可在任何 Bun / Node 环境运行。
- alter CLI 只依赖 alter-core 与 Gateway 协议；`alter nodejs` 之外的子命令不启动 Bun。
- 协议类型的源头在 alter-core（Rust），@alter/protocol 由 `cargo run -p alter-core --bin export-ts` 生成；手改生成文件算违规。
- 上表的组件都已落地（add-workspace-skeleton、add-spaces-sidebar、fix-orphan-alterd、main-stdio-epipe、add-core-features、add-release-and-cookies 建立）。
- 只有 alterd 的更新检查、`alter upgrade` 与 install.sh 访问网络，都经系统 curl；测试一律指向本地假 Release 服务器（`ALTER_RELEASE_API` / `ALTER_RELEASE_BASE`）。

测试接入的 seam（需求的 tasks.md 从这些入口驱动测试）：
1. Gateway 协议：WS 客户端对 alterd 发 JSON-RPC，断言响应与事件（契约测试，宪章第四条）。
2. Shell Control Channel 协议：假 Electron 端连 socket，断言 alterd 下发的快照、错误与指令（`crates/alterd/tests/shell.rs`）。
3. SDK 公开 API：`bun test` 对假 Gateway，断言请求体与错误映射。
4. CLI 子命令：`assert_cmd` 跑真实二进制，用临时 `ALTER_HOME` 隔离状态。
5. SQLite 表：迁移后的 schema 与仓储层查询（Rust 单测，内存库）。
6. Electron IPC 与 DOM：Playwright 的 `_electron.launch` 跑真实壳，断言侧边栏 / 命令栏 / 叠层。
7. 发布脚本：`assert_cmd` 跑 `install.sh`（`--dry-run` 断言步骤；PATH 里放假的 `lsappinfo` / `open`）、`dist.sh` / `check-bundle.sh`，配 `crates/alter-cli/tests/common/release_server.rs` 的假 Release 服务器（`crates/alter-cli/tests/{install_script,release_scripts,upgrade,skill}.rs`）。
8. 文档一致性：`packages/sdk/tests/distribution.test.ts` 读 `site/`、`skills/`、README 与 SDK 源码，断言安装命令、API 列表、提示文案一致。
