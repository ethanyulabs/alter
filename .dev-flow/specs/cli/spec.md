# cli Specification

## Purpose
`alter` 命令行，agent 与用户共用。

## Requirements

### Requirement: 版本输出
系统 SHALL 在 `alter --version` 时打印 CLI 的名字与版本号。

#### Scenario: 打印版本
- WHEN 执行 `alter --version`
- THEN stdout 恰好为一行 `alter 0.1.0`
- AND 退出码为 0

### Requirement: 状态查询
系统 SHALL 在 `alter status` 时根据数据目录里的健康文件以及其中 pid 对应的进程是否存活来报告守护进程状态。

#### Scenario: 守护进程运行中
- WHEN `<ALTER_HOME>/alterd.json` 存在，其 `pid` 对应一个存活的进程（例如正在运行的 `alterd run`），`version` 为 `0.1.0`，执行 `alter status`
- THEN stdout 包含 `running`、该 pid 和 `0.1.0`
- AND 退出码为 0
- AND 改为执行 `alter status --json` 时，stdout 为一行 JSON `{"running": true, "pid": <该 pid>, "version": "0.1.0", "dbPath": "<ALTER_HOME>/alter.db"}`

#### Scenario: 没有健康文件
- WHEN `ALTER_HOME` 指向一个没有 `alterd.json` 的目录，执行 `alter status`
- THEN stdout 为一行 `not running`
- AND 退出码为 1
- AND 改为执行 `alter status --json` 时，stdout 为一行 JSON `{"running": false}`

#### Scenario: 健康文件残留
- WHEN `<ALTER_HOME>/alterd.json` 存在且其中 `pid` 为 999999（不存在的进程），执行 `alter status`
- THEN stdout 为一行 `not running (stale)`
- AND 退出码为 1
- AND 改为执行 `alter status --json` 时，stdout 为一行 JSON `{"running": false, "stale": true}`

### Requirement: alter nodejs
系统 SHALL 在 `alter nodejs` 时用 Bun 运行 `-e` 参数或 stdin 中的脚本，预置全局函数 `taskSpace`、`takeOverTaskSpace`、`claimTaskSpace`、`listTaskSpaces`，退出码与脚本相同；找不到 `gateway.json` 时以退出码 2 结束。

#### Scenario: -e 运行脚本
- WHEN app 在运行，执行 `alter nodejs -e 'const t = await taskSpace("demo"); console.log(t.page("p1").label)'`
- THEN stdout 为一行 `p1`，退出码为 0

#### Scenario: 从 stdin 读脚本
- WHEN app 在运行，以 heredoc 向 `alter nodejs` 的 stdin 写入 `console.log((await listTaskSpaces()).length >= 0)`
- THEN stdout 为一行 `true`，退出码为 0

#### Scenario: 脚本抛出异常
- WHEN 执行 `alter nodejs -e 'throw new Error("boom")'`
- THEN 退出码为 1，stderr 包含 `boom`

#### Scenario: alterd 未运行
- WHEN `ALTER_HOME` 指向一个没有 `gateway.json` 的临时目录，执行 `alter nodejs -e '1'`
- THEN 退出码为 2，stderr 包含 `alter.app`

### Requirement: alter spaces list
系统 SHALL 在 `alter spaces list` 时经 Gateway 打印全部 Space，每行 `<id>  <name>  <tabCount> tabs`；带 `--json` 时打印 `space.list` 的结果 JSON。

#### Scenario: 文本与 JSON 输出
- WHEN alterd 只有 `Personal`（0 个标签），执行 `alter spaces list` 与 `alter spaces list --json`
- THEN 第一个命令的 stdout 恰好一行，包含 `Personal` 与 `0 tabs`，退出码为 0
- AND 第二个命令的 stdout 是一行 JSON，`spaces[0].name` 为 `Personal`

### Requirement: alter import
系统 SHALL 在 `alter import --from chrome|arc [--profile <path>] --space <id> [--items …]` 时经 Gateway 运行导入，等待结束后打印各类计数；部分完成时退出码为 0 并在 stderr 打印 `warning: <原因>`；来源不存在时退出码为 1。

#### Scenario: 导入成功
- WHEN alterd 在运行，执行 `alter import --from chrome --profile <标准 fixture profile> --space <Work 的 id> --items bookmarks,history`
- THEN stdout 为一行 `imported bookmarks=3 history=2 cookies=0 skipped=0`，退出码为 0

#### Scenario: 来源不存在
- WHEN 执行 `alter import --from chrome --profile /nonexistent --space <id>`
- THEN 退出码为 1，stderr 包含 `E_IMPORT_SOURCE_NOT_FOUND`

#### Scenario: 钥匙串拒绝授权
- WHEN alterd 带 `ALTER_KEYCHAIN_FAKE=denied` 运行，执行 `alter import --from chrome --profile <有标准 Cookie fixture 的 profile> --space <id> --items cookies`
- THEN stdout 为一行 `imported bookmarks=0 history=0 cookies=0 skipped=0`，stderr 有一行 `warning: keychain_denied`，退出码为 0

#### Scenario: 帮助里不再说不支持 Cookie
- WHEN 执行 `alter import --help`
- THEN stdout 包含 `cookies`，且不包含 `暂不支持`

### Requirement: 从符号链接运行
系统 SHALL 在 `alter` 经符号链接调用时，按链接解析后的真实路径查找同目录的 `bun`、`../sdk/runtime.js` 与 `../skills/alter-browser`。

#### Scenario: 经 ~/.local/bin 的链接运行 alter nodejs
- WHEN 临时目录 `<P>/bin` 下有 `alter`（构建出的二进制的副本）与 `bun`（指向本机 Bun 的符号链接），`<P>/sdk/runtime.js` 是打包后的运行时，`<Q>/alter` 是指向 `<P>/bin/alter` 的符号链接，没有设置 `ALTER_BUN_PATH` 与 `ALTER_SDK_RUNTIME`，`PATH` 里没有 bun，alterd 在运行
- THEN `<Q>/alter nodejs -e 'console.log(typeof taskSpace)'` 输出 `function`，退出码为 0

### Requirement: alter skill install
系统 SHALL 在 `alter skill install [--target <dir>]…` 时把包内的 `alter-browser` skill 复制到每个目标目录下的 `alter-browser/`，覆盖已有的同名目录，并写入记录版本的 `.alter-skill`；省略 `--target` 时目标为 `~/.claude/skills` 与 `~/.codex/skills` 中已经存在的那些。

#### Scenario: 装到已存在的 agent 目录
- WHEN `HOME` 是一个临时目录，其中只有 `.claude/skills/` 存在，执行 `alter skill install`
- THEN 退出码为 0，stdout 恰好一行 `installed alter-browser -> <HOME>/.claude/skills/alter-browser`
- AND `<HOME>/.claude/skills/alter-browser/SKILL.md` 与源 skill 的 `SKILL.md` 内容相同，`.alter-skill` 内容为 `0.1.0`
- AND `<HOME>/.codex/skills` 不存在

#### Scenario: 两个目录都不存在
- WHEN `HOME` 是一个空的临时目录，执行 `alter skill install`
- THEN 退出码为 1，stderr 包含 `--target`

#### Scenario: 指定目录并覆盖旧版本
- WHEN `<D>/alter-browser/` 已有一个内容不同的 `SKILL.md` 和一个多余文件 `old.md`，执行 `alter skill install --target <D>`
- THEN `<D>/alter-browser/SKILL.md` 与源相同，`<D>/alter-browser/old.md` 不存在

### Requirement: alter skill uninstall
系统 SHALL 在 `alter skill uninstall [--target <dir>]…` 时删除目标目录下带 `.alter-skill` 标记的 `alter-browser/`，不带标记的目录不删除。

#### Scenario: 只删除自己装的
- WHEN `<HOME>/.claude/skills/alter-browser/` 由 `alter skill install` 装入，`<HOME>/.codex/skills/alter-browser/` 是用户手写的、没有 `.alter-skill`，执行 `alter skill uninstall`
- THEN `<HOME>/.claude/skills/alter-browser` 不存在，`<HOME>/.codex/skills/alter-browser` 仍在
- AND stderr 有一行包含 `.codex/skills/alter-browser` 与 `未删除`

### Requirement: alter upgrade
系统 SHALL 在 `alter upgrade [--check] [--version v<x.y.z>]` 时从 Release API 取最新版本并与自身版本比较：`--check` 只打印比较结果；有更新的版本时下载该版本的 `install.sh`，以 `--version`、当前 app 所在目录作为 `--prefix`、CLI 链接所在目录作为 `--bin-dir` 执行它，再对升级前装过 skill（带 `.alter-skill`）的每个目录执行新版的 `alter skill install --target`；取不到版本信息时退出码为 1。

#### Scenario: 检查到新版本
- WHEN 自身版本为 `0.1.0`，假 Release 服务器的 latest 为 `v0.2.0`，执行 `alter upgrade --check`
- THEN stdout 恰好一行 `alter 0.2.0 is available (current 0.1.0)`，退出码为 0
- AND 改为执行 `alter upgrade --check --json` 时，stdout 为一行 JSON `{"current":"0.1.0","latest":"0.2.0","updateAvailable":true}`

#### Scenario: 已是最新
- WHEN 自身版本为 `0.1.0`，latest 为 `v0.1.0`，执行 `alter upgrade`
- THEN stdout 恰好一行 `alter 0.1.0 is up to date`，退出码为 0，没有请求任何 `install.sh`

#### Scenario: 升级时调用 install.sh
- WHEN 构建出的 `alter` 被复制到临时目录 `<A>/alter.app/Contents/Resources/bin/alter`，`<B>/alter` 是指向它的符号链接，`<H>/.claude/skills/alter-browser/.alter-skill` 存在，latest 为 `v0.2.0`，假服务器上 `v0.2.0` 的 `install.sh` 把参数写进 `<C>/install-args`，以 `HOME=<H>` 执行 `<B>/alter upgrade`
- THEN 退出码为 0，`<C>/install-args` 的内容为 `--version v0.2.0 --prefix <A> --bin-dir <B>`
- AND 假 `install.sh` 把 `<A>/alter.app/Contents/Resources/bin/alter` 换成了一个把参数写进 `<C>/skill-args` 的脚本，升级结束后 `<C>/skill-args` 的内容为 `skill install --target <H>/.claude/skills`

#### Scenario: 取不到版本信息
- WHEN `ALTER_RELEASE_API` 指向一个没有监听的本地端口，执行 `alter upgrade --check`
- THEN 退出码为 1，stderr 包含 `无法获取最新版本`

#### Scenario: 不是从安装包运行
- WHEN latest 为 `v0.2.0`，执行 `target/debug/alter upgrade`（不在 `alter.app` 包内）
- THEN 退出码为 1，stderr 包含 `install.sh`，没有请求任何 `install.sh`
