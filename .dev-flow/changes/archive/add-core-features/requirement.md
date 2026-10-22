---
id: add-core-features
title: alter 核心功能：Space、agent、浏览增强与导入
type: feature
status: archived
revision: 2
approved_revision: 2
repos: [alter]
created: 2026-10-21
skip_specs: false
---

# alter 核心功能：Space、agent、浏览增强与导入

<!-- 流程路线型需求的父级需求文档。Spec Delta 写在每个节点自己的 requirement.md 里（nodes/<node-slug>/）；这份文件写整体：背景、变更内容、非功能需求、成功标准。正文用中文写。语法见 references/formats.md §14 -->

## 背景
alter 的目标是一个人和 AI agent 共用的 macOS 浏览器（`.dev-flow/infra/overview.md`）。目前合并到 `main` 的只有三件：工程骨架（PR #1）、alterd 残留修复（PR #2）、浏览器基础 UI（PR #3）。也就是说，现在只能在唯一的默认 Space `Personal` 里开标签、浏览、后退前进、重启恢复。产品承诺的核心能力一项都还没有：多个隔离的 Space、agent 用的 `alter` CLI / `alter nodejs` 脚本运行时、人机接管、命令栏 / 分屏 / Peek / 自动归档、从 Chrome / Arc 导入，以及别人能装上用的安装包、主页和 agent skill（AGENTS.md「现状」一节）。

用户的要求（2026-10-21）是「app + 主页 + skills」一次写成完整需求，而不是逐个开小需求。目标架构、Gateway 方法表、接管与导入的时序图都已经写在 `.dev-flow/infra/`（architecture.md、api.md、core-flows.md、components.md、data-model.md）。这份需求以它们为准，按依赖顺序拆成 6 个节点，每个节点补上可测的场景。2026-10-21 收窄（revision 2）：发布节点 N6 与 N5 的 Cookie 导入移到后续 change，本需求交付 N1–N5。

## 变更内容
- 侧边栏底部一排 Space 圆点：可以新建、改名改色、切换、删除 Space；删除时清掉它的 Cookie 和存储；重启后回到上次所在的 Space；不同 Space 之间 Cookie 互相隔离。
- 固定标签（Arc 式：关闭只卸载页面）、拖拽排序，以及一组常用快捷键。
- agent 网关：`alter nodejs` 运行脚本，用 TaskSpace / Page API（仿 ego-browser）在用户的 Space 里开页面、快照、点击、填表、截图、下载；也可以经 alterd 转发的 CDP 端点让 Playwright `connectOverCDP` 直连。
- 人机协作：页面上显示 agent 光标和动作文字；agent 可以 `handOff()` 把控制权交给用户，用户做完点「交还给 Agent」；用户也可以随时「停止 Agent」接管，之后 agent 的命令一律被拒（`E_USER_CONTROL`）。
- Cmd+T 命令栏（搜标签、Space、历史、书签，或打开网址）、两个标签分屏、Shift+点击链接的 Peek 预览、不活跃标签自动归档和「已归档」列表。
- 从 Chrome / Arc 导入书签和最近 90 天历史到指定 Space，app 内有导入向导，也有 `alter import` 命令。

## 范围
- 范围内：上面列出的全部，按 `flow.md` 拆成 5 个节点，每个节点的 Spec Delta、设计和任务写在 `nodes/<node-slug>/`；对应的 Rust 契约测试、SDK 单测、CLI 测试和 Electron 端到端测试；基础文档在存档时更新。
- 范围外：
  - 多窗口、标签在 Space 之间移动（data-model.md 规定 `tabs.space_id` 不可变）、Space 拖拽排序。
  - 书签管理界面（书签只在命令栏里搜索和打开）、下载管理界面（下载只经 agent API 与系统默认目录）。
  - MCP server 适配、云同步、多用户。
  - Developer ID 签名与公证（install.sh 安装的路径不需要，见澄清记录）；macOS 以外的平台（Windows / Linux）。
  - Arc 侧边栏专有数据（`StorableSidebar.json` 里的 Space、固定项）的导入：只导入 Arc profile 里 Chromium 格式的书签与历史。
  - 移到后续 change（revision 2）：
    - 发布：`skills/alter-browser` agent skill、`site/` 主页、DMG 安装包、安装脚本、推 tag 自动发布、`alter upgrade`（原节点 N6 release-site）。
    - 导入 Cookie：Keychain 取 Safe Storage 口令、解密、写进目标 Space 的 partition，拒绝授权时部分完成，损坏 Cookie 的跳过（原 N5 的 R3、R4、R7、R9.S3）。
  - app 内的更新提示（`app.updateAvailable` 事件）。

## 用户故事
1. 作为用户，我希望把工作和个人浏览分成几个 Space，彼此的登录状态互不影响，并能随时切换、改名、删除。（P1）
2. 作为用户，我希望常用页面固定在顶部，标签可以拖动排序，常用操作有快捷键。（P2）
3. 作为 AI agent，我希望用 `alter nodejs` 脚本在用户已登录的 Space 里打开页面、读快照、点击和填表，以便替用户完成网页任务。（P1）
4. 作为用户，我希望看到 agent 在页面上做什么，遇到登录、验证码时它把控制权交给我，我做完再交还；我也能随时叫停它。（P1）
5. 作为用户，我希望用一个输入框完成打开网址、搜索和切换标签，能并排看两个页面，预览链接而不离开当前页，不常用的标签自动收起。（P2）
6. 作为从 Chrome / Arc 迁移过来的用户，我希望把书签和历史导入到某个 Space。（P2）（登录状态即 Cookie 的导入，以及主页、安装、skill、`alter upgrade` 的用户故事随后续 change 交付。）

## 节点
行为按节点写在 `flow.md` 和 `nodes/<node-slug>/requirement.md` 里。先做 N1（Space 与标签整理），它带来本需求唯一的数据库迁移，以及后面所有节点都依赖的 Space 语义和快捷键注册表。N2（agent 网关）是 N3、N5 的地基；N4 只依赖 N1。每个节点完成后都能单独演示：N1 之后是能用的多 Space 浏览器，N2 之后 agent 能跑脚本，N3 之后能接管交还，N4 之后有命令栏 / 分屏 / Peek / 归档，N5 之后能导入书签与历史。

## 非功能需求
- 性能（宪章门禁，architecture.md「横切关注点」）：`alter --version` 冷启动 < 20 ms；Gateway 从收到 `page.click` 到发出 CDP 命令的内部开销 < 5 ms；`page.snapshot` 对 2000 个节点的页面 < 150 ms（含 CDP 往返）；侧边栏在 500 个标签时切换 Space < 50 ms。各由一个 `#[ignore]` 基准或 e2e 计时守住，见成功标准。
- 安全：Gateway、CDP 转发端点只监听 127.0.0.1；`gateway.json` 与 `shell.sock` 权限 0600；Gateway 与 CDP 转发端点都要求 `Authorization: Bearer <token>`；Chromium 自己的 remote-debugging 端口只绑定 127.0.0.1，端口号只存在于 Chromium 写在数据目录里的 `ALTER_HOME/chromium/DevToolsActivePort` 和 alterd 的内存中，不另外写文件、不经 Gateway 暴露。
- 隐私：删除 Space 时清掉它的 Cookie、localStorage 和缓存（partition 目录）；导入只读源浏览器的数据，先拷贝到临时目录再读，读完删除临时副本。
- 可靠性：`E_USER_CONTROL` 不可绕过，SDK 不自动重试；alterd 仍是 SQLite 与 CDP 的唯一使用者（Electron Main 不碰数据库与 CDP）。
- 兼容性：macOS 13+（安装包随后续的发布 change）；Chrome / Arc 的数据格式以 2026-10 的稳定版为准，用 fixture 固定。
- 可观测性：每个 agent 动作写一条 receipt 日志（`task_space_id`、`label`、`method`、`duration_ms`、`ok`、`error_code`）；导入任务写开始、每类计数、结束三条日志；alterd 日志仍为 stderr 上的 JSON 行，由 Electron Main 转发。

## 成功标准
- SC1: `alter --version` 冷启动耗时 — measure: 100 次运行的中位数 < 20 ms — check: command `cargo test -p alter-cli --release --test perf -- --ignored --nocapture`
- SC2: Gateway 单动作内部开销 — measure: 1000 次 `page.click`（假 CDP 端）从收到请求到发出 CDP 命令的 p95 < 5 ms — check: command `cargo test -p alterd --release --test gateway_perf -- --ignored --nocapture`
- SC3: `page.snapshot` 耗时 — measure: 2000 个节点的本地测试页，10 次的 p95 < 150 ms — check: test `bun run e2e --grep SC3`
- SC4: 切换 Space 的界面耗时 — measure: 两个 Space 各 250 个标签，切换 20 次，从发出切换到侧边栏渲染完成的 p95 < 50 ms — check: test `bun run e2e --grep SC4`
- SC7: 全部门禁通过 — measure: `bun run ci:check` 与 `bun run e2e` 退出码为 0 — check: command `bun run ci:check && bun run e2e`

## 假设
- 节点按 N1 → N2 → N3 → N4 → N5 串行实现：N2 与 N4 共享协议、TabService 和侧边栏文件，并行只会制造冲突。
- page budget 默认 8，ownership=user 超过 30 分钟未交还标记 `inactive`，非固定标签 12 小时不活跃自动归档；三者都可在 `ALTER_HOME/config.toml` 改，测试用环境变量缩短（理由：沿用 data-model.md 与 architecture.md 的约定值）。
- 下载默认存到 `~/Downloads`，可用 config.toml 的 `download_dir` 改（理由：与 Chrome 默认一致，不另做下载界面）。
- 搜索引擎固定为 Google（沿用 tabs 规格的地址规范化规则，不做设置项）。
- 开发时 `alter nodejs` 用 `ALTER_BUN_PATH` 或 PATH 里的 `bun`（随 app 打包的 Bun 随后续的发布 change）。

## 待确认问题
- 无。

## 澄清记录
### Session 2026-10-21
- Q: 下一个需求只做 Space 管理，还是覆盖全部剩余核心功能？ → A: 「生成的需求是全部核心功能，而非只是 space」：一个路由 change 覆盖 Space、agent、浏览增强、导入、skill / 主页 / 发布。
- Q: 节点怎么拆？ → A: 6 个节点：space-tabs → agent-gateway → agent-handoff；browse-extras 在 space-tabs 之后；profile-import 在 agent-gateway 之后；release-site 最后。
- Q: Space 的范围（新建 / 编辑 / 切换 / 删除、隔离、固定、拖拽、快捷键）要不要再拆？ → A: 放在一个节点里，不再拆。
- Q: 删除 Space 时数据怎么处理？ → A: 彻底删除：先确认，硬删 Space 行并级联删除标签等数据，立即清掉该 partition 的 Cookie / storage，alterd 下次启动时删除 partition 目录；最后一个 Space 不能删。
- Q: 记住上次所在的 Space 用什么存？ → A: 迁移 0002 给 `spaces` 加 `last_active_at` 列（与标签「最大值即激活」同规则），并实现迁移前备份 `alter.db.bak-1`。
- Q: 快捷键做哪一组？ → A: 基础一组：Cmd+T、Cmd+W、Cmd+L、Cmd+R、Cmd+[ / Cmd+]、Cmd+1…9、Ctrl+Tab / Ctrl+Shift+Tab、Cmd+Shift+D。
- Q: Space 在侧边栏怎么呈现？ → A: 底部一排彩色圆点加「+」；点圆点切换，点「+」或右键圆点弹出 Renderer 内的编辑面板（名称、8 个预设颜色、编辑时有删除按钮）。
- Q: 固定标签怎么表现？ → A: Arc 式：固定分区在顶部，关闭固定标签时不移除，只卸载页面并回到固定时的 URL。
- Q: 拖拽排序的范围？ → A: 标签可拖，拖过固定分区的分隔线即固定或取消固定；Space 不可拖，按创建顺序排列。
- Q: DMG 要不要 Developer ID 签名与公证？ → A: 用户问「生成 install.sh 安装 app，是否不需要签名 / 公证」。结论：用 install.sh 安装时不需要。curl 下载的文件不带 `com.apple.quarantine` 属性，Gatekeeper 不检查；仍要做 ad-hoc 签名，因为 Apple Silicon 要求可执行代码至少有签名。在浏览器里直接下载的 DMG 会被拦，主页主推 install.sh，并写明「系统设置 → 隐私与安全性 → 仍要打开」的兜底办法。
- Q: 主页用什么做、什么语言？ → A: `site/` 手写纯静态 HTML / CSS，英文为主，中文页放 `site/zh/`，GitHub Actions 发布到 Pages。
- Q: 发布由谁触发？ → A: 推送 `v*` tag 时 GitHub Actions 在 macOS 上打包并创建 Release；install.sh 与 `alter upgrade` 读 latest release。
- Q: N5 的 Cookie 部分怎么办？（实现时生成 Keychain 与解密代码被安全分类器拦下） → A: 「cookie 部分后续实现」，之后在存档前选择「收窄后存档」：Cookie 导入移到后续 change。
- Q: 存档前 N6 release-site 还没开始，怎么办？ → A: 「收窄后存档」：删除节点 N6，发布另开后续 change。
- Q: R8 原场景导入三类含 Cookie，收窄后怎么定？ → A: 改为两类，省略 `items` 默认书签与历史。
- Q: R7（跳过损坏记录）唯一的场景是 Cookie 截断，怎么处理？ → A: 随 Cookie 一起移到后续 change。

## 变更记录
- 2026-10-21: 创建。
- 2026-10-21: amended: 收窄范围——删除节点 N6 release-site（发布另开后续 change）；N5 的 Cookie 导入移到后续 change（见 N5 的变更记录）；标题去掉「与发布」；成功标准去掉 SC5、SC6，SC7 去掉 `shellcheck scripts/install.sh`; revision: 2; re-approved by the user
