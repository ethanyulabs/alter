# install Specification

## Purpose
用一条命令安装或更新 alter.app 与 `alter` CLI。

## Requirements

### Requirement: 安装 app 并链接 CLI
系统 SHALL 在执行 `scripts/install.sh [--version v<x.y.z>] [--prefix <dir>] [--bin-dir <dir>]` 时下载对应版本（省略时为 latest）的 DMG 与 `.sha256`，校验通过后把 `alter.app` 复制到 `<prefix>`（默认 `/Applications`，不可写时为 `~/Applications`），把 `<bin-dir>/alter`（默认 `~/.local/bin/alter`）链接到 `alter.app/Contents/Resources/bin/alter`，最后打印 `alter <version> installed to <prefix>/alter.app`。

#### Scenario: 从假 Release 服务器安装指定版本
- WHEN 假 Release 服务器提供 `v0.2.0` 的假 app 包 DMG 与相符的 `.sha256`，以 `ALTER_RELEASE_BASE=<假服务器>` 执行 `bash scripts/install.sh --version v0.2.0 --prefix <临时目录 A> --bin-dir <临时目录 B>`
- THEN 退出码为 0，stdout 最后一行为 `alter 0.2.0 installed to <A>/alter.app`
- AND `<B>/alter` 是指向 `<A>/alter.app/Contents/Resources/bin/alter` 的符号链接，执行它输出 `alter 0.2.0`
- AND 安装过程中挂载的 DMG 已卸载（`hdiutil info` 里没有该镜像）

#### Scenario: 省略版本时安装 latest
- WHEN 假 Release 服务器的 `/releases/latest` 返回 `{"tag_name":"v0.3.0","html_url":"…"}` 并提供 `v0.3.0` 的假 app 包，以 `ALTER_RELEASE_API` 与 `ALTER_RELEASE_BASE` 指向它执行 `install.sh --prefix <A> --bin-dir <B>`
- THEN stdout 最后一行为 `alter 0.3.0 installed to <A>/alter.app`

#### Scenario: 覆盖已安装的旧版本
- WHEN `<A>/alter.app` 已是 `0.2.0` 的假 app 包，执行 `install.sh --version v0.3.0 --prefix <A> --bin-dir <B>`
- THEN `<B>/alter` 输出 `alter 0.3.0`，`<A>` 下只有 `alter.app` 一个条目

#### Scenario: bin 目录不在 PATH 时给出提示
- WHEN `PATH` 不包含 `<B>`，执行 `install.sh --version v0.2.0 --prefix <A> --bin-dir <B>`
- THEN stderr 有一行包含 `<B>` 与 `PATH` 的提示，退出码为 0

### Requirement: 校验失败时不安装
系统 SHALL 在 DMG 的 SHA256 与 `.sha256` 不符或下载失败时，以非 0 退出码结束，不改动 `<prefix>` 与 `<bin-dir>`。

#### Scenario: 校验和不符
- WHEN 假 Release 服务器上 `v0.2.0` 的 `.sha256` 与 DMG 不符，`<A>` 与 `<B>` 为空目录，执行 `install.sh --version v0.2.0 --prefix <A> --bin-dir <B>`
- THEN 退出码非 0，stderr 包含 `SHA256`，`<A>` 与 `<B>` 仍为空

#### Scenario: 版本不存在
- WHEN 假 Release 服务器对 `v9.9.9` 的资产返回 404，执行 `install.sh --version v9.9.9 --prefix <A> --bin-dir <B>`
- THEN 退出码非 0，stderr 包含 `v9.9.9`，`<A>` 与 `<B>` 仍为空

### Requirement: 预演与平台检查
系统 SHALL 在 `install.sh --dry-run` 时只按顺序打印将执行的步骤、不下载也不改动任何文件，并在系统不是 macOS 或 CPU 不是 arm64 时以退出码 1 拒绝安装。

#### Scenario: 预演
- WHEN 执行 `bash scripts/install.sh --dry-run --version v0.2.0 --prefix <A> --bin-dir <B> --with-skill`
- THEN 退出码为 0，stdout 依次有以 `[dry-run]` 开头、分别包含 `alter-0.2.0-arm64.dmg`、`SHA256`、`<A>/alter.app`、`<B>/alter`、`alter skill install` 的行
- AND `<A>` 与 `<B>` 仍为空，没有发出任何 HTTP 请求（假 Release 服务器的请求记录为空）

#### Scenario: 非 Apple Silicon
- WHEN 以 `ALTER_INSTALL_UNAME_M=x86_64` 执行 `install.sh --version v0.2.0 --prefix <A> --bin-dir <B>`
- THEN 退出码为 1，stderr 包含 `Apple Silicon`，`<A>` 为空

### Requirement: 安装时可选装 skill
系统 SHALL 在 `install.sh --with-skill` 时于安装完成后执行一次新装的 `alter skill install`，其输出原样转给用户；`alter skill install` 失败时安装本身仍算成功，并在 stderr 提示。

#### Scenario: 带 --with-skill 安装
- WHEN 假 app 包里的 `bin/alter` 被调用 `skill install` 时把参数写进 `<临时目录 C>/skill-args`，执行 `install.sh --version v0.2.0 --prefix <A> --bin-dir <B> --with-skill`
- THEN 退出码为 0，`<C>/skill-args` 的内容为 `skill install`
