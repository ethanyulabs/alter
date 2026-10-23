---
id: add-release-and-cookies
node: N2
title: 打包发布、安装、升级与更新提示
repos: [alter]
revision: 1
approved_revision: 1
---

# N2 打包发布、安装、升级与更新提示

<!-- 流程路线型需求中一个节点（子流程）的需求文档。这里写本节点的 Spec Delta；整体（背景、范围、非功能需求）在父级 requirement.md，节点顺序在 flow.md。保留字保持英文："Spec Delta:"、"ADDED/MODIFIED/REMOVED/RENAMED Requirement:"、"Scenario:"、WHEN/THEN/AND、SHALL/MUST。语法见 references/formats.md §3 和 §14 -->

## 范围
- 交付：
  - `bun run dist` 产出 Apple Silicon DMG。DMG 内的 `alter.app` 自带 alterd、`alter` CLI、Bun、打包好的 SDK 运行时和 agent skill，并做 ad-hoc 签名。
  - 推 `v*` tag 时，GitHub Actions 检查版本号、打包，并创建 Release。
  - `scripts/install.sh` 一键安装 app，并把 CLI 链接到 `~/.local/bin`。
  - `alter upgrade [--check]` 自更新，复用 install.sh。
  - `alter skill install` / `alter skill uninstall` 单独安装或删除 agent skill。
  - alterd 定期检查新版本，经 Gateway 通知 `app.updateAvailable` 与 Shell 快照告诉 agent 和用户；侧边栏显示提示条，`alter nodejs` 打一行 `[alter:notice]`。
- 交给下游（N3）：
  - install 命令：`curl -fsSL https://github.com/ethanyulabs/alter/releases/latest/download/install.sh | bash`。
  - 包内 skill 的位置：`alter.app/Contents/Resources/skills/alter-browser`。
  - `alter skill install` 命令。
  - Release 资产的命名：`alter-<version>-arm64.dmg`、`alter-<version>-arm64.dmg.sha256`、`install.sh`。
  - 本节点用一个占位的 `skills/alter-browser/SKILL.md` 验证打包与安装，正式内容由 N3 写。
- 依赖：N1 的 Shell 协议增量方式，以及向导文案样式，提示条沿用。
- 不做：
  - Developer ID 签名与公证。用 install.sh 安装时不需要，见 add-core-features 的澄清记录。
  - Intel / Universal 包。
  - app 内一键升级：提示条只给出命令与 Release 链接。
  - Homebrew cask。
  - 增量更新与回滚到旧版本：`alter upgrade --version` 可以指定版本，但不维护回滚记录。

术语：
- **Release API**：`ALTER_RELEASE_API`，默认 `https://api.github.com/repos/ethanyulabs/alter`。新版本信息取 `<Release API>/releases/latest` 的 `tag_name` 与 `html_url`。
- **Release 下载根**：`ALTER_RELEASE_BASE`，默认 `https://github.com/ethanyulabs/alter/releases/download`。资产地址为 `<下载根>/v<version>/<资产名>`。
- **假 Release 服务器**：测试在 127.0.0.1 上起的 HTTP 服务器，按路径返回 fixture 文件，用来替代 GitHub。
- **假 app 包**：测试用 `hdiutil create` 做的小 DMG，里面的 `alter.app/Contents/Resources/bin/alter` 是一个脚本，打印 `alter <version>`。

## Spec Delta: release
把仓库打包成可分发的 macOS 安装包并发布到 GitHub Releases。

### ADDED Requirement: R1 打包 DMG
系统 SHALL 在 `bun run dist` 时产出 `dist/alter-<version>-arm64.dmg` 与同名 `.sha256`，其中的 `alter.app` 含 `Contents/Resources/bin/{alterd,alter,bun}`、`Contents/Resources/sdk/runtime.js` 与 `Contents/Resources/skills/alter-browser/SKILL.md`，`CFBundleShortVersionString` 等于 Cargo workspace 版本，并通过 `codesign --verify --deep --strict`。

#### Scenario: R1.S1 DMG 的内容与签名
- WHEN Cargo workspace 版本为 `0.1.0`，执行 `bun run dist`
- THEN `dist/alter-0.1.0-arm64.dmg` 与 `dist/alter-0.1.0-arm64.dmg.sha256` 存在，后者的内容与 `shasum -a 256` 对 DMG 的计算结果一致
- AND 挂载 DMG 后，`alter.app/Contents/Resources/bin/alter --version` 输出 `alter 0.1.0`，`bin/bun --version` 输出 `.bun-version` 里的版本
- AND `Info.plist` 的 `CFBundleShortVersionString` 为 `0.1.0`，`codesign --verify --deep --strict alter.app` 退出码为 0

### ADDED Requirement: R2 打包后的 app 使用包内的程序
系统 SHALL 让打包后的 `alter.app` 启动包内的 `bin/alterd`，并让包内的 `bin/alter` 经 `nodejs` 子命令使用同目录的 `bun` 与 `../sdk/runtime.js`。

#### Scenario: R2.S1 打包后的 app 拉起包内 alterd
- WHEN 以临时目录为 `ALTER_HOME` 启动 `bun run dist` 产出的 `alter.app`（`--dir` 构建），且没有设置 `ALTER_ALTERD_PATH`
- THEN 10 秒内 `ALTER_HOME/alterd.json` 出现，其 `pid` 对应进程的可执行文件路径是 `alter.app/Contents/Resources/bin/alterd`
- AND 用包内的 `bin/alter nodejs -e 'console.log((await listTaskSpaces()).length >= 0)'` 输出 `true`，退出码为 0

### ADDED Requirement: R3 版本与 tag 一致才发布
系统 SHALL 在 `scripts/version-check.sh --tag <tag>` 时，当 `<tag>` 不是 `v` 加三处一致的版本号，以退出码 1 结束并打印 tag 与版本号。

#### Scenario: R3.S1 tag 与版本号不一致
- WHEN 三处版本号都是 `0.1.0`，执行 `bash scripts/version-check.sh --tag v0.2.0`
- THEN 退出码为 1，stderr 同时包含 `v0.2.0` 与 `0.1.0`

#### Scenario: R3.S2 tag 与版本号一致
- WHEN 三处版本号都是 `0.1.0`，执行 `bash scripts/version-check.sh --tag v0.1.0`
- THEN 退出码为 0

## Spec Delta: install
用一条命令安装或更新 alter.app 与 `alter` CLI。

### ADDED Requirement: R4 安装 app 并链接 CLI
系统 SHALL 在执行 `scripts/install.sh [--version v<x.y.z>] [--prefix <dir>] [--bin-dir <dir>]` 时下载对应版本（省略时为 latest）的 DMG 与 `.sha256`，校验通过后把 `alter.app` 复制到 `<prefix>`（默认 `/Applications`，不可写时为 `~/Applications`），把 `<bin-dir>/alter`（默认 `~/.local/bin/alter`）链接到 `alter.app/Contents/Resources/bin/alter`，最后打印 `alter <version> installed to <prefix>/alter.app`。

#### Scenario: R4.S1 从假 Release 服务器安装指定版本
- WHEN 假 Release 服务器提供 `v0.2.0` 的假 app 包 DMG 与相符的 `.sha256`，以 `ALTER_RELEASE_BASE=<假服务器>` 执行 `bash scripts/install.sh --version v0.2.0 --prefix <临时目录 A> --bin-dir <临时目录 B>`
- THEN 退出码为 0，stdout 最后一行为 `alter 0.2.0 installed to <A>/alter.app`
- AND `<B>/alter` 是指向 `<A>/alter.app/Contents/Resources/bin/alter` 的符号链接，执行它输出 `alter 0.2.0`
- AND 安装过程中挂载的 DMG 已卸载（`hdiutil info` 里没有该镜像）

#### Scenario: R4.S2 省略版本时安装 latest
- WHEN 假 Release 服务器的 `/releases/latest` 返回 `{"tag_name":"v0.3.0","html_url":"…"}` 并提供 `v0.3.0` 的假 app 包，以 `ALTER_RELEASE_API` 与 `ALTER_RELEASE_BASE` 指向它执行 `install.sh --prefix <A> --bin-dir <B>`
- THEN stdout 最后一行为 `alter 0.3.0 installed to <A>/alter.app`

#### Scenario: R4.S3 覆盖已安装的旧版本
- WHEN `<A>/alter.app` 已是 `0.2.0` 的假 app 包，执行 `install.sh --version v0.3.0 --prefix <A> --bin-dir <B>`
- THEN `<B>/alter` 输出 `alter 0.3.0`，`<A>` 下只有 `alter.app` 一个条目

#### Scenario: R4.S4 bin 目录不在 PATH 时给出提示
- WHEN `PATH` 不包含 `<B>`，执行 `install.sh --version v0.2.0 --prefix <A> --bin-dir <B>`
- THEN stderr 有一行包含 `<B>` 与 `PATH` 的提示，退出码为 0

### ADDED Requirement: R5 校验失败时不安装
系统 SHALL 在 DMG 的 SHA256 与 `.sha256` 不符或下载失败时，以非 0 退出码结束，不改动 `<prefix>` 与 `<bin-dir>`。

#### Scenario: R5.S1 校验和不符
- WHEN 假 Release 服务器上 `v0.2.0` 的 `.sha256` 与 DMG 不符，`<A>` 与 `<B>` 为空目录，执行 `install.sh --version v0.2.0 --prefix <A> --bin-dir <B>`
- THEN 退出码非 0，stderr 包含 `SHA256`，`<A>` 与 `<B>` 仍为空

#### Scenario: R5.S2 版本不存在
- WHEN 假 Release 服务器对 `v9.9.9` 的资产返回 404，执行 `install.sh --version v9.9.9 --prefix <A> --bin-dir <B>`
- THEN 退出码非 0，stderr 包含 `v9.9.9`，`<A>` 与 `<B>` 仍为空

### ADDED Requirement: R6 预演与平台检查
系统 SHALL 在 `install.sh --dry-run` 时只按顺序打印将执行的步骤、不下载也不改动任何文件，并在系统不是 macOS 或 CPU 不是 arm64 时以退出码 1 拒绝安装。

#### Scenario: R6.S1 预演
- WHEN 执行 `bash scripts/install.sh --dry-run --version v0.2.0 --prefix <A> --bin-dir <B> --with-skill`
- THEN 退出码为 0，stdout 依次有以 `[dry-run]` 开头、分别包含 `alter-0.2.0-arm64.dmg`、`SHA256`、`<A>/alter.app`、`<B>/alter`、`alter skill install` 的行
- AND `<A>` 与 `<B>` 仍为空，没有发出任何 HTTP 请求（假 Release 服务器的请求记录为空）

#### Scenario: R6.S2 非 Apple Silicon
- WHEN 以 `ALTER_INSTALL_UNAME_M=x86_64` 执行 `install.sh --version v0.2.0 --prefix <A> --bin-dir <B>`
- THEN 退出码为 1，stderr 包含 `Apple Silicon`，`<A>` 为空

### ADDED Requirement: R7 安装时可选装 skill
系统 SHALL 在 `install.sh --with-skill` 时于安装完成后执行一次新装的 `alter skill install`，其输出原样转给用户；`alter skill install` 失败时安装本身仍算成功，并在 stderr 提示。

#### Scenario: R7.S1 带 --with-skill 安装
- WHEN 假 app 包里的 `bin/alter` 被调用 `skill install` 时把参数写进 `<临时目录 C>/skill-args`，执行 `install.sh --version v0.2.0 --prefix <A> --bin-dir <B> --with-skill`
- THEN 退出码为 0，`<C>/skill-args` 的内容为 `skill install`

## Spec Delta: cli

### ADDED Requirement: R8 从符号链接运行
系统 SHALL 在 `alter` 经符号链接调用时，按链接解析后的真实路径查找同目录的 `bun`、`../sdk/runtime.js` 与 `../skills/alter-browser`。

#### Scenario: R8.S1 经 ~/.local/bin 的链接运行 alter nodejs
- WHEN 临时目录 `<P>/bin` 下有 `alter`（构建出的二进制的副本）与 `bun`（指向本机 Bun 的符号链接），`<P>/sdk/runtime.js` 是打包后的运行时，`<Q>/alter` 是指向 `<P>/bin/alter` 的符号链接，没有设置 `ALTER_BUN_PATH` 与 `ALTER_SDK_RUNTIME`，`PATH` 里没有 bun，alterd 在运行
- THEN `<Q>/alter nodejs -e 'console.log(typeof taskSpace)'` 输出 `function`，退出码为 0

### ADDED Requirement: R9 alter skill install
系统 SHALL 在 `alter skill install [--target <dir>]…` 时把包内的 `alter-browser` skill 复制到每个目标目录下的 `alter-browser/`，覆盖已有的同名目录，并写入记录版本的 `.alter-skill`；省略 `--target` 时目标为 `~/.claude/skills` 与 `~/.codex/skills` 中已经存在的那些。

#### Scenario: R9.S1 装到已存在的 agent 目录
- WHEN `HOME` 是一个临时目录，其中只有 `.claude/skills/` 存在，执行 `alter skill install`
- THEN 退出码为 0，stdout 恰好一行 `installed alter-browser -> <HOME>/.claude/skills/alter-browser`
- AND `<HOME>/.claude/skills/alter-browser/SKILL.md` 与源 skill 的 `SKILL.md` 内容相同，`.alter-skill` 内容为 `0.1.0`
- AND `<HOME>/.codex/skills` 不存在

#### Scenario: R9.S2 两个目录都不存在
- WHEN `HOME` 是一个空的临时目录，执行 `alter skill install`
- THEN 退出码为 1，stderr 包含 `--target`

#### Scenario: R9.S3 指定目录并覆盖旧版本
- WHEN `<D>/alter-browser/` 已有一个内容不同的 `SKILL.md` 和一个多余文件 `old.md`，执行 `alter skill install --target <D>`
- THEN `<D>/alter-browser/SKILL.md` 与源相同，`<D>/alter-browser/old.md` 不存在

### ADDED Requirement: R10 alter skill uninstall
系统 SHALL 在 `alter skill uninstall [--target <dir>]…` 时删除目标目录下带 `.alter-skill` 标记的 `alter-browser/`，不带标记的目录不删除。

#### Scenario: R10.S1 只删除自己装的
- WHEN `<HOME>/.claude/skills/alter-browser/` 由 `alter skill install` 装入，`<HOME>/.codex/skills/alter-browser/` 是用户手写的、没有 `.alter-skill`，执行 `alter skill uninstall`
- THEN `<HOME>/.claude/skills/alter-browser` 不存在，`<HOME>/.codex/skills/alter-browser` 仍在
- AND stderr 有一行包含 `.codex/skills/alter-browser` 与 `未删除`

### ADDED Requirement: R11 alter upgrade
系统 SHALL 在 `alter upgrade [--check] [--version v<x.y.z>]` 时从 Release API 取最新版本并与自身版本比较：`--check` 只打印比较结果；有更新的版本时下载该版本的 `install.sh`，以 `--version`、当前 app 所在目录作为 `--prefix`、CLI 链接所在目录作为 `--bin-dir` 执行它，再对升级前装过 skill（带 `.alter-skill`）的每个目录执行新版的 `alter skill install --target`；取不到版本信息时退出码为 1。

#### Scenario: R11.S1 检查到新版本
- WHEN 自身版本为 `0.1.0`，假 Release 服务器的 latest 为 `v0.2.0`，执行 `alter upgrade --check`
- THEN stdout 恰好一行 `alter 0.2.0 is available (current 0.1.0)`，退出码为 0
- AND 改为执行 `alter upgrade --check --json` 时，stdout 为一行 JSON `{"current":"0.1.0","latest":"0.2.0","updateAvailable":true}`

#### Scenario: R11.S2 已是最新
- WHEN 自身版本为 `0.1.0`，latest 为 `v0.1.0`，执行 `alter upgrade`
- THEN stdout 恰好一行 `alter 0.1.0 is up to date`，退出码为 0，没有请求任何 `install.sh`

#### Scenario: R11.S3 升级时调用 install.sh
- WHEN 构建出的 `alter` 被复制到临时目录 `<A>/alter.app/Contents/Resources/bin/alter`，`<B>/alter` 是指向它的符号链接，`<H>/.claude/skills/alter-browser/.alter-skill` 存在，latest 为 `v0.2.0`，假服务器上 `v0.2.0` 的 `install.sh` 把参数写进 `<C>/install-args`，以 `HOME=<H>` 执行 `<B>/alter upgrade`
- THEN 退出码为 0，`<C>/install-args` 的内容为 `--version v0.2.0 --prefix <A> --bin-dir <B>`
- AND 假 `install.sh` 把 `<A>/alter.app/Contents/Resources/bin/alter` 换成了一个把参数写进 `<C>/skill-args` 的脚本，升级结束后 `<C>/skill-args` 的内容为 `skill install --target <H>/.claude/skills`

#### Scenario: R11.S4 取不到版本信息
- WHEN `ALTER_RELEASE_API` 指向一个没有监听的本地端口，执行 `alter upgrade --check`
- THEN 退出码为 1，stderr 包含 `无法获取最新版本`

#### Scenario: R11.S5 不是从安装包运行
- WHEN latest 为 `v0.2.0`，执行 `target/debug/alter upgrade`（不在 `alter.app` 包内）
- THEN 退出码为 1，stderr 包含 `install.sh`，没有请求任何 `install.sh`

## Spec Delta: gateway

### ADDED Requirement: R12 更新提示通知
系统 SHALL 在 `config.toml` 没有设置 `update_check = false` 时，于 alterd 启动后与之后每 24 小时从 Release API 取一次最新版本；比自身新时，向当时已连接与之后连接的每个 Gateway 客户端各推送一次 `app.updateAvailable {version, url}`。检查失败时只记 warn 日志。

#### Scenario: R12.S1 启动后推送新版本
- WHEN 假 Release 服务器的 latest 为 `{"tag_name":"v9.0.0","html_url":"https://example.invalid/r/v9.0.0"}`，以 `ALTER_RELEASE_API=<假服务器>`、`ALTER_UPDATE_CHECK_DELAY_MS=0` 启动 alterd，Gateway 客户端连接并完成鉴权
- THEN 该客户端 3 秒内收到 `{"method":"app.updateAvailable","params":{"version":"9.0.0","url":"https://example.invalid/r/v9.0.0"}}`
- AND 之后再连接的客户端在鉴权后也收到同样的一条

#### Scenario: R12.S2 已是最新或关闭检查时不推送
- WHEN latest 为 alterd 自身版本，或 `config.toml` 中有 `update_check = false`（此时 latest 为 `v9.0.0`），以同样的环境启动 alterd 并连接 Gateway
- THEN 5 秒内客户端没有收到 `app.updateAvailable`
- AND `update_check = false` 时假 Release 服务器没有收到任何请求

#### Scenario: R12.S3 检查失败
- WHEN `ALTER_RELEASE_API` 指向一个没有监听的本地端口，`ALTER_UPDATE_CHECK_DELAY_MS=0`，启动 alterd
- THEN alterd 的 stderr 3 秒内有一行 `level` 为 `WARN`、消息包含 `检查更新失败` 的 JSON 日志，alterd 仍在运行

## Spec Delta: sdk

### ADDED Requirement: R13 打印更新提示
系统 SHALL 在 `alter nodejs` 运行的脚本收到 `app.updateAvailable` 时，向 stderr 打印恰好一次 `[alter:notice] alter <version> is available, run: alter upgrade`。

#### Scenario: R13.S1 收到两次通知只打印一次
- WHEN 假 Gateway 在 SDK 连接后推送两次 `app.updateAvailable {"version":"9.0.0","url":"…"}`
- THEN SDK 运行时的 stderr 恰好一行 `[alter:notice] alter 9.0.0 is available, run: alter upgrade`

## Spec Delta: app-shell

### ADDED Requirement: R14 侧边栏的更新提示条
系统 SHALL 在 Shell 快照的 `updateAvailable` 不为空时，在侧边栏底部 Space 圆点上方显示提示条「新版本 <version> 可用 · 运行 alter upgrade」；点提示条在当前 Space 新标签打开 `url`，点「×」在本次运行内隐藏它。

#### Scenario: R14.S1 显示并打开 Release 页
- WHEN 本地测试服务器提供 `/release` 页面，以 `ALTER_RELEASE_API` 指向一个返回 `{"tag_name":"v9.0.0","html_url":"http://127.0.0.1:<port>/release"}` 的假服务器、`ALTER_UPDATE_CHECK_DELAY_MS=0` 启动 app
- THEN 5 秒内侧边栏出现文字为「新版本 9.0.0 可用 · 运行 alter upgrade」的提示条
- AND 点它之后，当前 Space 新增一个激活的标签，URL 为 `http://127.0.0.1:<port>/release`
- AND 点提示条上的「×」后它不再可见

## Spec Delta: protocol

### ADDED Requirement: R15 导出更新提示类型
系统 SHALL 把更新提示的消息体与它在 Shell 快照中的字段导出为 TypeScript 声明。

#### Scenario: R15.S1 导出 UpdateAvailable
- WHEN 执行 `cargo run -p alter-core --bin export-ts -- <空的临时目录>`
- THEN 目录中存在 `UpdateAvailable.ts`，其字段恰好为 `version`、`url`
- AND 目录中存在 `ShellState.ts`，其字段包含 `updateAvailable`

## 变更记录
- 2026-10-22: 创建。
