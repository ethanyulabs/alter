---
change: add-release-and-cookies
---

# 执行日志：打包发布、安装、升级与更新提示

<!-- 记录实际运行过什么。没有亲眼看到的结果不要写。
### T<n> <标题>
- Revision: <当前 revision>
- RED: `<命令>` → FAIL: <能看出预期失败原因的输出摘录>
- GREEN: `<命令>` → PASS
- REFACTOR: <改了什么> → PASS
每个覆盖的场景一对 RED/GREEN（RED 和 GREEN 两行都写上场景 ID）。一写就通过的测试，要先证明它会失败：
"- RED (mutation: <你故意改坏了什么>): `<命令>` → FAIL: …"，恢复代码后再记 GREEN。
（[baseline] 任务不写 RED/GREEN，改写 "- BASELINE: `<命令>` → PASS on unchanged code"。） -->

## 证据

### T1 协议：UpdateAvailable 与 ShellState.updateAvailable；配置项 update_check
- Revision: 1
- RED: `cargo test -p alter-core --test export_ts rc_n2_r15` → FAIL: 读取 …/UpdateAvailable.ts 失败：No such file or directory (R15.S1)
- GREEN: `cargo test -p alter-core --test export_ts rc_n2_r15` → PASS (1 test) (R15.S1)
- VERIFY: `bun run protocol:export` 生成 UpdateAvailable.ts，更新 ShellState.ts；`@alter/protocol` 入口补上再导出；`cargo clippy --all-targets -- -D warnings`、`cargo test`（全部 ok）、`bun run typecheck`、`bun test`（10 pass）、`bun run lint` 通过

### T2 version-check.sh --tag
- Revision: 1
- RED: `cargo test -p alter-cli --test release_scripts` → FAIL: rc_n2_r3_s1_tag_mismatch_fails — code=Some(0) stdout="版本号一致：0.1.0"（脚本还不认 --tag） (R3.S1)
- 中间一次失败：实现后在 cargo 测试环境里报 `line 45: cargo_version�: unbound variable`。原因是测试环境的 locale 下，bash 把紧跟在 `$cargo_version` 后面的全角括号字节当成了变量名的一部分（终端里手动运行正常）。改为 `${cargo_version}` 后修复
- GREEN: `cargo test -p alter-cli --test release_scripts` → PASS (2 tests) (R3.S1)
- RED (mutation: 只要给了 --tag 就失败): `cargo test -p alter-cli --test release_scripts` → FAIL: rc_n2_r3_s2_tag_matches (R3.S2)
- GREEN: 恢复后 `cargo test -p alter-cli --test release_scripts` → PASS (2 tests) (R3.S2)
- VERIFY: `shellcheck -S style scripts/version-check.sh` 无输出；`bun run version:check` → 版本号一致：0.1.0（不带 --tag 时行为不变）

### T3 打包：dist.sh、fetch-bun.sh、check-bundle.sh、electron-builder.yml、占位 SKILL.md
- Revision: 1
- RED: `bash scripts/check-bundle.sh <空的 alter.app> 0.1.0` → FAIL (exit 1)：缺少 bin/alterd、bin/alter、bin/bun、sdk/runtime.js、skills/alter-browser/SKILL.md、Info.plist，codesign 没有通过 (R1.S1)
- 第一次 `bun run dist` → exit 1：`.app` 已经打好并签了名，但出 DMG 时 dmgbuild 报 `hdiutil: couldn't unmount "disk4" - Resource busy`。`hdiutil info` 显示没有残留的挂载，原样重跑一次就成功了，判断为偶发问题（新挂载的卷被占用）。`dist.sh` 改为 DMG 这一步失败时重试一次
- GREEN: `bun run dist` → exit 0，产出 dist/alter-0.1.0-arm64.dmg（147M）、.sha256、.blockmap；`bash scripts/check-bundle.sh dist/mac-arm64/alter.app 0.1.0` → 「检查通过（0.1.0）」，其中挂载 DMG 后检查了 bin/alter --version、bin/bun --version 和 codesign (R1.S1)
- VERIFY: 缺陷检测：期望版本写成 0.2.0 → FAIL（bin/alter 与 CFBundleShortVersionString 不符）；`.sha256` 改成 0000 → FAIL（SHA256 不符）；恢复后检查通过。`shellcheck -S style scripts/*.sh` 无输出。electron-builder 26.15.3 的 `identity: "-"` 做了 ad-hoc 签名（日志 `signing … identityName=-`），不需要 afterSign 兜底。Bun 1.3.14 从 GitHub 下载，并按 SHASUMS256.txt 校验

### T5 exe::real_exe()：经符号链接运行时找同目录的 bun 与 runtime
- Revision: 1
- RED: `cargo test -p alter-cli --test nodejs rc_n2_r8` → FAIL: code=Some(1) stderr="找不到 bun：设置 ALTER_BUN_PATH，或把 bun 放进 PATH"（current_exe 返回的是链接本身 <Q>/alter） (R8.S1)
- GREEN: `cargo test -p alter-cli --test nodejs` → PASS (6 tests) (R8.S1)
- VERIFY: 用例给打包后的 runtime.js 开头加了 `[packaged-runtime]` 标记，并断言 stderr 里有它，用来排除退回开发版 runtime 也能通过的情况；`cargo clippy --all-targets -- -D warnings`、`cargo test`（全部 ok）通过

### T4 打包后的 app 使用包内的 alterd、bun 和 runtime
- Revision: 1
- 说明：这一行为由 T3（打包布局）、T5（real_exe）和 daemon.ts 已有的 resourcesPath 分支共同实现，测试写好时已经成立；按 tdd.md 在包上做文件级变异来证明
- VERIFY: 没有 dist/mac-arm64 时，`bun run e2e --grep "RC N2 R2"` 显示 1 skipped，原因已说明
- 中间一次失败：第一次运行时 `expect(out).toBe("true\n")` 失败，但差异里两边看起来一样。手动运行包内的 alter nodejs，`od -c` 看到的输出恰好是 `true\n`。原因是 Playwright 给子进程设了 FORCE_COLOR，Bun 打印布尔值时加了颜色转义码。用例的 env 里去掉 FORCE_COLOR 后通过
- RED (mutation: 移走包内的 bin/alterd，app 退回仓库的 target/debug/alterd): `bun run e2e --grep "RC N2 R2"` → FAIL: packaged.spec.ts:33 `alterd 进程` 不以 alter.app/Contents/Resources/bin/alterd 结尾 (R2.S1)
- GREEN: 恢复后 `bun run dist:dir && bun run e2e --grep "RC N2 R2"` → 1 passed (R2.S1)
- VERIFY: 第二个变异「移走包内的 bin/bun」→ FAIL（找不到 bun），恢复后 PASS；`bun run lint`、`bun run typecheck` 通过

### T6 alter skill install
- Revision: 1
- RED: `cargo test -p alter-cli --test skill` → FAIL: code=Some(2) stderr="error: unrecognized subcommand 'skill'" (R9.S1)
- GREEN: `cargo test -p alter-cli --test skill` → PASS (1 test) (R9.S1)
- RED (mutation: 去掉「没有目标时退出码 1」的检查): `cargo test -p alter-cli --test skill rc_n2_r9_s2` → FAIL: code=Some(0) stdout="" stderr=""（R9.S1 的最小实现已带上这个检查，所以测试一写就通过） (R9.S2)
- GREEN: 恢复后 `cargo test -p alter-cli --test skill` → PASS (3 tests) (R9.S2)
- RED: `cargo test -p alter-cli --test skill` → FAIL: rc_n2_r9_s3_target_replaces_old_install panicked at skill.rs:110（旧版本的 old.md 还在） (R9.S3)
- GREEN: `cargo test -p alter-cli --test skill` → PASS (3 tests) (R9.S3)
- VERIFY: widen.sh 第一次运行：fmt exit=1（tests/skill.rs 有一处 assert 没按 rustfmt 换行），其余各项 exit=0，提交 f8248fe 时却写成了「全部为 0」，记录有误。`cargo fmt` 之后重跑，fmt、clippy、cargo test、lint、typecheck、bun test、version:check、shellcheck 的 exit 都为 0

### T7 alter skill uninstall
- Revision: 1
- RED: `cargo test -p alter-cli --test skill rc_n2_r10` → FAIL: code=Some(2) stderr="error: unrecognized subcommand 'uninstall'" (R10.S1)
- GREEN: `cargo test -p alter-cli --test skill` → PASS (4 tests) (R10.S1)
- VERIFY: widen.sh 各项 exit 都为 0（fmt、clippy、cargo test、lint、typecheck、bun test、version:check、shellcheck）

### T8 scripts/install.sh 的安装路径
- Revision: 1
- RED: `cargo test -p alter-cli --test install_script` → FAIL (4 tests)：code=Some(127) stderr="bash: …/scripts/install.sh: No such file or directory" (R4.S1)
- RED: 同上，rc_n2_r4_s2_installs_latest FAIL（脚本不存在，没有 latest 解析） (R4.S2)
- RED: 同上，rc_n2_r4_s3_replaces_old_version FAIL（脚本不存在） (R4.S3)
- RED: 同上，rc_n2_r4_s4_path_hint FAIL（脚本不存在） (R4.S4)
- 中间两次失败：
  - 第一版退出码是 1。EXIT trap 里最后一条 `mount | grep` 没有匹配，它的状态成了脚本的退出码；而且 mount 显示的是 `/private/var/...`，与 `$mnt` 的路径对不上，所以 DMG 从没卸载过，留下了 5 个挂载（已经 `hdiutil detach -force` 清掉）。改为记录 `mounted` 状态，复制完显式 detach，cleanup 固定 `return 0`
  - R4.S1 的「已卸载」断言会被并行用例的挂载干扰，改为给脚本设用例专用的 TMPDIR，只检查这个目录下的挂载
- GREEN: `cargo test -p alter-cli --test install_script` → PASS (4 tests) (R4.S1)
- GREEN: 同一命令 → PASS (R4.S2)
- GREEN: 同一命令 → PASS (R4.S3)
- GREEN: 同一命令 → PASS (R4.S4)
- VERIFY: 变异「复制后不 detach」→ rc_n2_r4_s1 FAIL（DMG 应当已卸载），恢复后 4 passed，`hdiutil info` 没有残留挂载；`shellcheck -S style scripts/*.sh` 无输出；widen.sh 各项 exit 都为 0

### T9 install.sh 的校验失败、版本不存在、预演与平台检查
- Revision: 1
- RED: `cargo test -p alter-cli --test install_script -- rc_n2_r5 rc_n2_r6 rc_n2_t9` → FAIL: rc_n2_r5_s1_checksum_mismatch（还没有 SHA256 校验，安装照常完成） (R5.S1)
- GREEN: `cargo test -p alter-cli --test install_script` → PASS (10 tests) (R5.S1)
- RED (mutation: 下载失败时只打印「下载失败」，不带版本号): `cargo test -p alter-cli --test install_script rc_n2_r5_s2` → FAIL: stderr="curl: (22) … 404\n下载失败"，不含 v9.9.9（T8 已处理 404，测试一写就通过） (R5.S2)
- GREEN: 恢复后 `cargo test -p alter-cli --test install_script rc_n2_r5_s2` → PASS (R5.S2)
- RED: 同一组命令 → FAIL: rc_n2_r6_s1_dry_run（还不认 --dry-run 和 --with-skill，exit 2） (R6.S1)
- GREEN: `cargo test -p alter-cli --test install_script` → PASS (10 tests) (R6.S1)
- RED: 同一组命令 → FAIL: rc_n2_r6_s2_rejects_non_arm64（没有平台检查） (R6.S2)
- GREEN: `cargo test -p alter-cli --test install_script` → PASS (10 tests) (R6.S2)
- VERIFY: 边界用例 rc_n2_t9_dry_run_without_version（预演时不查 latest，也不发请求）与 rc_n2_t9_dry_run_running_app（默认 prefix 且假定 app 在运行时，打印退出、重开两步）修复前都 FAIL，修复后 PASS；widen.sh 各项 exit 都为 0；`hdiutil info` 没有残留挂载
- 未覆盖：真实安装时退出、重开正在运行的 alter（osascript）。测试都用非默认 prefix，不会碰正在运行的 app；这条路径只在预演里断言

### T10 install.sh 的 --with-skill
- Revision: 1
- RED: `cargo test -p alter-cli --test install_script -- rc_n2_r7 rc_n2_t10` → FAIL: rc_n2_r7_s1_with_skill_runs_skill_install（参数已解析，但安装后没有调用 alter skill install，skill-args 为空） (R7.S1)
- GREEN: `cargo test -p alter-cli --test install_script` → PASS (12 tests) (R7.S1)
- VERIFY: 边界用例 rc_n2_t10_skill_failure_keeps_install（skill install 失败时安装仍以 0 结束并提示）修复前 FAIL，修复后 PASS；widen.sh 各项 exit 都为 0

### T11 alter upgrade 的版本检查与前提条件
- Revision: 1
- RED: `cargo test -p alter-cli --test upgrade` → FAIL (4 tests)：`unrecognized subcommand 'upgrade'` (R11.S1)
- GREEN: `cargo test -p alter-cli --test upgrade` → PASS (4 tests) (R11.S1)
- RED: 同上，rc_n2_r11_s2_up_to_date FAIL（没有 upgrade 子命令） (R11.S2)
- GREEN: 同上 → PASS (R11.S2)
- RED: 同上，rc_n2_r11_s4_unreachable_api FAIL (R11.S4)
- GREEN: 同上 → PASS (R11.S4)
- RED: 同上，rc_n2_r11_s5_not_in_bundle FAIL (R11.S5)
- GREEN: 同上 → PASS (R11.S5)
- VERIFY: 单元测试 `upgrade::tests::compares_versions_numerically`（0.10.0 > 0.9.9；两段的、带 -beta 的版本号不算更新）通过；`cargo test -p alter-cli --release --test perf -- --ignored`，「SC1: alter --version 100 次 中位数 5.37 ms，p95 9.16 ms」（阈值 20 ms）；widen.sh 各项 exit 都为 0

### T12 alter upgrade 调用 install.sh 并刷新 skill
- Revision: 1
- RED: `cargo test -p alter-cli --test upgrade -- rc_n2_r11_s3 rc_n2_t12` → FAIL: rc_n2_r11_s3 — code=Some(1) stderr="升级失败：还不能调用 install.sh"（T11 留下的占位分支） (R11.S3)
- GREEN: `cargo test -p alter-cli --test upgrade` → PASS (6 tests) (R11.S3)
- VERIFY: 边界用例 rc_n2_t12_install_sh_failure：占位实现下就能通过，所以做了变异「install.sh 非 0 也当成功」→ FAIL（code=Some(0)，升级后照样刷新了 skill），恢复后 PASS；widen.sh 各项 exit 都为 0

### T13 shellcheck 门禁与 release 工作流
- Revision: 1
- VERIFY: `bun run lint:sh`（`shellcheck -S style scripts/*.sh`）exit 0；加一个临时的问题脚本 `cd $1` 后 exit 1（SC2164、SC2086），删掉后 exit 0；`bun run ci:check` exit 0（末尾已经是 lint:sh）
- VERIFY: `ruby -ryaml -e 'YAML.load_file(…release.yml); YAML.load_file(…ci.yml)'` → yaml-ok。release 工作流本身只能靠推 tag 验证，对应父级 SC6（monitored after release）；它调用的 version-check --tag、dist、check-bundle 都已在 T2、T3 本地跑过

### T14 alterd 检查更新并推送 app.updateAvailable
- Revision: 1
- RED: `cargo test -p alterd --test gateway rc_n2_r12` → FAIL: rc_n2_r12_s1 panicked at gateway.rs:1237（3 秒内没有收到 app.updateAvailable） (R12.S1)
- GREEN: `cargo test -p alterd --test gateway rc_n2_r12` → PASS (3 tests) (R12.S1)
- RED: 同上 → FAIL: rc_n2_r12_s2 panicked at gateway.rs:1276「前提：确实检查过」（还没有任何检查） (R12.S2)
- GREEN: 同上 → PASS (R12.S2)
- RED: 同上 → FAIL: rc_n2_r12_s3 panicked at gateway.rs:1309（stderr 里没有「检查更新失败」） (R12.S3)
- GREEN: 同上 → PASS (R12.S3)
- VERIFY: 变异「新连接不补推」→ rc_n2_r12_s1 FAIL（检查往往在客户端连上之前就完成了，所以第一个客户端也依赖补推）；变异「忽略 update_check = false」→ rc_n2_r12_s2 FAIL。恢复后都 PASS。单元测试 `update::tests` 通过；widen.sh 各项 exit 都为 0；`alter --version` 冷启动中位数 4.47 ms（父级 SC1）

### T15 SDK 打印更新提示
- Revision: 1
- RED: `bun test packages/sdk/tests/update-notice.test.ts` → FAIL: stderr 里没有 [alter:notice] 行（Expected 1 行，Received 0 行） (R13.S1)
- GREEN: `bun test packages/sdk/tests/update-notice.test.ts` → 1 pass (R13.S1)
- VERIFY: 变异「去掉只打印一次的判断」→ FAIL（打印了两行），恢复后 PASS；widen.sh 各项 exit 都为 0

### T16 侧边栏的更新提示条
- Revision: 1
- RED: `bun run e2e --grep "RC N2 R14"` → FAIL: R14.S1：5000 ms 内条件没有同时成立；最后一次：还没有提示条 (R14.S1)
- GREEN: `bun run e2e --grep "RC N2 R14"` → ✓ (1.5s) (R14.S1)
- VERIFY: Main 的 shell.stateChanged 处理把 `updateAvailable` 带进 Renderer 的状态；完整 `bun run e2e` → 108 passed (2.1m)；widen.sh 各项 exit 都为 0

### T17 节点门禁
- Revision: 1
- VERIFY: `bun run dist:dir` exit 0（在当前代码上重新打包）；`bash scripts/check-bundle.sh dist/mac-arm64/alter.app 0.1.0` → 检查通过
- VERIFY: `bun run ci:check` exit 0（cargo 222 passed / 0 failed，bun test 11 pass，lint、typecheck、version:check、lint:sh 都通过）
- VERIFY: `bun run e2e --grep "RC N2"` → 2 passed（R2.S1 打包后的 app、R14.S1 更新提示条）；`hdiutil info` 没有残留挂载
- VERIFY: 覆盖率用 rustc `-C instrument-coverage` 加 `xcrun llvm-cov`，统计 `git diff 28b5273..HEAD` 的新增可执行行：
  - alterd 112/116 = 96.6%：gateway/mod.rs 24/25、tabs/mod.rs 18/19、update.rs 69/71。
  - alter-cli 223/267 = 83.5%：skill.rs 69/86、upgrade.rs 139/166，未覆盖的是复制、写库、启动 bash 等失败分支。
  - 合计 335/383 = 87.5%（≥ 80%）。
  - install.sh 等 shell 脚本没有行覆盖率工具，以 R4–R7 的场景测试代替。
  - SDK 的改动由 update-notice.test.ts 覆盖。
- 测量过程中发现的问题：测试辅助的 `Daemon` 在 Drop 时用 SIGKILL 结束 alterd，进程没机会写出覆盖率数据，第一次测量 alterd 只有 53.4%，偏低。现已改为先 SIGTERM，2 秒内没退出再 kill（两个 crate 的 tests/common 都改了）；这样也更接近真实的退出路径。N1 当时的 81.5% 同样偏低，这只影响测量，不影响已经达标的结论

### T18 install.sh 的失败回退、运行中 app 的判断与退出、detach 失败、相对路径
- Revision: 1
- RED: `cargo test -p alter-cli --test install_script -- rc_n2_t18 rc_n2_t9_dry_run_running` → FAIL (5 tests)：rc_n2_t18_rolls_back_when_link_fails 断言失败，链接失败时旧的 0.2.0 已被删掉；另外 4 个是 quits_and_reopens、refuses_when_app_does_not_quit、relative_paths_with_spaces、rc_n2_t9_dry_run_running_app (R4.S3)
- RED: 同上 → FAIL：rc_n2_t9_dry_run_running_app，没有「[dry-run] 退出正在运行的 alter」 (R6.S1)
- 中间两次失败：
  - 两个「运行中」的用例里，A 里根本没装 app，「正在运行的就是 A/alter.app」不可能成立。改为先装 0.2.0、运行时再装 0.3.0。
  - 假 app 是测试进程的子进程，TERM 之后成了没人回收的僵尸进程，`kill -0` 仍然成功，安装因此误判 10 秒没退出。改为经中间的 bash 放到后台，交给 launchd 回收，和真实 app 一样。
- GREEN: `cargo test -p alter-cli --test install_script` → PASS (17 tests) (R4.S3)
- GREEN: 同一命令 → PASS (R6.S1)
- VERIFY: 变异「cleanup 不恢复 .old」→ rc_n2_t18_rolls_back FAIL；变异「不比较包路径，只要在运行就退出」→ rc_n2_t18_leaves_other_bundle_alone FAIL；恢复后都 PASS。`hdiutil info` 没有残留挂载；shellcheck 无输出；widen.sh 各项 exit 都为 0
- 未覆盖：detach 普通与强制方式都失败的分支（测试里没法让 hdiutil 稳定失败）

### T19 upgrade：只升不降、--json 必须带 --check、--version 校验、严格的包路径、独占的临时目录
- Revision: 1
- RED: `cargo test -p alter-cli --test upgrade rc_n2_t19` → FAIL: rc_n2_t19_json_requires_check（`--json` 单独用时会去升级，退出码不是 2） (R11.S1)
- GREEN: `cargo test -p alter-cli --test upgrade` → PASS (10 tests) (R11.S1)
- RED: 同上 → FAIL: rc_n2_t19_older_latest_is_up_to_date（latest 更旧时会去升级，不在包内所以退出码 1） (R11.S2)
- GREEN: 同上 → PASS (R11.S2)
- RED: 同上 → FAIL: rc_n2_t19_strict_bundle_layout（`x.app/Other/Resources/bin/alter` 被当成安装包，去请求了 install.sh） (R11.S5)
- GREEN: 同上 → PASS (R11.S5)
- VERIFY: rc_n2_t19_rejects_bad_version（foo、0.2.0、v0.2、vv0.2.0、空串都以退出码 1 拒绝，不访问网络）修复前 FAIL、修复后 PASS；临时目录改为 0700 加 create_new（CR12），这一条没有测试；N2 design.md 的接口契约已同步；widen.sh 各项 exit 都为 0

### T20 skill install 先复制到暂存目录再替换；拒绝源与目标相同
- Revision: 1
- RED: `cargo test -p alter-cli --test skill rc_n2_t20` → FAIL: rc_n2_t20_copy_failure_keeps_old_install（复制失败时旧的 SKILL.md 已经被删掉） (R9.S3)
- GREEN: `cargo test -p alter-cli --test skill` → PASS (6 tests) (R9.S3)
- VERIFY: rc_n2_t20_rejects_same_source_and_target 修复前 FAIL（源被先删掉），修复后 PASS；N2 design.md 的接口契约已同步；widen.sh 各项 exit 都为 0

### T21 补推与广播同一把锁、R12.S1 证明广播、curl 不读 .curlrc、html_url 校验、按版本去重
- Revision: 1
- RED (mutation: announce_update 不向已连接的客户端广播): 加强后的 `cargo test -p alterd --test gateway rc_n2_r12_s1`（先扣住 latest，客户端连上后再放行）→ FAIL at gateway.rs:1244，3 秒内收不到通知；加强之前，这个变异下用例仍能通过（CR8） (R12.S1)
- GREEN: 恢复后 `cargo test -p alterd --test gateway -- rc_n2_r12 rc_n2_t21` → PASS (5 tests) (R12.S1)
- VERIFY: rc_n2_t21_ignores_curlrc（.curlrc 里的 X-Leak 头不再出现，请求带 User-Agent alter/<version>）与 rc_n2_t21_rejects_bad_html_url（javascript: 地址不推送，记「检查更新失败」）修复前都 FAIL、修复后 PASS。补推与广播改在同一把锁下，「同一连接恰好一次」由 R12.S1 的「1 秒内没有第二条」断言钉住；交错的竞态本身无法稳定复现，由锁的顺序保证。N2 design.md 已同步；widen.sh 各项 exit 都为 0

### T22 release 工作流的 action 钉到 commit SHA
- Revision: 1
- VERIFY: `gh api repos/<action>/commits/<ref> --jq .sha` 查到当前的 SHA：actions/checkout@v4 → 11d5960a…、dtolnay/rust-toolchain@stable → b725eda5…、Swatinem/rust-cache@v2 → d1527c8f…、oven-sh/setup-bun@v2 → 0c5077e5…，写成 `@<sha> # <ref>`，checkout 加 `persist-credentials: false`；`ruby -ryaml -e 'YAML.load_file(".github/workflows/release.yml")'` → yaml-ok

### T23 收敛第 2 轮：latest 查询失败的提示、tag 格式校验、html_url 本机前缀精确匹配
- Revision: 1
- RED: `cargo test -p alter-cli --test install_script rc_n2_t23` → FAIL: rc_n2_t23_latest_query_failure_is_reported（API 返回 404 时，脚本以 curl 的退出码在赋值处直接结束，没有「无法获取 latest 版本」） (R4.S2)
- GREEN: `cargo test -p alter-cli --test install_script` → PASS (19 tests) (R4.S2)
- RED: 同上 → FAIL: rc_n2_t23_rejects_bad_version（`../evil` 等被接受，并发出了下载请求） (R5.S2)
- GREEN: 同上 → PASS (R5.S2)
- VERIFY: `allowed_url` 先原样抽出旧判断，单元测试 accepts_https_and_exact_loopback_only FAIL（`http://127.0.0.1.evil.com` 被接受），改为只认 `http://127.0.0.1:` 和 `http://127.0.0.1/` 后 PASS；shellcheck 无输出；`hdiutil info` 没有残留挂载；widen.sh 各项 exit 都为 0

### T24 收敛整体第 1 轮：打包 app 的 e2e 关掉更新检查；改正样式注释与 design.md 的 release 工作流描述
- Revision: 1
- VERIFY: 先证明问题存在、修法有效。用打包出的 release 版 alterd，`ALTER_RELEASE_API` 指向本地一个记录请求的服务器，`ALTER_UPDATE_CHECK_DELAY_MS=0`：
  - 临时 home 里没有 config.toml 时，记录到 1 次 `/releases/latest` 请求；用例原来就是这种情况，不设 `ALTER_RELEASE_API` 时去的就是 GitHub。
  - 写入 `update_check = false` 后，0 次请求。
- VERIFY: packaged.spec.ts 在临时 ALTER_HOME 写入 `update_check = false`；`bun run e2e --grep "RC N2 R2"` → 1 passed。
- VERIFY: styles.css 的注释改为如实描述醒目配色的原因；N2 design.md 组件表按实现改正 check-bundle 的用法与检查项，以及 gh release create 的资产与 `--title`。
- VERIFY: widen.sh 各项 exit 都为 0

### T25 收敛第 4 轮：改正提示条注释的理由；packaged e2e 断言没有检查更新
- Revision: 1
- VERIFY: packaged.spec.ts 在用例里起一个本地服务器记录请求，`ALTER_RELEASE_API` 指向它，`ALTER_UPDATE_CHECK_DELAY_MS=0`，结束前断言请求列表为空。变异「去掉写 config.toml 的那一行」：`bun run e2e --grep "RC N2 R2"` → ✘，Received 为 `["/releases/latest"]`；恢复后 → 1 passed
- VERIFY: styles.css 的注释与本 log 第 4 轮前那条 decided 改为「点它在新标签打开 Release 页」，与 UpdateBanner.tsx:25、R14.S1 一致；widen.sh 各项 exit 都为 0

## 决策与偏差

- decided: 起点状态 — N2 从 HEAD 28b5273 开始（N1 done），工作区干净
- decided: 快照里的 `update_available` 先固定为 None，T14 再接上检查更新的结果 — from: T1 只交付类型

- decided: 本需求的 bash 脚本里，紧挨中文（多字节字符）的变量一律写成 `${var}` — from: T2 在测试环境中实际遇到的 locale 问题

- decided: electron-builder 用 `electronDist: node_modules/electron/dist`（不再下载 Electron），`files` 排除 node_modules — from: 主进程与 preload 的构建产物在运行时只 require electron 和 node 内置模块（grep out/ 确认），renderer 由 vite 打包
- decided: DMG 这一步失败时重试一次 — from: 第一次打包遇到 hdiutil `Resource busy`，原样重跑即成功；CI 上同样可能偶发

- decided: 更正 T4 的证据 — 提交 5ad1eb0 那条 VERIFY 写了「`bun run lint` 通过」，但那次实际输出是 `error: script "lint" exited with code 1`：eslint 扫到了 `scripts/dist.sh` 生成的 `build/sdk/runtime.js`（`process` / `WebSocket` 未定义）。现已在 eslint 配置的 ignores 里加上 `build/**`，`bun run lint` 通过；这次修正与 T3 引入的 `build/` 目录属于同一个问题 — from: 重新运行 lint 的实际输出

- decided: 测试里的假 install.sh 替换包内的 alter 时先 `rm` 再写新文件 — from: 在 macOS 上原地改写正在运行的可执行文件，会让正在执行的进程被系统杀掉；真实的 install.sh 也是用 `.new` 改名替换，同样换了新 inode

- decided: alterd 调 curl 用 `std::process::Command` 放在 `spawn_blocking` 里，不开 tokio 的 `process` 特性 — from: 宪章「简单优先」，workspace 的 tokio 特性里没有 process，打开它意义不大
- decided: 版本号解析在 alterd 的 update.rs 与 alter-cli 的 upgrade.rs 里各写一份（十几行），各自配单元测试 — from: tasks.md T14 备注

- design: SDK 的 Client 新增被动监听 `onNotification`（不建连、不阻止空闲断开），`createAlter()` 的返回值也加了 `onNotification`，供 runtime.ts 使用。原因是现有的 `subscribe` 会主动建连并一直保持连接，脚本进程会因此退不出。N2 design.md「组件」的 SDK 一行已同步

- design: install.sh 不再用 osascript 判断、退出 app，改为 lsappinfo + kill -TERM + open，并且只处理「正在运行的就是要替换的那个包」的情况；删掉 `ALTER_INSTALL_ASSUME_RUNNING`（CR2、CR3、CR11）。替换改为 .old 改名加 trap 恢复（CR1）。N2 design.md 的接口契约第 6、7 步、决策表、时序图，以及父级 design 的环境变量表、父级 requirement 的假设已同步

- decided: `-q`（不读 .curlrc）只加在 alterd 后台的检查更新上；用户手动运行的 `alter upgrade` 与 install.sh 保留用户的 curl 配置 — from: 父级「只发 User-Agent」的 NFR 是针对后台联网的，用户自己的代理设置在手动下载时往往是必需的

- decided: 第 2 轮由会话自己评审（self-review）— codex 这一轮的最终答案只是一段草稿（「New findings 7… Let's craft.」），没有列出任何发现，按规则算作没有作答，不重试；claude:opus-5 本会话已记为不可用。两个评审者都没有可用结果时，由会话自己评审这一轮 — from: principles.md「Reviewers」：当列表里所有评审者都没有作答时，本轮由会话自己评审

- decided: 更新提示条用醒目配色，不沿用导入向导提示文案的灰色字：点提示条会在新标签打开 Release 页（R14.S1），灰色字看不出能点。flow.md 里 N1 → N2 交接的「提示文案样式」不再适用，记在这里，存档时以实现为准 — from: 整体评审第 1 轮 CR4

## 代码评审
<!-- 由评审步骤填写，每次评审一轮：
### Round <n>
- Revision: <当前 revision>
- Scope: <repo> `git diff <base>...HEAD`（含未提交的改动）
- Reviewers: <agent> (<model>): <n> findings; <agent> (<model>): <n> findings | unavailable (<原因>)（配置的 reviewer 各返回了什么；没配置时写 `<agent> (<model>, session)` 或 `(<model>, self-review)`）
- Tasks: T1, T2, …（这一轮读过代码的任务；收敛任务和重新打开的任务需要新的一轮）
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
最后恰好一行 "Review: PASS" 或 "Review: CHANGES REQUESTED"。语法见 references/formats.md -->

### Round 1
- Revision: 1
- Scope: alter `git diff 28b5273...HEAD`（N2 的全部改动，不含 Cargo.lock 与 .dev-flow/；没有未提交的改动），基准是 N1 完成时的提交
- Reviewers: codex (gpt-5.6-sol): 18 findings; claude (opus-5): unavailable (unknown model，本会话不再重试)
- Tasks: T1-T17
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR1 | major | correctness | scripts/install.sh:184 | 先删掉旧 alter.app，再把 .new 改名：改名或之后的链接失败时，旧安装已经没了，违反「install.sh 失败时原安装不受影响」（R4.S3、R11.S3） | fixed in R3 |
| CR2 | major | correctness | scripts/install.sh:76 | 是否退出正在运行的 app，取决于有没有传 --prefix；alter upgrade 总会传，所以升级时不会退出 app，会直接替换正在运行的包（R11.S3） | fixed in R3 |
| CR3 | major | correctness | scripts/install.sh:176 | 等 10 秒后不确认 app 已退出就继续替换；app 已退出而之后安装失败时，也不会把它重新打开 | fixed in R3 |
| CR4 | major | correctness | crates/alter-cli/src/upgrade.rs:37 | 不带 --version 时只判断「等于当前版本」：latest 回退成更旧的版本时，alter upgrade 会悄悄装上旧版本（R11.S2） | fixed in R3 |
| CR5 | major | spec | crates/alter-cli/src/main.rs:62 | `--json` 不要求 `--check`：`alter upgrade --json` 会忽略 JSON 选项并真的升级（R11.S1） | fixed in R3 |
| CR6 | minor | correctness | crates/alter-cli/src/skill.rs:27 | 覆盖 skill 时先删旧目录再复制：复制失败会丢掉旧版本、留下半成品；源与目标相同时会先删掉源（R9.S3） | fixed in R3 |
| CR7 | major | correctness | crates/alterd/src/gateway/mod.rs:112 | 新连接的补推与更新广播分别拿 update、listeners 两把锁：特定交错下客户端会漏收，或收到两次（R12.S1） | fixed in R3 |
| CR8 | major | tests | crates/alterd/tests/gateway.rs:1227 | 检查在 0 ms 时就开始，第一个客户端通常走的是「连接时补推」：删掉向已连接客户端广播的实现，R12.S1 照样通过 | fixed in R3 |
| CR9 | major | security | crates/alterd/src/update.rs:60 | curl 默认读取用户的 `.curlrc`，其中的 header、cookie、认证会随后台请求发给 GitHub，违反「只带 User-Agent」（父级安全 NFR） | fixed in R3 |
| CR10 | major | security | .github/workflows/release.yml:16 | 有 contents: write 权限的发布 job 使用 @v4、@stable 等可变引用，上游被篡改时能改写 Release 资产 | fixed in R3 |
| CR11 | major | spec | scripts/install.sh:82 | `ALTER_INSTALL_ASSUME_RUNNING` 按设计只用于 --dry-run，真实安装也在读它；退出与重开的真实路径没有测试 | fixed in R3 |
| CR12 | minor | security | crates/alter-cli/src/upgrade.rs:84 | 临时目录只用 PID 命名且 create_dir_all 复用已有目录，写 install.sh 会跟随预置的符号链接 | fixed in R3 |
| CR13 | minor | correctness | scripts/install.sh:138 | 普通与强制 detach 都失败时用「或 true」吞掉错误并清除 mounted 标记，脚本报成功但 DMG 仍挂着 | fixed in R3 |
| CR14 | minor | correctness | scripts/install.sh:193 | 相对路径的 --prefix 会让 bin-dir 里的链接指向错误位置 | fixed in R3 |
| CR15 | minor | spec | crates/alter-cli/src/upgrade.rs:13 | `--version` 没按 `v<x.y.z>` 校验，空值、foo 等都会被接受 | fixed in R3 |
| CR16 | minor | spec | crates/alter-cli/src/upgrade.rs:193 | bundle_root 不检查 `Contents` 与文件名，形如 `x.app/any/Resources/bin/renamed` 也被当成安装包（R11.S5） | fixed in R3 |
| CR17 | minor | security | crates/alterd/src/update.rs:81 | html_url 缺失时变成空字符串仍然推送；任意 scheme 都会进到侧边栏 | fixed in R3 |
| CR18 | minor | spec | crates/alterd/src/gateway/mod.rs:114 | 去重比较整个 UpdateAvailable 且只记最后一个：同版本换了 URL 会再推，A→B→A 时 A 推两次 | fixed in R3 |

Review: CHANGES REQUESTED

### Round 2
- Revision: 1
- Scope: alter `git diff 508c01d...HEAD`（T18–T22 的收敛修复，不含 .dev-flow/；没有未提交的改动）
- Reviewers: codex (gpt-5.6-sol): unavailable (no usable answer：最终答案只是一段草稿，没有列出任何发现); claude (opus-5): unavailable (unknown model，本会话不再重试); claude (claude-opus-5-5, self-review): 3 findings
- Tasks: T18, T19, T20, T21, T22
- 复核第 1 轮：CR1–CR18 都已修复。
  - CR1：.old 改名加 trap 恢复。
  - CR2、CR3、CR11：lsappinfo 判断包路径，kill -TERM，10 秒不退出就不做任何改动，失败时也会重开。
  - CR4、CR5、CR12、CR15、CR16：只升不降、`--json` 必须带 `--check`、独占临时目录、版本号校验、严格的包路径。
  - CR6：暂存目录。
  - CR7、CR18：同一把锁，按版本去重。
  - CR8：先扣住 latest 再连客户端，「去掉广播」的变异能被测出。
  - CR9：curl 加 `-q`。
  - CR10：action 钉到 SHA。
  - CR13：detach 失败时报错。
  - CR14：路径转为绝对路径。
  - CR17：html_url 校验，但前缀判断有漏洞，见 CR21。
  - 交错竞态本身无法稳定复现，由锁的顺序保证。
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR19 | minor | correctness | scripts/install.sh:136 | `set -euo pipefail` 下 `tag="$(curl … 管道 sed …)"` 一失败，脚本就在赋值处以 curl 的退出码直接结束，「无法获取 latest 版本」这句提示永远打不出来 | fixed in R3 |
| CR20 | minor | security | scripts/install.sh:142 | `--version` 与 latest 的 tag 没有校验格式，直接拼进下载地址与临时文件名（`alter-${version}-arm64.dmg`） | fixed in R3 |
| CR21 | minor | security | crates/alterd/src/update.rs:81 | html_url 只检查 `starts_with("http://127.0.0.1")`，`http://127.0.0.1.evil.com/…` 也能通过 | fixed in R3 |

Review: CHANGES REQUESTED

### Round 3
- Revision: 1
- Scope: alter `git diff 44cc237...HEAD`（T23 的修复，不含 .dev-flow/；没有未提交的改动）
- Reviewers: codex (gpt-5.6-sol): unavailable (第 2 轮没有作答，本会话不再重试); claude (opus-5): unavailable (unknown model); claude (claude-opus-5-5, self-review): no findings
- Tasks: T23
- 复核第 2 轮：
  - CR19：curl 失败时由 `|| true` 兜住，后面的空值判断照常给出提示，由 rc_n2_t23_latest_query_failure_is_reported 钉住。
  - CR20：`--version` 与 latest 的 tag 都按 `^v[0-9]+\.[0-9]+\.[0-9]+$` 校验，`../evil` 等在发请求之前就被拒绝。
  - CR21：allowed_url 只认 `http://127.0.0.1:` 与 `http://127.0.0.1/`，单元测试覆盖了 `.evil.com` 这种绕过前缀的写法。
  - 第 1 轮的 CR1–CR18 已在第 2 轮复核为修复。
Findings: none.

Review: PASS

### Round 4
- Revision: 1
- Scope: alter `git show 017c7fe`（T24，整体评审第 1 轮的 CR3–CR5）
- Reviewers: codex (gpt-5.6-sol): unavailable (本会话不再重试); claude (opus-5): unavailable (unknown model); claude (claude-opus-5-5, session): 2 findings（委托会话内一个全新的子代理；CR3、CR5 已修复，核对了 config.rs、lifecycle.rs:131、update.rs:18 确认 update_check = false 会在读 ALTER_RELEASE_API 之前返回）
- Tasks: T24
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR22 | minor | maintainability | apps/browser/src/renderer/src/styles.css:535 | 新注释与 log 的 decided 都说提示条「整条可点击（复制升级命令）」；实际点击是在新标签打开 Release 页（UpdateBanner.tsx:25，R14.S1），× 是单独的按钮 | fixed in R5 |
| CR23 | nit | tests | apps/browser/e2e/packaged.spec.ts:22 | 只写入 update_check = false，没有断言确实没有请求；以后键名或判断顺序变了，e2e 会悄悄访问 GitHub | fixed in R5 |

Review: CHANGES REQUESTED

### Round 5
- Revision: 1
- Scope: alter `git show ea48bed`（T25）
- Reviewers: codex (gpt-5.6-sol): unavailable (本会话不再重试); claude (opus-5): unavailable (unknown model); claude (claude-opus-5-5, session): 0 findings（委托会话内一个全新的子代理。它核对了 e2e 断言的时序：lifecycle.rs:131 在写健康文件之前就启动检查，延迟为 0 时请求比 alterd.json 还早发出，waitUntil 期间事件循环在跑；daemon.ts 把环境变量传给了包内的 alterd）
- Tasks: T25

Review: PASS

## 验证报告
<!-- 每次验证追加一份新报告，放在最近一轮评审之后。以 "- Revision: <当前 revision>" 和 "- Review: Round <最近一轮>" 开头。由验证步骤填写：本仓库的测试结果，每个场景一行 "| R<n>.S<m> | <path::test> | PASS |"，成功标准对照表，收敛缺口，fix 类型还要写 "Fix: verified | partial | failed"，最后恰好一行 "Overall: READY" 或 "Overall: NOT READY"。 -->

### 节点报告 1
- Revision: 1
- Review: Round 3
- 运行环境：本机 macOS（Apple Silicon），分支 feature/add-release-and-cookies，HEAD 057b872，工作区干净

| Repo | Command | Result |
|---|---|---|
| alter | `bun run dist` | PASS（exit 0，dist/alter-0.1.0-arm64.dmg 与 .sha256） |
| alter | `bash scripts/check-bundle.sh dist/mac-arm64/alter.app 0.1.0` | PASS（检查通过，含挂载 DMG） |
| alter | `bun run ci:check` | PASS（exit 0；cargo 238 passed / 0 failed，bun test 11 pass，lint、typecheck、version:check、lint:sh） |
| alter | `bun run e2e` | PASS（108 passed (2.1m)，含打包后的 app） |
| alter | `cargo test -p alter-cli --release --test perf -- --ignored` | PASS（中位数 4.03 ms < 20 ms，父级 SC1） |
| alter | `cargo test -p alterd --release --test gateway_perf -- --ignored` | PASS（page.click p95 0.422 ms < 5 ms，父级 SC2） |
| alter | 改动行覆盖率（rustc `-C instrument-coverage` + `xcrun llvm-cov`，`git diff 28b5273..HEAD`） | PASS（alterd 135/139 = 97.1%，alter-cli 278/329 = 84.5%，合计 413/468 = 88.2%） |

| Scenario | Test | Result |
|---|---|---|
| R1.S1 | scripts/check-bundle.sh（`bun run dist` 后 `bash scripts/check-bundle.sh dist/mac-arm64/alter.app 0.1.0`，含挂载 DMG 检查） | PASS |
| R2.S1 | apps/browser/e2e/packaged.spec.ts::RC N2 R2.S1 打包后的 app 拉起包内的 alterd | PASS |
| R3.S1 | crates/alter-cli/tests/release_scripts.rs::rc_n2_r3_s1_tag_mismatch_fails | PASS |
| R3.S2 | crates/alter-cli/tests/release_scripts.rs::rc_n2_r3_s2_tag_matches | PASS |
| R4.S1 | crates/alter-cli/tests/install_script.rs::rc_n2_r4_s1_installs_given_version | PASS |
| R4.S2 | crates/alter-cli/tests/install_script.rs::rc_n2_r4_s2_installs_latest | PASS |
| R4.S3 | crates/alter-cli/tests/install_script.rs::rc_n2_r4_s3_replaces_old_version | PASS |
| R4.S4 | crates/alter-cli/tests/install_script.rs::rc_n2_r4_s4_path_hint | PASS |
| R5.S1 | crates/alter-cli/tests/install_script.rs::rc_n2_r5_s1_checksum_mismatch | PASS |
| R5.S2 | crates/alter-cli/tests/install_script.rs::rc_n2_r5_s2_version_not_found | PASS |
| R6.S1 | crates/alter-cli/tests/install_script.rs::rc_n2_r6_s1_dry_run | PASS |
| R6.S2 | crates/alter-cli/tests/install_script.rs::rc_n2_r6_s2_rejects_non_arm64 | PASS |
| R7.S1 | crates/alter-cli/tests/install_script.rs::rc_n2_r7_s1_with_skill_runs_skill_install | PASS |
| R8.S1 | crates/alter-cli/tests/nodejs.rs::rc_n2_r8_s1_runs_through_symlink | PASS |
| R9.S1 | crates/alter-cli/tests/skill.rs::rc_n2_r9_s1_installs_into_existing_agent_dirs | PASS |
| R9.S2 | crates/alter-cli/tests/skill.rs::rc_n2_r9_s2_no_agent_dirs | PASS |
| R9.S3 | crates/alter-cli/tests/skill.rs::rc_n2_r9_s3_target_replaces_old_install | PASS |
| R10.S1 | crates/alter-cli/tests/skill.rs::rc_n2_r10_s1_uninstall_only_marked | PASS |
| R11.S1 | crates/alter-cli/tests/upgrade.rs::rc_n2_r11_s1_check_finds_newer | PASS |
| R11.S2 | crates/alter-cli/tests/upgrade.rs::rc_n2_r11_s2_up_to_date | PASS |
| R11.S3 | crates/alter-cli/tests/upgrade.rs::rc_n2_r11_s3_runs_install_sh_and_refreshes_skill | PASS |
| R11.S4 | crates/alter-cli/tests/upgrade.rs::rc_n2_r11_s4_unreachable_api | PASS |
| R11.S5 | crates/alter-cli/tests/upgrade.rs::rc_n2_r11_s5_not_in_bundle | PASS |
| R12.S1 | crates/alterd/tests/gateway.rs::rc_n2_r12_s1_pushes_update_notice | PASS |
| R12.S2 | crates/alterd/tests/gateway.rs::rc_n2_r12_s2_no_notice_when_current_or_disabled | PASS |
| R12.S3 | crates/alterd/tests/gateway.rs::rc_n2_r12_s3_check_failure_is_logged | PASS |
| R13.S1 | packages/sdk/tests/update-notice.test.ts::RC N2 R13.S1 收到两次通知只打印一次 | PASS |
| R14.S1 | apps/browser/e2e/update.spec.ts::RC N2 R14.S1 显示更新提示条 | PASS |
| R15.S1 | crates/alter-core/tests/export_ts.rs::rc_n2_r15_s1_export_update_available | PASS |

收敛检查：
- 对照 N2 requirement.md 与 design.md 逐条核对，没有 missing / partial / contradicts。
- unrequested：评审收敛时加了一些收紧已有行为的规则，都写进了 design.md 的「接口契约」并由测试钉住，没有新增对外能力：
  - install.sh 的回退与运行中 app 的判断；
  - upgrade 只升不降与参数校验；
  - skill 的暂存目录；
  - 更新提示按版本去重与地址校验。
- 宪章：
  - 只有 alterd 发起后台联网，用的是系统 curl，带 `-q`。
  - 不新增运行时依赖。
  - install.sh 过了 shellcheck，`--dry-run` 有测试断言。
  - 没有改动 SQLite 表结构。
- 测量说明：覆盖率导出只用 alter 本身。多加 `-object` 时，测试版二进制里的那份源文件也会被算进分母，得到 74.5%，偏低。
- 父级 SC6（首次发布）是发布后监控项，不在节点验证的范围内。

Gaps: none.

Overall: READY

### 节点报告 2
- Revision: 1
- Review: Round 5
- 运行环境：本机 macOS（Apple Silicon），分支 feature/add-release-and-cookies，HEAD 63f7111；`bun run dist:dir` 用当前代码重新打了包

| Repo | Command | Result |
|---|---|---|
| alter | `bun run ci:check` | PASS（exit 0；cargo 238 passed / 0 failed，bun test 20 pass，lint、typecheck、version:check、lint:sh） |
| alter | `bun run dist:dir && bun run e2e` | PASS（109 passed，含打包后 app 与 skill 示例的用例） |

| Scenario | Test | Result |
|---|---|---|
| R1.S1 | scripts/check-bundle.sh（`bun run dist` 后 `bash scripts/check-bundle.sh dist/mac-arm64/alter.app 0.1.0`，含挂载 DMG 检查） | PASS |
| R2.S1 | apps/browser/e2e/packaged.spec.ts::RC N2 R2.S1 打包后的 app 拉起包内的 alterd | PASS |
| R3.S1 | crates/alter-cli/tests/release_scripts.rs::rc_n2_r3_s1_tag_mismatch_fails | PASS |
| R3.S2 | crates/alter-cli/tests/release_scripts.rs::rc_n2_r3_s2_tag_matches | PASS |
| R4.S1 | crates/alter-cli/tests/install_script.rs::rc_n2_r4_s1_installs_given_version | PASS |
| R4.S2 | crates/alter-cli/tests/install_script.rs::rc_n2_r4_s2_installs_latest | PASS |
| R4.S3 | crates/alter-cli/tests/install_script.rs::rc_n2_r4_s3_replaces_old_version | PASS |
| R4.S4 | crates/alter-cli/tests/install_script.rs::rc_n2_r4_s4_path_hint | PASS |
| R5.S1 | crates/alter-cli/tests/install_script.rs::rc_n2_r5_s1_checksum_mismatch | PASS |
| R5.S2 | crates/alter-cli/tests/install_script.rs::rc_n2_r5_s2_version_not_found | PASS |
| R6.S1 | crates/alter-cli/tests/install_script.rs::rc_n2_r6_s1_dry_run | PASS |
| R6.S2 | crates/alter-cli/tests/install_script.rs::rc_n2_r6_s2_rejects_non_arm64 | PASS |
| R7.S1 | crates/alter-cli/tests/install_script.rs::rc_n2_r7_s1_with_skill_runs_skill_install | PASS |
| R8.S1 | crates/alter-cli/tests/nodejs.rs::rc_n2_r8_s1_runs_through_symlink | PASS |
| R9.S1 | crates/alter-cli/tests/skill.rs::rc_n2_r9_s1_installs_into_existing_agent_dirs | PASS |
| R9.S2 | crates/alter-cli/tests/skill.rs::rc_n2_r9_s2_no_agent_dirs | PASS |
| R9.S3 | crates/alter-cli/tests/skill.rs::rc_n2_r9_s3_target_replaces_old_install | PASS |
| R10.S1 | crates/alter-cli/tests/skill.rs::rc_n2_r10_s1_uninstall_only_marked | PASS |
| R11.S1 | crates/alter-cli/tests/upgrade.rs::rc_n2_r11_s1_check_finds_newer | PASS |
| R11.S2 | crates/alter-cli/tests/upgrade.rs::rc_n2_r11_s2_up_to_date | PASS |
| R11.S3 | crates/alter-cli/tests/upgrade.rs::rc_n2_r11_s3_runs_install_sh_and_refreshes_skill | PASS |
| R11.S4 | crates/alter-cli/tests/upgrade.rs::rc_n2_r11_s4_unreachable_api | PASS |
| R11.S5 | crates/alter-cli/tests/upgrade.rs::rc_n2_r11_s5_not_in_bundle | PASS |
| R12.S1 | crates/alterd/tests/gateway.rs::rc_n2_r12_s1_pushes_update_notice | PASS |
| R12.S2 | crates/alterd/tests/gateway.rs::rc_n2_r12_s2_no_notice_when_current_or_disabled | PASS |
| R12.S3 | crates/alterd/tests/gateway.rs::rc_n2_r12_s3_check_failure_is_logged | PASS |
| R13.S1 | packages/sdk/tests/update-notice.test.ts::RC N2 R13.S1 收到两次通知只打印一次 | PASS |
| R14.S1 | apps/browser/e2e/update.spec.ts::RC N2 R14.S1 显示更新提示条 | PASS |
| R15.S1 | crates/alter-core/tests/export_ts.rs::rc_n2_r15_s1_export_update_available | PASS |

收敛检查：整体评审第 1 轮的 CR3–CR5 由 T24、T25 收敛（第 4、5 轮）。T24、T25 只改了 e2e、样式注释与 design.md，没有改生产代码，报告 1 的覆盖率结论不变。打包后的 app 的 e2e 现在断言 release 构建的 alterd 在 `update_check = false` 时一次更新检查请求都没有发。

Gaps: none.

Overall: READY
