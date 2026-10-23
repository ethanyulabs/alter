# release Specification

## Purpose
把仓库打包成可分发的 macOS 安装包并发布到 GitHub Releases。

## Requirements

### Requirement: 打包 DMG
系统 SHALL 在 `bun run dist` 时产出 `dist/alter-<version>-arm64.dmg` 与同名 `.sha256`，其中的 `alter.app` 含 `Contents/Resources/bin/{alterd,alter,bun}`、`Contents/Resources/sdk/runtime.js` 与 `Contents/Resources/skills/alter-browser/SKILL.md`，`CFBundleShortVersionString` 等于 Cargo workspace 版本，并通过 `codesign --verify --deep --strict`。

#### Scenario: DMG 的内容与签名
- WHEN Cargo workspace 版本为 `0.1.0`，执行 `bun run dist`
- THEN `dist/alter-0.1.0-arm64.dmg` 与 `dist/alter-0.1.0-arm64.dmg.sha256` 存在，后者的内容与 `shasum -a 256` 对 DMG 的计算结果一致
- AND 挂载 DMG 后，`alter.app/Contents/Resources/bin/alter --version` 输出 `alter 0.1.0`，`bin/bun --version` 输出 `.bun-version` 里的版本
- AND `Info.plist` 的 `CFBundleShortVersionString` 为 `0.1.0`，`codesign --verify --deep --strict alter.app` 退出码为 0

### Requirement: 打包后的 app 使用包内的程序
系统 SHALL 让打包后的 `alter.app` 启动包内的 `bin/alterd`，并让包内的 `bin/alter` 经 `nodejs` 子命令使用同目录的 `bun` 与 `../sdk/runtime.js`。

#### Scenario: 打包后的 app 拉起包内 alterd
- WHEN 以临时目录为 `ALTER_HOME` 启动 `bun run dist` 产出的 `alter.app`（`--dir` 构建），且没有设置 `ALTER_ALTERD_PATH`
- THEN 10 秒内 `ALTER_HOME/alterd.json` 出现，其 `pid` 对应进程的可执行文件路径是 `alter.app/Contents/Resources/bin/alterd`
- AND 用包内的 `bin/alter nodejs -e 'console.log((await listTaskSpaces()).length >= 0)'` 输出 `true`，退出码为 0

### Requirement: 版本与 tag 一致才发布
系统 SHALL 在 `scripts/version-check.sh --tag <tag>` 时，当 `<tag>` 不是 `v` 加三处一致的版本号，以退出码 1 结束并打印 tag 与版本号。

#### Scenario: tag 与版本号不一致
- WHEN 三处版本号都是 `0.1.0`，执行 `bash scripts/version-check.sh --tag v0.2.0`
- THEN 退出码为 1，stderr 同时包含 `v0.2.0` 与 `0.1.0`

#### Scenario: tag 与版本号一致
- WHEN 三处版本号都是 `0.1.0`，执行 `bash scripts/version-check.sh --tag v0.1.0`
- THEN 退出码为 0
