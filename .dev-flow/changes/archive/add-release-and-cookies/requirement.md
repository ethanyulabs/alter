---
id: add-release-and-cookies
title: 补齐：Cookie 导入、发布安装、主页与 agent skill
type: feature
status: archived
revision: 1
approved_revision: 1
repos: [alter]
created: 2026-10-22
spec_base: spec-base.json
skip_specs: false
---

# 补齐：Cookie 导入、发布安装、主页与 agent skill

<!-- 流程路线型需求的父级需求文档。Spec Delta 写在每个节点自己的 requirement.md 里（nodes/<node-slug>/）；这份文件写整体：背景、变更内容、非功能需求、成功标准。正文用中文写。语法见 references/formats.md §14 -->

## 背景
add-core-features（PR #5，2026-10-22 存档）交付了 alter 的核心功能，但在 revision 2 收窄了范围，有两块移到了「后续 change」：
- 导入 Cookie：从 Chrome / Arc 导入时，用钥匙串取口令并解密，再写进 Space。没有它，迁移过来的用户要在每个网站重新登录。这是 architecture.md 里「首期导入书签 + 历史 + Cookie」这个决策的原意。
- 发布：DMG 安装包、推 tag 自动发布、`scripts/install.sh`、`alter upgrade`、`site/` 主页、`skills/alter-browser` agent skill。没有这些，除了开发者，没有人能装上 alter，agent 也不知道怎么用它。

AGENTS.md 的「现状」一节，以及 `.dev-flow/infra/` 下 overview.md、architecture.md、components.md、api.md 里标「待建立」或「后续 change」的部分，都是这两块。代码里也留好了接口：
- `ImportCounts.cookies`、`ImportStatus::Partial`、`imports.cookies_count` 已经存在，但含 `cookies` 的导入目前以 `E_INVALID_ARGUMENT` 拒绝（`crates/alterd/src/import/mod.rs:72-77`）。
- `apps/browser/src/main/daemon.ts:26` 与 `crates/alter-cli/src/nodejs.rs:81-114` 已经预留了打包态的路径。
- `electron-builder` 已是 devDependency。
- api.md 预留了 `app.updateAvailable`。

用户在 2026-10-22 提出「补齐完整项目开发」：一个需求把剩下的全部做完，分三个节点。

## 变更内容
- 导入向导、`alter import` 和 Gateway `import.run` 可以导入 Chrome / Arc 的 Cookie 到指定 Space。导入时 macOS 弹出钥匙串授权；用户拒绝时，书签和历史照常导入，结果显示「部分完成」。
- 新增 `bun run dist`，产出 Apple Silicon 的 `alter-<version>-arm64.dmg`，app 里自带 alterd、`alter`、Bun 和 agent skill。推送 `v*` tag 时 GitHub Actions 自动发布 Release。
- 新增 `curl -fsSL https://github.com/ethanyulabs/alter/releases/latest/download/install.sh | bash` 一键安装，CLI 链接到 `~/.local/bin/alter`。
- 新增 CLI 子命令：
  - `alter upgrade [--check]` 自更新。
  - `alter skill install|uninstall` 随时把 agent skill 装进或移出 `~/.claude/skills`、`~/.codex/skills`。
- alter 每 24 小时检查一次新版本。有新版本时：
  - 侧边栏显示提示条；
  - agent 脚本的 stderr 出现 `[alter:notice]` 提示；
  - `config.toml` 里设 `update_check = false` 可以关闭。
- 新增 GitHub Pages 主页（英文与中文各一页）、`skills/alter-browser` agent skill，README 增加安装说明。

## 范围
- 范围内：上面列出的全部，按 `flow.md` 拆成 3 个节点，每个节点的 Spec Delta、设计和任务写在 `nodes/<node-slug>/`。
- 范围外：
  - Developer ID 签名与公证（install.sh 安装的路径不需要，见 add-core-features 的澄清记录）；Intel / Universal 安装包；Homebrew。
  - app 内一键下载安装更新：提示条只给出 `alter upgrade` 命令和 Release 链接。
  - 导入密码、表单、扩展；Safari / Firefox；Windows / Linux 的 Cookie 格式。
  - `page.waitForFileChooser()`：仍然推迟，skill 里写明上传文件用 `setInputFiles`。
  - 站点生成器、自定义域名、站点统计；skill 的多语言版本。

## 用户故事
1. 作为从 Chrome / Arc 迁移过来的用户，我希望把网站的登录状态（Cookie）一起导入到某个 Space，以免逐个网站重新登录。（P1）
2. 作为想试用 alter 的用户，我希望用一条命令安装 app 和 CLI，并能用一条命令升级到新版本。（P1）
3. 作为用户，我希望 alter 有新版本时提示我，但不要擅自替我升级。（P2）
4. 作为 AI agent 的使用者，我希望随时用 `alter skill install` 把 alter 的 skill 装进我的 agent，让 agent 知道怎么在 alter 里干活、什么时候把控制权交给我。（P1）
5. 作为访客，我希望在主页上看懂 alter 是什么、怎么安装、浏览器下载被拦时怎么办。（P2）
6. 作为维护者，我希望推一个 tag 就自动打包并发布，版本号对不上时发布失败。（P2）

## 节点
行为按节点写在 `flow.md` 和 `nodes/<node-slug>/requirement.md` 里。N1（Cookie 导入）先做，它独立交付迁移用户最缺的登录状态，也是最可能卡住的部分；N2（打包、发布、安装、升级、更新提示）做完后，别人就能装上 alter；N3（主页与 skill）以 N2 定下的安装命令和包内 skill 路径为准，最后交付。

## 非功能需求
- 性能（宪章门禁）：
  - `alter --version` 冷启动仍然 < 20 ms：upgrade 与 skill 的代码只在各自的子命令里执行，不新增 HTTP 依赖。
  - Gateway 单动作内部开销仍然 < 5 ms。
  - 导入 1000 条 Cookie 的耗时 < 2 s。
- 安全与隐私：
  - Safe Storage 口令、派生密钥和解密后的 Cookie 只在内存中，不写日志、不落盘。它们在 alterd 与 Electron Main 之间只经 0600 的 `shell.sock` 传一次。
  - 测试不触发真实的钥匙串授权：钥匙串替身只在 debug 构建里生效。
  - install.sh 和 `alter upgrade` 先校验 SHA256 再安装，校验失败时不改动已有安装。
  - 更新检查只向 GitHub API 发 `User-Agent: alter/<version>`，不带任何标识；debug 构建默认不联网。
- 兼容性：macOS 13+、Apple Silicon；Chrome / Arc 的 Cookie 格式以 2026-10 的稳定版为准（`meta.version` 24，v10 密文），用 fixture 固定。
- 可观测性：
  - Cookie 导入记 `取 Safe Storage 口令`（带 `service`、`ok`）和 `导入完成一类`（`item=cookies`、`count`、`skipped`）两类日志。
  - 更新检查失败记 `WARN 检查更新失败`。
  - install.sh 的每一步都在 stdout 打一行进度。
- 可回滚：
  - Release 出问题时，把它标为 draft，latest 就回到上一版。
  - 用户用 `install.sh --version <旧版本>` 回退。
  - 更新提示可以用 `update_check = false` 关闭。

## 成功标准
- SC1: CLI 冷启动不回退 — measure: `alter --version` 100 次运行的中位数 < 20 ms — check: command `cargo test -p alter-cli --release --test perf -- --ignored --nocapture`
- SC2: Gateway 单动作开销不回退 — measure: 1000 次 `page.click` 的 p95 < 5 ms — check: command `cargo test -p alterd --release --test gateway_perf -- --ignored --nocapture`
- SC3: Cookie 导入耗时 — measure: 1000 条有效 Cookie（假 Electron 端立即回报）跑 5 次，中位数 < 2000 ms — check: command `cargo test -p alterd --test import_perf -- --ignored --nocapture`
- SC4: 安装脚本质量 — measure: `shellcheck -S style scripts/*.sh` 没有输出；`--dry-run` 的场景测试通过 — check: command `shellcheck -S style scripts/*.sh && cargo test -p alter-cli --test install_script`
- SC5: 全部门禁通过 — measure: `bun run ci:check` 与 `bun run e2e` 的退出码都为 0 — check: command `bun run ci:check && bun run e2e`
- SC6: 首次发布可用 — measure: 推送第一个 `v*` tag 后 release 工作流成功，Release 上有 DMG、`.sha256` 与 install.sh；在一台干净的 Apple Silicon Mac 上执行安装命令后，`alter --version` 输出该版本；Pages 主页可以访问 — check: monitored after release

## 假设
- 省略 `items` 时仍只导入书签与历史，Cookie 必须显式选择；向导里 Cookie 复选框默认勾选。理由：CLI 与 agent 静默触发钥匙串弹窗会让用户意外；在向导里用户看得到说明。
- 取不到 Safe Storage 口令时，不论是用户拒绝还是钥匙串里没有这一项，都记为 `keychain_denied`。理由：对用户来说处理办法相同，重新导入并允许授权即可。
- Cookie 写给最近一次上报 `cdpReady` 的 Shell 连接，也就是持有 Chromium 的 Electron Main；等待回报的时限为 10 秒。理由：只有它能写 partition，广播会把明文发给无关的客户端。
- app 装在 `/Applications`，不可写时装在 `~/Applications`；CLI 链接到 `~/.local/bin/alter`。理由：不需要 sudo，与常见 CLI 工具的做法一致。
- install.sh 遇到正在运行、且正是要替换的那个包的 alter 时，先退出它（10 秒不退出就放弃，不做任何改动），装完再打开。理由：升级是用户主动发起的，alterd 会随 app 一起退出。
- 首次检查更新在 alterd 启动 5 秒后，之后每 24 小时一次；debug 构建只在设置了 `ALTER_RELEASE_API` 时检查。理由：不拖慢启动，开发和测试时不联网。
- appId 为 `dev.ethanyulabs.alter`。理由：沿用 GitHub 组织名；这是落盘标识，决定之后不改。
- skill 用英文写，主页中英各一页，README 用中文。理由：与 ego-browser skill 一致；仓库文档用中文。
- 新测试统一带 `rc_` / `RC` 前缀（本需求 slug 的缩写）。理由：避免与 add-core-features 节点内同名的场景 ID（`N1 R1.S1` 等）在 `--grep` 时混在一起。

## 待确认问题
- 无。

## 澄清记录
### Session 2026-10-22
- Q: branch → A: 从 main 新建 `feature/add-release-and-cookies`（创建需求时仓库在 main 上，工作区干净）。
- Q: 这次「补齐完整项目开发」要覆盖哪些剩余内容？ → A: 全部四块：Cookie 导入、打包与发布、安装与升级、主页与 agent skill。
- Q: 节点怎么分？ → A: 3 个节点：N1 cookie-import；N2 release-install（DMG、Release 工作流、install.sh、alter upgrade）；N3 site-skill，排在 N2 之后。
- Q: 上一轮实现 Cookie 时，钥匙串与解密的代码被安全分类器拦下，这次怎么做？ → A: 照常实现：用 security-framework 取口令，用 RustCrypto 解密；如果再被拦，就把该任务标 blocked，其余继续。写入方式见下一条。
- Q: DMG 面向哪些 CPU 架构？ → A: 只出 Apple Silicon（arm64）。
- Q: 解密后的 Cookie 怎么写进目标 Space 的 partition？ → A: 用 Electron 的 cookies API：新增 Shell 指令 `shell.importCookies`，由 Main 调用 `session.fromPartition('persist:space-<id>').cookies.set` 写入并回报。
- Q: `alter upgrade` 怎么实现？ → A: 复用 install.sh：Rust 用 curl 取 latest 版本并比较，有新版本时下载该版本的 install.sh 并带 `--version` 执行；另有 `--check`。
- Q: 要不要做自动更新提示（`app.updateAvailable`）？ → A: 做。
- Q: agent skill 怎么装？ → A: 先选了 install.sh 的 `--with-skill`；审阅计划时改为「skills 可以使用 alter 命令单独安装，原因：用户可以后期自己安装」。于是新增 `alter skill install|uninstall`，install.sh 的 `--with-skill` 只是调用它，`alter upgrade` 之后刷新已经装过的 skill。

## 变更记录
- 2026-10-22: 创建。
