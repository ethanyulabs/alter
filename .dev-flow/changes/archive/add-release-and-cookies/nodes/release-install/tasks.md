---
change: add-release-and-cookies
---

# 执行文档：打包发布、安装、升级与更新提示

<!-- 执行文档。每个任务是一个薄的垂直切片，能由一个测试单独证明。
行语法：- [ ] T<n> [covers:R1.S1] [after:T<k>] [P] <做什么> — seam: <测试驱动的公开入口> — verify: <命令>
有变更的场景用 RED/GREEN，已有行为用 BASELINE，没有 covers 的任务用 GREEN/VERIFY。
对应的证据写进 log.md 之后才能勾选 [x]。执行时做不完的任务标 [blocked:<原因>]，问题写进 log.md。
完整语法见 references/formats.md -->

## Repo: alter

全节点约定：
- 测试命名：Rust 用 `rc_n2_r<n>_s<m>_<描述>`；e2e 标题以 `RC N2 R<n>.S<m>` 开头。
- 不访问外网：
  - 所有 GitHub 交互都走假 Release 服务器，新文件 `crates/alter-cli/tests/common/release_server.rs`。
  - 服务器用 `std::net::TcpListener` 加一个线程实现，只支持 `GET`；按路径从一个 `HashMap<String, (u16, Vec<u8>)>` 返回状态码和内容，并把每个请求的路径记进 `Arc<Mutex<Vec<String>>>`。
  - 不引入 HTTP 依赖，dev 依赖也不引入。
  - alterd 的测试复用同一个实现：把它复制成 `crates/alterd/tests/common/release_server.rs`。两个 crate 的 tests/common 本来就各管各的，见现有的 `fake_cdp.rs`。
- 假 app 包：辅助函数 `fake_app_dmg(version, out_dir)` 生成 DMG。
  - 在临时目录里建 `alter.app/Contents/Resources/bin/alter`，内容是 bash 脚本：收到 `--version` 时打印 `alter <version>`；收到其他参数时，如果设置了 `ALTER_FAKE_ARGS_FILE`，就把参数追加写进它。
  - 再用 `hdiutil create -quiet -fs HFS+ -srcfolder <dir> -volname alter <out>.dmg` 打包，并计算 `.sha256`。
  - 这一步大约需要 1 秒，只在需要真实安装的用例里调用。
- 单向门：CLI 的退出码、输出格式、install.sh 的参数、`app.updateAvailable` 与 `ShellState.updateAvailable` 字段，都以 design.md「接口契约」为准，不得另加。
- shellcheck：本机没有时用 `brew install shellcheck`；装不上就把 T13 标为 `[blocked:shellcheck]`，其余继续。T13 同时把 shellcheck 加进 `ci:check` 与 `ci.yml`。

- [x] T1 [covers:R15.S1] 协议：`UpdateAvailable` 与 `ShellState.updateAvailable`；配置项 `update_check` — seam: `cargo run -p alter-core --bin export-ts` — verify: `cargo test -p alter-core --test export_ts rc_n2_r15`
  - 新测试仿照 `crates/alter-core/tests/export_ts.rs:191`。`UpdateAvailable` 放在 `crates/alter-core/src/protocol/gateway.rs`。
  - `ShellState`（`shell.rs:68`）加 `#[serde(default, skip_serializing_if = "Option::is_none")] pub update_available: Option<UpdateAvailable>`。没有更新时快照里不带这个字段，现有的 `shell.rs` 契约测试不受影响。
  - `crates/alter-core/src/config.rs:11` 加 `update_check: Option<bool>`。
  - 然后执行 `bun run protocol:export`。

- [x] T2 [covers:R3.S1,R3.S2] `version-check.sh --tag` — seam: `bash scripts/version-check.sh --tag <tag>` — verify: `cargo test -p alter-cli --test release_scripts rc_n2_r3`
  - 新文件 `crates/alter-cli/tests/release_scripts.rs`，在仓库根目录执行脚本。仓库根目录用 `env!("CARGO_MANIFEST_DIR")/../..` 求得。
  - 先比较三处版本号；一致之后再比较 tag。tag 不一致时 stderr 输出 `tag <tag> 与版本号 <v> 不一致`，退出码 1。
  - 不带 `--tag` 时行为不变，`bun run version:check` 照常通过。

- [x] T3 [covers:R1.S1] 打包：`scripts/dist.sh`、`fetch-bun.sh`、`check-bundle.sh`、`electron-builder.yml`，以及占位的 `skills/alter-browser/SKILL.md` — seam: `bun run dist` + `bash scripts/check-bundle.sh` — verify: `bun run dist && bash scripts/check-bundle.sh dist/mac-arm64/alter.app 0.1.0`
  - 先写 `check-bundle.sh <app> <version>`，在还没有打包的 `.app` 上确认它失败（RED），再写打包部分。
  - `check-bundle.sh` 断言以下事实，任何一条不成立就 `exit 1` 并打印原因：
    - 文件布局：`bin/{alterd,alter,bun}` 可执行，`sdk/runtime.js` 存在，`skills/alter-browser/SKILL.md` 存在。
    - `bin/alter --version` 的输出为 `alter <version>`。
    - `bin/bun --version` 的输出等于 `.bun-version`。
    - `/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString'` 的结果为 `<version>`。
    - `codesign --verify --deep --strict` 通过。
    - DMG 旁的 `.sha256` 与 `shasum -a 256` 的计算结果一致。DMG 不存在时（`--dir` 构建）跳过这一项。
  - 根 `package.json` 加两个脚本：`"dist": "bash scripts/dist.sh"`，`"dist:dir": "bash scripts/dist.sh --dir"`。
  - `scripts/dist.sh` 依次执行：
    1. `cargo build --release -p alterd -p alter-cli`，然后复制到 `build/bin/`。`build/` 已在 `.gitignore` 里则保持；不在就加上，`dist/` 同理。
    2. `scripts/fetch-bun.sh build/bin/bun`。下载地址是 `https://github.com/oven-sh/bun/releases/download/bun-v$(cat .bun-version)/bun-darwin-aarch64.zip`，用同一 release 的 `SHASUMS256.txt` 校验。
    3. `bun build packages/sdk/src/runtime.ts --target bun --outfile build/sdk/runtime.js`。
    4. `bun run build`。
    5. `cd apps/browser && bunx electron-builder -c electron-builder.yml --mac --arm64`；带 `--dir` 时改为 `--dir`。
    6. `cd dist && shasum -a 256 alter-<v>-arm64.dmg > alter-<v>-arm64.dmg.sha256`。
  - `electron-builder.yml` 的字段按 design.md「组件」一行一行写。如果 `identity: "-"` 没有产生签名（`check-bundle` 的 codesign 失败），在 `apps/browser/build/after-sign.cjs` 里执行 `codesign --force --deep -s - <appOutDir>/alter.app`，并记一条 `decided:`。
  - `runtime.ts` 打包后，`find_runtime` 要能找到 `<exe 目录>/../sdk/runtime.js`（`nodejs.rs:100-114` 已有这条路径），这一点由 T4 证明。
  - 占位 skill：`skills/alter-browser/SKILL.md` 只有 front matter（`name: alter-browser`，`description` 写一句占位）和一行「完整内容由 N3 提供」，正式内容由 N3/T3 替换。
  - 前置条件：需要联网下载 Bun 和 Electron。没有网络时把本任务和 T4 标为 `[blocked:network]`，其余继续。

- [x] T4 [covers:R2.S1] [after:T3,T5] 打包后的 app 使用包内的 alterd、bun 和 runtime — seam: Playwright `_electron.launch({executablePath})`，新文件 `apps/browser/e2e/packaged.spec.ts` — verify: `bun run dist:dir && bun run e2e --grep "RC N2 R2"`
  - `executablePath` 为 `dist/mac-arm64/alter.app/Contents/MacOS/alter`。环境变量里删掉 `ALTER_ALTERD_PATH`、`ALTER_BUN_PATH` 和 `ALTER_SDK_RUNTIME`；`ALTER_HOME` 用临时目录。
  - 用 `ps -o comm= -p <pid>` 读进程的可执行文件路径，断言它以 `alter.app/Contents/Resources/bin/alterd` 结尾。
  - 包内的 `bin/alter nodejs` 用 `execFileSync` 调用，`PATH` 设成 `/usr/bin:/bin`，确保系统里的 bun 不会被用到。
  - `dist/mac-arm64` 不存在时，这个用例 `test.skip` 并给出明确原因，免得默认的 `bun run e2e` 因为没打包而失败。verify 命令先执行 `dist:dir`，所以本任务里它一定会真正跑一遍。

- [x] T5 [covers:R8.S1] `exe::real_exe()`：经符号链接运行时找同目录的 bun 与 runtime — seam: `alter nodejs`（`assert_cmd`，测试里起的 alterd） — verify: `cargo build -p alterd && cargo test -p alter-cli --test nodejs rc_n2_r8`
  - 新文件 `crates/alter-cli/src/exe.rs`。`nodejs.rs:81-114` 的 `current_exe()` 都改为调用 `exe::real_exe()`。
  - 测试仿照 `crates/alter-cli/tests/nodejs.rs` 现有的启动方式，用临时 `ALTER_HOME` 起 alterd。`<P>/sdk/runtime.js` 由测试执行 `bun build packages/sdk/src/runtime.ts --target bun --outfile …` 生成。
  - `PATH` 设成 `/usr/bin:/bin`，同时删掉 `ALTER_BUN_PATH` 和 `ALTER_SDK_RUNTIME`。
  - 先 RED：在不改代码时测试应当失败，报找不到 bun 或者用了开发态的 runtime。如果因为 `current_exe` 在 macOS 上恰好已经解析了链接而一写就通过，就做变异（让 `real_exe` 不再 canonicalize），确认测试会失败。

- [x] T6 [covers:R9.S1,R9.S2,R9.S3] [after:T5] `alter skill install` — seam: `alter skill install`（`assert_cmd`，`HOME` 指向临时目录） — verify: `cargo test -p alter-cli --test skill rc_n2_r9`
  - 新文件 `crates/alter-cli/src/skill.rs`；clap 子命令仿照 `main.rs:13-63` 的 `Spaces`。
  - 源目录的查找顺序见 design.md「组件」。测试设置 `ALTER_SKILL_DIR` 指向一个临时的 fixture skill，不依赖仓库里那个会被 N3 改写的 SKILL.md。
  - 复制目录用 `std::fs` 递归实现，不引入 `fs_extra`。符号链接不跟随，原样复制成链接。
  - 边界情况：
    - `--target` 可以重复传。
    - 目标目录不存在时，先 `create_dir_all` 再安装。显式指定的目标照装；只有默认目标才要求目录已经存在。
    - 同一个目录传了两次，只装一次。

- [x] T7 [covers:R10.S1] [after:T6] `alter skill uninstall` — seam: `alter skill uninstall` — verify: `cargo test -p alter-cli --test skill rc_n2_r10`

- [x] T8 [covers:R4.S1,R4.S2,R4.S3,R4.S4] [after:T2] `scripts/install.sh` 的安装路径 — seam: `bash scripts/install.sh`（`assert_cmd` + 假 Release 服务器 + 假 app 包） — verify: `cargo test -p alter-cli --test install_script rc_n2_r4`
  - 新文件 `scripts/install.sh` 与 `crates/alter-cli/tests/install_script.rs`。步骤以 design.md「接口契约」的第 1–11 步为准。
  - 测试里的 `--prefix` 都是非默认目录，所以第 6 步（退出和重新打开运行中的 app）不会执行。这一步只在 `--dry-run` 的输出里出现，由 T9 断言。
  - 先写 R4.S1：脚本还不存在，测试应当失败。R4.S2、R4.S3、R4.S4 各自都能先失败：最小实现只支持 `--version`，没有 latest 解析，没有 `.new` 替换，也没有 PATH 提示。
  - 卸载 DMG：用 `hdiutil info` 的输出断言镜像已不在。
  - 并发：每个用例用自己的临时目录和自己的服务器端口，所以可以并行。`hdiutil attach` 的挂载点在各自的临时目录里，互不冲突。

- [x] T9 [covers:R5.S1,R5.S2,R6.S1,R6.S2] [after:T8] install.sh 的校验失败、版本不存在、预演与平台检查 — seam: `bash scripts/install.sh` — verify: `cargo test -p alter-cli --test install_script rc_n2_r5 && cargo test -p alter-cli --test install_script rc_n2_r6`
  - R6.S1 断言服务器的请求记录为空，以及各行的先后顺序。`--dry-run` 下 latest 也不解析：省略 `--version` 时打印 `[dry-run] 查询 latest 版本`。这一条没有单独的场景，用一个 `#[test]` 顺带断言。
  - 额外断言一条 `[dry-run]` 行：设置 `ALTER_INSTALL_ASSUME_RUNNING=1` 并使用默认 prefix 时，会打印「退出正在运行的 alter」与「重新打开 alter」。这是测试用的环境变量，写进脚本头部的注释，但不写进 `--help`。（T18 起已删掉这个变量，改为在 PATH 里放假的 `lsappinfo`，见 CR11。）

- [x] T10 [covers:R7.S1] [after:T8,T6] install.sh 的 `--with-skill` — seam: `bash scripts/install.sh --with-skill` — verify: `cargo test -p alter-cli --test install_script rc_n2_r7`
  - 假 app 包里的 `bin/alter` 在设置了 `ALTER_FAKE_ARGS_FILE` 时记录收到的参数，见全节点约定。

- [x] T11 [covers:R11.S1,R11.S2,R11.S4,R11.S5] [after:T5] `alter upgrade` 的版本检查与前提条件 — seam: `alter upgrade`（`assert_cmd` + 假 Release 服务器） — verify: `cargo test -p alter-cli --test upgrade rc_n2_r11`
  - 新文件 `crates/alter-cli/src/upgrade.rs`。
  - 版本比较：用 `env!("CARGO_PKG_VERSION")` 和 `tag_name` 比较，去掉开头的 `v`，再按 `.` 拆成三个 `u64`。不是三段数字时当作取不到版本信息。
  - 测试里让版本看起来是 `0.1.0`，就用 workspace 当前的版本。版本号升级以后，测试应当读 `CARGO_PKG_VERSION`，不能写死。场景里的 `0.1.0` 只是举例。
  - 冷启动：upgrade 的代码只在子命令分支里执行。`alter --version` 的冷启动基准（`crates/alter-cli/tests/perf.rs`）在 T14 跑一次，中位数必须仍 < 20 ms。

- [x] T12 [covers:R11.S3] [after:T11,T6,T8] `alter upgrade` 调用 install.sh 并刷新 skill — seam: `<B>/alter upgrade`（复制进假 `alter.app` 的真实 alter 二进制） — verify: `cargo test -p alter-cli --test upgrade rc_n2_r11_s3`
  - 假 `install.sh` 由测试写到服务器上。它做两件事：把 `"$@"` 写进 `<C>/install-args`；再用一个记录参数的脚本替换 `<A>/alter.app/Contents/Resources/bin/alter`，那个脚本收到参数时写 `<C>/skill-args`。
  - `--bin-dir`：测试从 `<B>/alter` 调用。`argv[0]` 是 `<B>/alter`，不等于 `real_exe`，所以按 design.md 的规则取 `<B>`。
  - 边界：install.sh 失败（非 0）时，不刷新 skill，退出码为 1，stderr 包含 `升级失败`。用一个 `#[test]` 顺带断言。

- [x] T13 [after:T2,T3,T8] shellcheck 门禁与 release 工作流 — seam: `shellcheck` + `.github/workflows/release.yml` — verify: `shellcheck -S style scripts/*.sh && bun run ci:check`
  - 根 `package.json` 加 `"lint:sh": "shellcheck -S style scripts/*.sh"`，并接进 `ci:check`。
  - `ci.yml` 在 `bun run ci:check` 之前加一步 `brew install shellcheck`；macos-latest 镜像如果自带 shellcheck，这一步会自动跳过。
  - `release.yml` 按 design.md「组件」的那一行编写。`gh` 用 `GITHUB_TOKEN`，权限为 `contents: write`。
  - 工作流本身只能靠推 tag 验证，对应父级 SC6 `monitored after release`。本地只验证 YAML 能被 `ruby -ryaml -e 'YAML.load_file(...)'` 解析，记为 VERIFY。

- [x] T14 [covers:R12.S1,R12.S2,R12.S3] [after:T1] alterd 检查更新并推送 `app.updateAvailable` — seam: Gateway 客户端（`crates/alterd/tests/gateway.rs`）+ 假 Release 服务器 — verify: `cargo test -p alterd --test gateway rc_n2_r12 && cargo test -p alter-cli --release --test perf -- --ignored --nocapture`
  - 新文件 `crates/alterd/src/update.rs`。`lifecycle.rs` 在 Gateway 启动之后 spawn 它，读 `config.update_check`。
  - Gateway 的推送仿照 `gateway/events.rs` 里 `taskSpace.ownershipChanged` 的广播方式。新连接在鉴权成功后，如果有 `update_available` 就补推一次。
  - debug 构建只在设置了 `ALTER_RELEASE_API` 时检查。这样现有测试和开发环境都不会联网，也就不需要再给 `Daemon::spawn` 加默认环境变量。
  - R12.S2 的「5 秒内没有收到」：用 `ws_recv_within(…, 5s)` 断言收到的是 `None`，同时断言服务器没有收到请求（`update_check = false` 的情况）。
  - 版本号比较的逻辑与 CLI 的 `upgrade.rs` 重复，大约十几行。不为它抽共享 crate，两边各写一份并各自配单元测试。这条在 log 里记为 `decided:`，理由是 alter-core 不应该依赖进程调用。
  - verify 里顺带跑 CLI 冷启动基准，确认 T11 之后 `alter --version` 仍 < 20 ms（父级 SC1）。

- [x] T15 [covers:R13.S1] [after:T14] SDK 打印更新提示 — seam: SDK 运行时 + 假 Gateway（`packages/sdk/tests/fake-gateway.ts`） — verify: `bun test packages/sdk/tests/update-notice.test.ts`
  - `packages/sdk/src/runtime.ts` 在创建 client 时注册一个 listener（`client.ts:120` 已有的通知分发），收到 `app.updateAvailable` 时用一个模块级的布尔值保证只打印一次。
  - 测试在子进程里运行 runtime，以便捕获 stderr。仿照 `smoke.test.ts` 的启动方式；如果它没有子进程的写法，就用 `Bun.spawn` 并设置 `ALTER_HOME` 指向假 Gateway。

- [x] T16 [covers:R14.S1] [after:T14] 侧边栏的更新提示条 — seam: Electron e2e（新文件 `apps/browser/e2e/update.spec.ts`） — verify: `bun run e2e --grep "RC N2 R14"`
  - 新组件 `UpdateBanner.tsx`，挂在 `Sidebar.tsx` 的 SpaceBar 上方，样式沿用 N1/T8 向导里提示文案的样式。
  - 点击时经 preload 已有的 IPC 发 `openRequested`，即当前 Space、`input` 为 url。「×」把版本号记进组件的 signal，不持久化。
  - 假 Release 服务器在 e2e 里用 `e2e/fixtures/server.ts` 新增的 `/api/releases/latest` 路由实现，返回指向同一台服务器 `/release` 的 `html_url`。用 `launchApp({ALTER_RELEASE_API: "http://127.0.0.1:<port>/api", ALTER_UPDATE_CHECK_DELAY_MS: "0"})` 启动。
  - 计时用 `expectAllWithin`（`e2e/ui.ts`），`t0` 在 `launchApp` 之前取。

- [x] T17 [after:T1,T2,T3,T4,T5,T6,T7,T8,T9,T10,T11,T12,T13,T14,T15,T16] 节点门禁 — seam: 仓库门禁 — verify: `bun run ci:check && bun run e2e --grep "RC N2"`
  - 覆盖率：`cargo llvm-cov -p alter-cli` 检查 `skill.rs`、`upgrade.rs`、`exe.rs`，`-p alterd` 检查 `update.rs`；`bun test --coverage` 检查 `runtime.ts` 的改动行。要求 ≥ 80%。
  - install.sh 的覆盖率无法用工具测量，以 R4–R7 的场景表代替，在 log 里注明。

## Convergence

- [x] T18 [covers:R4.S3,R6.S1] install.sh 的失败回退、运行中 app 的判断与退出、detach 失败、相对路径（CR1、CR2、CR3、CR11、CR13、CR14） — seam: `bash scripts/install.sh`（假 Release 服务器 + PATH 里的假 lsappinfo） — verify: `cargo test -p alter-cli --test install_script`
  - 替换：旧的 alter.app 先改名为 `alter.app.old`，再把 `.new` 改名为 alter.app，链接成功后删除 `.old`；中途任何一步失败时，由 EXIT trap 把 `.old` 恢复回来。
  - 运行中的 app：用 `lsappinfo info -only pid,bundlepath -app dev.ethanyulabs.alter` 取 pid 与包路径。只有包路径正是 `<prefix>/alter.app` 时才去退出：`kill -TERM <pid>`，等它退出，最多 10 秒，仍没退出就报错，并且不做任何改动。装完、或中途失败时，都用 `open` 把它重新打开。这样不需要 AppleScript 的自动化授权，删掉 `ALTER_INSTALL_ASSUME_RUNNING`；预演用同一个判断。
  - 测试：在 PATH 前面放一个假的 `lsappinfo`（报一个真实 `sleep` 进程的 pid 与 `<A>/alter.app`）和一个假的 `open`（记录参数），用来测：
    - 退出并重开；
    - 进程不退出时（忽略 TERM 的子进程）安装失败且 `<A>` 没有被改动；
    - 包路径不同时不去碰它。
  - 回退的测试：先装 0.2.0，再以一个普通文件作为 `--bin-dir` 安装 0.3.0；断言非 0 退出，`<A>/alter.app` 仍是 0.2.0，没有 `.old`、`.new` 残留。
  - detach：只有成功时才清除 mounted 标记；最终仍失败时 stderr 提示残留的挂载并以非 0 退出。
  - `--prefix`、`--bin-dir` 先转成绝对路径；测试用相对路径加含空格的目录。
  - design.md 的接口契约第 6、7 步与父级 design 的环境变量表同步修改。
- [x] T19 [covers:R11.S1,R11.S2,R11.S5] upgrade：latest 不比自身新时一律 up to date；`--json` 必须带 `--check`；`--version` 校验；严格的包路径；独占的临时目录（CR4、CR5、CR12、CR15、CR16） — seam: `alter upgrade`（`assert_cmd` + 假 Release 服务器） — verify: `cargo test -p alter-cli --test upgrade && cargo test -p alter-cli --bin alter upgrade`
  - 不带 `--version` 时：`!is_newer(latest, current)` 即打印 up to date。显式 `--version` 才允许装更旧的版本。
  - clap：`json` 加 `requires = "check"`，非法组合是 clap 的用法错误，退出码 2，不访问网络。
  - `--version`：必须是 `v` 加三段整数，否则退出码 1，stderr `版本号应为 v<x.y.z>`，并且不访问网络。
  - bundle_root：要求恰好 `<x>.app/Contents/Resources/bin/alter`。
  - 临时目录：`temp_dir()/alter-upgrade-<pid>-<nanos>`，用 `create_dir`（已存在就失败）并设为 0700，脚本用 `OpenOptions::create_new` 写入。
  - 测试：
    - latest 比自身旧时 up to date，不请求 install.sh；
    - `--json` 不带 `--check` 时退出码 2；
    - `--version foo` 时退出码 1；
    - `x.app/Other/Resources/bin/alter` 被当成不在包内。
- [x] T20 [covers:R9.S3] skill install 先复制到同级的暂存目录，写好标记后再替换；拒绝源与目标相同（CR6） — seam: `alter skill install` — verify: `cargo test -p alter-cli --test skill`
  - 先复制到 `<dir>/.alter-browser.tmp-<pid>`，写 `.alter-skill`，再把旧目录改名为 `.alter-browser.old-<pid>`，把暂存目录改名为 alter-browser，最后删除 old；任何一步失败时清理暂存目录，并把 old 恢复回来。
  - 源目录与目标的 alter-browser 是同一个（canonicalize 后相等），或者一个包含另一个时，退出码 1，stderr `skill 的源与目标相同`。
  - 测试：源里放一个权限为 000 的文件让复制失败，断言退出码 1，旧的 SKILL.md 保持不变；`ALTER_SKILL_DIR` 指向目标里的 alter-browser 时被拒绝。
- [x] T21 [covers:R12.S1] Gateway 的补推与广播放进同一把锁；R12.S1 证明「广播给已连接的客户端」；curl 不读 .curlrc；html_url 校验；按版本去重（CR7、CR8、CR9、CR17、CR18） — seam: Gateway 客户端 + 假 Release 服务器（能扣住响应、记录请求头） — verify: `cargo test -p alterd --test gateway rc_n2_r12 && cargo test -p alterd --test gateway rc_n2_t21 && cargo test -p alterd --bin alterd update`
  - Gateway：`update` 与 `listeners` 合成一个 `Mutex<(已推版本集合, 当前更新, Vec<Listener>)>`，或者在登记 listener 与补推时持有 update 锁，锁顺序固定为先 update 后 listeners。按 version 去重，用 HashSet 记本次运行里推过的版本。
  - update.rs：curl 的第一个参数是 `-q`（不读 .curlrc）。html_url 必须是 `https://`，或 `http://127.0.0.1` 开头（本机测试），否则记为检查失败。
  - 假 Release 服务器（alterd 的 tests/common 那份）：增加「扣住某个路径的响应，直到测试放行」和「记录请求头」。
  - 测试：
    - R12.S1 改为：扣住 latest，先连上客户端，再放行；断言恰好收到一条，1 秒内没有第二条。之后连上的客户端也恰好一条。
    - rc_n2_t21_ignores_curlrc：HOME 里放一个 `.curlrc`，内容为 `header = "X-Leak: 1"`，断言服务器收到的请求头里没有 X-Leak。
    - rc_n2_t21_bad_html_url：html_url 为 `javascript:alert(1)` 时不推送，并记 WARN。
- [x] T22 release 工作流的 action 钉到 commit SHA（CR10） — seam: `.github/workflows/release.yml` — verify: `ruby -ryaml -e 'YAML.load_file(".github/workflows/release.yml")'`
  - 用 `gh api` 查 actions/checkout@v4、dtolnay/rust-toolchain@stable、Swatinem/rust-cache@v2、oven-sh/setup-bun@v2 当前指向的 commit，写成 `@<sha> # <tag>`。checkout 加 `persist-credentials: false`。
  - 前提是要联网。查不到时标 `[blocked:network]`。
- [x] T23 [covers:R4.S2,R5.S2] 收敛第 2 轮：latest 查询失败时给出提示；tag 格式校验；html_url 的本机前缀精确匹配（CR19、CR20、CR21） — seam: `bash scripts/install.sh` + Gateway — verify: `cargo test -p alter-cli --test install_script && cargo test -p alterd --bin alterd update`
  - install.sh：
    - `curl … || true`，然后判断 tag 是否为空。
    - `--version` 与 latest 的 tag 都要匹配 `^v[0-9]+\.[0-9]+\.[0-9]+$`，否则 stderr `版本号应为 v<x.y.z>`，退出码 1。
  - 测试：
    - latest 的 API 返回 404 时，退出码 1，stderr 含「无法获取 latest 版本」，覆盖 R4.S2 的失败路径。
    - `--version ../evil` 时退出码 1，不发请求，覆盖 R5.S2 的相邻情况。
  - update.rs：本机前缀只接受 `http://127.0.0.1:` 或 `http://127.0.0.1/`。判断抽成 `fn allowed_url(&str) -> bool`，单元测试覆盖 `http://127.0.0.1.evil.com` 被拒。
- [x] T24 收敛整体第 1 轮：打包后的 app 的 e2e 关掉更新检查；改正样式注释与 design.md 里 release 工作流的描述（CR3、CR4、CR5） — seam: `apps/browser/e2e/packaged.spec.ts` + 文档 — verify: `bun run dist:dir && bun run e2e --grep "RC N2 R2" && bun run lint`
  - packaged.spec.ts：在临时 ALTER_HOME 里写 `config.toml`，内容 `update_check = false`。release 构建的 alterd 读它后不会联网。
  - styles.css：注释改为如实描述（提示条可点击，所以用醒目的配色）。flow.md 的 N1 → N2 交接里「沿用提示文案样式」这一句是 flow 的描述，run 不改 flow.md，在 log 里记 decided。
  - N2 design.md 的组件表：check-bundle 的用法写成 `<app> <版本号>`，说明它还检查包内程序的版本、DMG 的 SHA256 与挂载后里面的 app；gh release create 写明三个资产，以及 `--title`。
- [x] T25 收敛第 4 轮：改正提示条注释与 decided 的理由；packaged e2e 断言更新检查一次请求都没发（第 4 轮的两条） — seam: `apps/browser/e2e/packaged.spec.ts` — verify: `bun run e2e --grep "RC N2 R2"`
  - styles.css 注释与 log 的 decided：「点它在新标签打开 Release 页，所以用醒目的配色」。
  - packaged.spec.ts：用例里起一个本地 HTTP 服务器记录请求，设 `ALTER_RELEASE_API` 指向它、`ALTER_UPDATE_CHECK_DELAY_MS=0`，结束前断言请求数为 0。用变异「去掉 config.toml 那一行」证明它会失败。
