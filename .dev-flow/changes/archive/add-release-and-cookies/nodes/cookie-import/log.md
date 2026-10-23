---
change: add-release-and-cookies
---

# 执行日志：从 Chrome / Arc 导入 Cookie

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

### T1 钉住保留下来的场景
- Revision: 1
- BASELINE: `cargo test -p alter-core --test export_ts n5_r12_s` → PASS on unchanged code（n5_r12_s1_export_error_code_and_health_file ok） (R9.S1)
- BASELINE: `cargo test -p alter-core --test export_ts n5_r12_s` → PASS on unchanged code（n5_r12_s3_export_gateway_types ok） (R9.S3)
- BASELINE: `bun run e2e --grep "N5 R10.S1"` → PASS on unchanged code（1 passed (5.4s)） (R7.S1)
- BASELINE: `cargo test -p alter-cli --test import n5_r9` → PASS on unchanged code（n5_r9_s1_import_prints_counts ok） (R8.S1)
- BASELINE: `cargo test -p alter-cli --test import n5_r9` → PASS on unchanged code（n5_r9_s2_missing_source_exits_1 ok） (R8.S2)

### T2 协议类型：ImportCookies、CookiesImported、ImportedCookie
- Revision: 1
- RED: `cargo test -p alter-core --test export_ts` → FAIL: rc_n1_r9_s2_export_shell_messages panicked at export_ts.rs:128 — left 缺 "shell.importCookies"（right 含它） (R9.S2)
- GREEN: `cargo test -p alter-core --test export_ts` → PASS (4 tests) (R9.S2)
- VERIFY: `bun run protocol:export` 生成 ImportedCookie.ts、SameSite.ts，更新 ShellCommand.ts、ShellEvent.ts；`cargo clippy --all-targets -- -D warnings`、`cargo test`、`bun test`（10 pass）、`bun run lint`、`bun run typecheck` 全部通过

### T3 端到端最薄切片：取口令、解密、下发、按回报计数
- Revision: 1
- RED: `cargo test -p alterd --test import rc_n1_r1` → FAIL: rc_n1_r1_s1_import_unexpired_cookies panicked — import.run 返回 E_INVALID_ARGUMENT「还不支持导入 Cookie」 (R1.S1)
- GREEN: `cargo test -p alterd --test import rc_n1_r1` → PASS (2 tests) (R1.S1)
- RED (mutation: 去掉 `skipped += rejected`，Main 回报的 rejected 不计入跳过): `cargo test -p alterd --test import rc_n1_r1` → FAIL: rc_n1_r1_s2_rejected_cookies_are_skipped — counts.skipped left 0，right 1 (R1.S2)
- GREEN: 恢复后 `cargo test -p alterd --test import rc_n1_r1` → PASS (2 tests) (R1.S2)
- VERIFY: Widen — `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test`（全部 ok）、`bun test`（10 pass）、`bun run lint`、`bun run typecheck` 通过；`bun run e2e --grep "N5 R10.S1"` → 1 passed（e2e 默认 `ALTER_KEYCHAIN_FAKE=denied` 之后）

### T4 Arc 的服务名、主机名前缀、截断密文与明文值
- Revision: 1
- 说明：四个场景的测试一写就通过——T3 为了让 R1.S1（标准 fixture 为 meta.version 24）变绿，已经实现了完整的 `decrypt`；按 tdd.md「Prove it can fail」逐个变异
- RED (mutation: keychain.rs 里 Arc 的条目改成 Chrome Safe Storage / Chrome): `cargo test -p alterd --test import rc_n1_r1_s3` → FAIL: services left ["Chrome Safe Storage"]，right ["Arc Safe Storage"] (R1.S3)
- GREEN: 恢复后 `cargo test -p alterd --test import rc_n1_` → PASS (7 tests) (R1.S3)
- RED (mutation: 去掉明文前 32 字节与 host_key 的 SHA256 的比较，只检查长度): `cargo test -p alterd --test import rc_n1_r1_s4` → FAIL: rc_n1_r1_s4_host_prefix_of_new_db panicked at import.rs:636（bad 被导入） (R1.S4)
- GREEN: 恢复后 `cargo test -p alterd --test import rc_n1_` → PASS (7 tests) (R1.S4)
- RED (mutation: 解密失败时不 `skipped += 1`): `cargo test -p alterd --test import rc_n1_r3_s1` → FAIL: counts.skipped left 0，right 1 (R3.S1)
- GREEN: 恢复后 `cargo test -p alterd --test import rc_n1_` → PASS (7 tests) (R3.S1)
- RED (mutation: `encrypted_value` 为空时返回 None 而不是 value 列): `cargo test -p alterd --test import rc_n1_r3_s2` → FAIL: rc_n1_r3_s2_plaintext_value panicked at import.rs:468（5 秒内没有收到 shell.importCookies） (R3.S2)
- GREEN: 恢复后 `cargo test -p alterd --test import rc_n1_` → PASS (7 tests) (R3.S2)
- VERIFY: 边界「全部过期不下发」`rc_n1_t4_all_expired_sends_nothing`，变异 `while !rest.is_empty()` → `loop` 后 FAIL（10 秒内没有 import.done），恢复后 PASS；单元测试 `import::cookies::tests`（非 v10 前缀跳过、空密文用 value 列）2 passed，变异「接受任意 3 字节前缀」后 non_v10_ciphertext_is_skipped FAIL（left Some("x")），恢复后 PASS。第一版非 v10 测试用随机字节，变异 `strip_prefix(b"v1")` 时仍通过，说明它测不出缺陷，已改为用合法 CBC 密文配 `v11` 前缀
- VERIFY: Widen — `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test`（全部 ok）、`bun test`（10 pass）通过

### T5 部分完成：钥匙串拒绝、没有 cdpReady 连接、Electron 超时
- Revision: 1
- RED: `cargo test -p alterd --test import rc_n1_r2` → FAIL: import.done status left "failed"，right "partial"（counts bookmarks 3、history 2、cookies 0） (R2.S1)
- GREEN: `cargo test -p alterd --test import rc_n1_r2` → PASS (1 test) (R2.S1)
- RED (mutation: 没有可写入连接时返回 Err 而不是 shell_unavailable): `cargo test -p alterd --test import rc_n1_r4_s1` → FAIL: status left "failed"，right "partial" (R4.S1)
- GREEN: 恢复后 `cargo test -p alterd --test import rc_n1_r4` → PASS (2 tests) (R4.S1)
- RED (mutation: 去掉等待回报的时限，一直等): `cargo test -p alterd --test import rc_n1_r4_s2` → FAIL: 2 秒内没有收到 import.done（import.rs:812） (R4.S2)
- GREEN: 恢复后 `cargo test -p alterd --test import rc_n1_r4` → PASS (2 tests) (R4.S2)
- VERIFY: Widen — `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test`（全部 ok）、`bun test`（10 pass）通过

### T6 明文与口令不落盘、不写日志
- Revision: 1
- 说明：测试写好时现有实现本来就不记录值（T3 的日志只带计数），一写就通过，按 tdd.md 用变异证明
- RED (mutation: 下发每批前用 tracing::info 打出第一个 Cookie 的 value): `cargo test -p alterd --test import rc_n1_r5` → FAIL: rc_n1_r5_s1_no_plaintext_left panicked at import.rs:864（stderr 含 Cookie 明文） (R5.S1)
- GREEN: 恢复后 `cargo test -p alterd --test import rc_n1_r5` → PASS (1 test) (R5.S1)
- VERIFY: 第二个变异「导入结束不删除 import-tmp」→ FAIL at import.rs:875（import-tmp 应当已删除或为空），恢复后 PASS；`cargo clippy --all-targets -- -D warnings`、`cargo test -p alterd` 通过

### T6 明文与口令不落盘、不写日志
- Revision: 1
- 说明：这是 T6 的第二次尝试。第一次（提交 8c1d0d7）的 VERIFY 写了「`cargo test -p alterd` 通过」，但那次实际输出是 `rc_n1_r5_s1_no_plaintext_left ... FAILED`（import.rs:875，import-tmp 残留），记录有误，以本块为准
- 根因：变异脚本先用 `cp` 备份源文件，再用 `mv` 还原。还原后的 mtime 早于带变异编出的二进制，cargo 判断不需要重编，于是一直在跑「不删除 import-tmp」那个变异版的 alterd。插桩确认：重新编译后删除成功、用例通过；`git diff` 也确认已提交的源码没有被改动。变异脚本已改为还原后 `touch` 源文件
- RED (mutation: 下发每批前用 tracing::info 打出第一个 Cookie 的 value): `cargo test -p alterd --test import rc_n1_r5` → FAIL: panicked at import.rs:864（stderr 含 Cookie 明文） (R5.S1)
- GREEN: 恢复并重新编译后 `cargo test -q -p alterd`（连跑 3 次）→ 全部 ok，含 rc_n1_r5_s1_no_plaintext_left (R5.S1)
- VERIFY: 变异「导入结束不删除 import-tmp」→ FAIL at import.rs:875，正好就是上面那次误判时看到的失败；Widen — `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test`（全部 ok）、`bun test`（10 pass）通过

### T8 向导：Cookie 复选框可用且默认勾选、说明文字、部分完成的原因
- Revision: 1
- RED: `bun run e2e --grep "RC N1 R7"` → FAIL: R7.S2 expect(locator).toBeEnabled() — Received: disabled (R7.S2)
- GREEN: `bun run e2e --grep "RC N1 R7"` → R7.S2 ✓ (1.3s) (R7.S2)
- RED: `bun run e2e --grep "RC N1 R7"` → FAIL: R7.S3 5000 ms 内条件没有同时成立 — 结果区没有「未获得钥匙串授权，Cookie 未导入」：部分完成书签 3历史 2Cookie 0跳过 0keychain_denied (R7.S3)
- GREEN: `bun run e2e --grep "RC N1 R7|N5 R10.S1"` → 3 passed (3.1s) (R7.S3)
- VERIFY: R7.S1 的用例（N5 R10.S1）改为先取消勾选 Cookie 再导入，重跑 ✓；`bun run lint`、`bun run typecheck`、`bun test`（10 pass）通过

### T7 Electron Main 写入 partition 并回报
- Revision: 1
- RED: `bun run e2e --grep "RC N1 R6"` → FAIL: Work 的页面：cookie: — Expected substring "sid=abc"，Received "cookie: " (R6.S1)
- GREEN: `bun run e2e --grep "RC N1 R6"` → ✓ (2.1s) (R6.S1)
- VERIFY: `@alter/protocol` 的手写入口 `index.ts` 补上 `ImportedCookie`、`SameSite` 的再导出；`bun run typecheck`、`bun run lint`、`bun test`（10 pass）通过；`partitionFor` 抽到 tab-views.ts，tab-views / peek / cookie-import 共用；完整 `bun run e2e` → 105 passed (2.1m)

### T9 CLI：partial 时 stderr 打 warning、退出码为 0；help 去掉「暂不支持」
- Revision: 1
- RED (mutation: `Partial` 和 `Failed` 一样以退出码 1 结束): `cargo test -p alter-cli --test import rc_n1_r8_s3` → FAIL: code=Some(1) stdout="" stderr="导入失败（import 1），见 alter 的日志" (R8.S3)
- GREEN: 恢复后 `cargo test -p alter-cli --test import` → PASS (4 tests) (R8.S3)
- RED: `cargo test -p alter-cli --test import rc_n1_r8` → FAIL: rc_n1_r8_s4_help_lists_cookies — help 为「从 Chrome / Arc 导入书签、历史（Cookie 暂不支持）到某个 Space」 (R8.S4)
- GREEN: `cargo test -p alter-cli --test import rc_n1_r8` → PASS (2 tests) (R8.S4)
- VERIFY: CLI 测试的 alterd 默认 `ALTER_KEYCHAIN_FAKE=denied`；Widen — `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test`（全部 ok）通过

### T10 基准：1000 条 Cookie 的导入耗时（父级 SC3）
- Revision: 1
- VERIFY: `cargo test -p alterd --test import_perf -- --ignored --nocapture` → PASS：「SC3 导入 1000 条 Cookie：5 次 [36.0ms, 36.5ms, 37.4ms, 38.6ms, 45.2ms]，中位数 37.389833ms」（阈值 2000 ms，debug 构建）
- VERIFY: 批大小边界：每次恰好收到 500 + 500 两批；变异 `BATCH = 400` 后用例 FAIL（第三批没人回报，import_perf.rs:70 等不到 import.done），恢复后 PASS；`cargo clippy --all-targets -- -D warnings` 通过

### T11 节点门禁：全部质量命令
- Revision: 1
- VERIFY: `bun run ci:check` → exit 0（fmt、clippy -D warnings、cargo test 全部 ok、eslint、tsc、bun test 10 pass、版本号一致：0.1.0）
- VERIFY: `bun run e2e --grep "RC N1|N5 R10"` → 4 passed (4.6s)
- VERIFY: 覆盖率——本机没有 `cargo llvm-cov`，改用 rustc 自带的 `-C instrument-coverage`，配 Xcode 命令行工具的 `xcrun llvm-profdata` / `xcrun llvm-cov`（Apple LLVM 21），输出到 scratchpad 下单独的 target 目录：`RUSTFLAGS="-C instrument-coverage" cargo test -p alterd --test import --bin alterd`（16 + 20 passed）→ 合并 profraw → `llvm-cov export -format=lcov` → 与 `git diff -U0 70c7e41..HEAD -- crates/alterd/src` 的新增行求交集：改动的可执行行 224/255 = 87.8%（≥ 80%）。cookies.rs 83/93、keychain.rs 16/24、mod.rs 92/102、tabs/mod.rs 33/36
- 未覆盖行说明：keychain.rs 的 29、38–47 是 release 分支，会真实调用 `get_generic_password`（系统边界，宪章允许 mock，测试中不会执行）；cookies.rs 的未覆盖行是读库失败、单行读取失败、samesite 0/1/2 的分支；mod.rs 是临时目录删除失败、TabService 已停止、序列化失败等错误路径；tabs/mod.rs 190–192 是 `shell.importCookies` 序列化失败的分支。Electron Main 的 `cookie-import.ts` 与向导改动由 e2e 覆盖（R6.S1、R7.S2、R7.S3），Electron 进程内没有行覆盖率工具

### T12 host-only Cookie 不带 domain 写入；R6.S1 断言导入完成且有回报
- Revision: 1
- 说明：R6.S1 加上 hostOnly 断言后一写就通过，原因是 fixture 的主机是 IP `127.0.0.1`，Chromium 对 IP 只认 host-only。所以 CR8 改用边界用例 `RC N1 T12` 来证明（主机名 app.example.test 加 `__Host-` Cookie）
- RED (mutation: Main 写入后不发 shell.event.cookiesImported): `bun run e2e --grep "RC N1 R6"` → FAIL: 结果区没有「导入完成」：部分完成…Cookie 0…浏览器窗口未就绪，Cookie 未导入 (R6.S1)
- GREEN: 恢复后 `bun run e2e --grep "RC N1|N5 R10"` → 5 passed (5.1s) (R6.S1)
- VERIFY: 边界用例 `RC N1 T12`：修复前 FAIL（alterd 日志 `导入完成一类` count 2、skipped 1，即 `__Host-sid` 被 Electron 拒绝），修复后 ✓；`bun run lint`、`bun run typecheck` 通过；N1 design.md「接口契约」已补上 Main 的 domain 处理

### T13 Cookies 库改为在内存里备份，不写 import-tmp
- Revision: 1
- RED: `cargo test -p alterd --test import rc_n1_r5` → FAIL: 导入过程中这些文件含明文 Cookie：[…/import-tmp/1/Cookies] (R5.S1)
- 中间一次失败：第一版写成 `run_to_completion(-1, …)`，rusqlite 断言 `pages_per_step > 0`，在阻塞线程里 panic，10 个 Cookie 用例都变成 failed。读 rusqlite 0.40 的 backup.rs 定位到原因，改为循环调用 `step(-1)`，被锁时每 100 ms 重试一次，最多 10 次
- GREEN: `cargo test -p alterd --test import` → PASS (20 tests)；`cargo test -p alterd --bin alterd` → PASS (16 tests) (R5.S1)
- VERIFY: `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test`（全部 ok）通过；N1 design.md 与时序图已同步。源库被锁时退回文件拷贝的那条路径没有测试覆盖：要模拟 Chromium 持有的独占锁，测试里做不稳定；这条路径沿用 history 已有的 `copy_in` 写法

### T14 回报的连接与计数校验、每批前确认 Space、只发给最近一次上报 cdpReady 的连接
- Revision: 1
- RED: `cargo test -p alterd --test import rc_n1_t14` → FAIL: rc_n1_t14_only_latest_cdp_connection_counts — import.done counts cookies 1、skipped 1（A 伪造的回报被接受了），期望 2 (R1.S1)
- RED: 同一条命令 → FAIL: rc_n1_t14_space_deleted_mid_import panicked at import.rs:42（Space 删除后第二批照常下发，10 秒内等不到 import.done） (R1.S1)
- GREEN: `cargo test -p alterd --test import` → PASS (22 tests) (R1.S1)
- VERIFY: 变异「只认第一个上报 cdpReady 的连接」→ rc_n1_t14_only_latest_cdp_connection_counts FAIL（B 收不到命令），恢复后 PASS (2 tests)；`cargo clippy --all-targets -- -D warnings`、`cargo test`（全部 ok）通过；N1 design.md「接口契约」已补上回报校验与每批确认 Space

### T15 SameSite 字面量与映射、钥匙串条目的测试
- Revision: 1
- 说明：这些测试钉住的是 T2 / T3 已有的实现，一写就通过，所以逐个做变异
- RED (mutation: SameSite 改为 rename_all camelCase): `cargo test -p alter-core --test export_ts rc_n1_r9_s2` → FAIL: left {"lax","noRestriction","strict","unspecified"}，right 含 "no_restriction" (R9.S2)
- GREEN: 恢复后 `cargo test -p alter-core --test export_ts` → PASS (R9.S2)
- VERIFY: 变异「samesite 1/2 的映射对调」→ rc_n1_t15_same_site_mapping FAIL（left "strict"，right "lax"），恢复后 PASS；变异「Arc 的账户改成 Chrome」→ keychain_entries_match_browsers FAIL，恢复后 PASS；`cargo clippy --all-targets -- -D warnings`、`cargo test`（全部 ok）通过

### T16 收敛第 2 轮：查询出错不当成已删除、保留原因、改正协议注释
- Revision: 1
- RED: `cargo test -p alterd --test import rc_n1_t14_space` → FAIL: import.status 的 error，left "导入记录已不存在（目标 Space 可能已被删除）"，right "目标 Space 已被删除" (R1.S1)
- GREEN: `cargo test -p alterd --test import` → PASS (23 tests) (R1.S1)
- VERIFY: `bun run protocol:export` 重新生成 ImportedCookie.ts（只有注释变化）；`cargo test -p alter-core --test export_ts`（4 passed）、`cargo test`（全部 ok）、`cargo clippy --all-targets -- -D warnings`、`bun run typecheck` 通过。`CookieSend::Internal` 这条分支（导入途中查询 Space 出错）测试里没法稳定复现，没有覆盖

## 决策与偏差

- design: `ImportedCookie.expirationDate` 从「UNIX 秒，有小数（f64）」改为整数秒（`Option<i64>`），因为 `ShellCommand` 派生了 `Eq`，f64 字段会破坏这一点；Electron `cookies.set` 接受整数秒，Chrome 的 `expires_utc` 精度到微秒，向下取整到秒对 Cookie 过期没有可见影响。design.md「接口契约」已同步
- decided: 旧测试 `n5_r12_s2_export_shell_messages` 改名为 `rc_n1_r9_s2_export_shell_messages` 并更新集合，因为 R9.S2 取代了原场景（同名 MODIFIED） — from: tasks.md T2 备注「先改 export_ts.rs:118 的期望集合」
- decided: `tabs/mod.rs` 先给 `ShellEvent::CookiesImported` 加一个返回 `Ok(false)` 的分支让工作区可编译，T3 再接到等待的导入任务 — from: T2 只交付类型，路由属于 T3

- design: `shell.importCookies` 的 `requestId` 从「ImportService 内的 AtomicU32」改为由 TabService 分配：等待方的登记本来就在 TabService 的单线程循环里，同一处分配与登记，不需要原子量。design.md「接口契约」已同步
- design: 不单独引入 `hmac`：pbkdf2 0.13 的默认特性自带 HMAC；`cbc` 开 `alloc` 特性用 `decrypt_padded_vec`。父级与 N1 的 design.md 已同步。依赖版本：security-framework 3.7.0、pbkdf2 0.13.0、sha1 0.11.0、sha2 0.11.1、aes 0.9.3、cbc 0.2.1（crates.io 当前最新）
- decided: alter-cli 增加同一批 dev 依赖 — from: alter-cli 的测试经路径复用 alterd 的 fixture_profile（Cookie fixture 要加密），宪章执行策略允许直接加 dev 依赖
- decided: R1.S1 的最小实现已包含 `meta.version ≥24` 的主机名前缀处理 — from: 标准 Cookie fixture 的 `meta.version` 就是 24（requirement.md 术语），不处理前缀 R1.S1 无法变绿；T4 的 R1.S4 若一写就通过，按 tdd.md 用变异证明

- decided: R4 的两个场景随 R2.S1 的同一段实现一起成立（部分完成的状态、warning 与 error 列是一条路径），按 tdd.md 用变异分别证明 — from: tdd.md「When the test passes at once」
- decided: 一类数据部分完成时另记一行 WARN `导入部分完成`（带 item、warning），不带任何 Cookie 内容 — from: 父级 requirement.md 可观测性 NFR，日志只带计数与原因

- decided: 复核此前每轮变异之后有没有跑在旧二进制上 — T4 最后一个变异之后改了 cookies.rs，T5 最后一个变异之后 `cargo fmt` 改写了 mod.rs，两次都触发了重编，随后的整套测试跑在正确的二进制上，那两轮的证据有效。另外更正一点：T4 证据里那几行「恢复后 … PASS (7 tests)」，实际看到的是变异之前那次运行的结果；变异恢复之后的 GREEN，以 T4 的 Widen 为准（重编之后整套 `cargo test` 全部 ok） — from: 本次排查（references/debugging.md）

- decided: 先做 T8 再做 T7 — from: R6.S1 的 WHEN 是「从导入向导导入 Cookie」，T8 之前复选框不可用；两者都已在可执行任务里，T7 依赖的向导入口由 T8 提供
- decided: e2e 的 Cookie fixture（`fixtures/profile.ts` 的 `writeCookies` / `standardCookies`）在 T8 写好，因为 R7.S3 需要 profile 里真有 `Cookies` 库（没有库时不会去取口令，也就不会有 keychain_denied） — from: design.md「cookies()：profile 里没有 Cookies 时为 0，也不去碰钥匙串」
- decided: `/echo-cookie` 路由在 `e2e/fixtures/server.ts` 里已经存在（add-core-features N1 用于 Cookie 隔离），T7 直接复用它返回的 `cookie: …` 正文，不另加 `<pre id="cookie">` — from: 现有代码

- decided: 基准用 debug 构建跑，不为它另开 feature 开关 — from: tasks.md T10 备注（`ALTER_KEYCHAIN_FAKE` 只在 debug 构建里生效；debug 更慢，结果只会高估）

- decided: 本机没有 `cargo llvm-cov`，覆盖率改用 rustc 自带的 `-C instrument-coverage` 加 Xcode 命令行工具里的 `llvm-profdata` / `llvm-cov` 来测。测的是同一个指标（改动行的行覆盖率），不需要安装任何东西，所以不按「缺工具即 blocked」处理 — from: 宪章质量门禁「改动行覆盖率 ≥ 80%」；执行策略里的 blocked 规则针对的是测不了的情况，这里测得了

- decided: 第 1 轮的定级理由 — CR1：macOS 上的 Chrome 会加密全部 Cookie，明文 `value` 行在真实 profile 里几乎不会出现，删除失败只记 WARN 也是本次改动之前就有的写法，所以降为 minor，但改成内存备份很便宜，仍然修；CR2：`shell.sock` 权限是 0600，伪造者必须是同一用户的进程，降为 minor；CR3：孤儿 partition 会在下次启动时被 `partitions::sweep` 删掉，降为 minor。CR8 不是 codex 提的，是我核实其他发现时读代码发现的，已对照 Electron 的 `CookiesSetDetails.domain` 文档确认（「will be normalized with a preceding dot」） — from: 核实 Round 1 的代码与文档
- decided: 评审者 `claude:opus-5` 本轮不可用：`claude -p --model opus-5` 返回 `There's an issue with the selected model (opus-5). It may not exist or you may not have access to it.` 按规则不换模型、不修改 config.json，本会话内也不再重试；要不要改这一项由用户决定 — from: principles.md「Reviewers」
- decided: 第一次派发评审者时，命令里的 `timeout` 在 macOS 上不存在（exit 127），两位评审者都没有启动。改用 `perl -e 'alarm 900; exec …'` 做 15 分钟限时后重新派发 — from: 实际命令输出

- design: Main 写入时，host-only Cookie（`domain` 不以点开头）不传 `domain`。协议字段不变，`ImportedCookie.domain` 仍是源库的 `host_key`，N1 design.md 已同步（CR8）

- design: Cookies 不再经 `snapshot()` 在 `import-tmp` 下做磁盘副本，改为用 backup API 读进内存库（CR1）；history 仍用 `snapshot()`。N1 design.md 与 seq-cookie-import.mmd 已同步

- decided: 导入期间目标 Space 被删除时，导入以 failed 结束（error 为「目标 Space 已被删除」） — from: 已有规则「导入行被级联删除时记 failed」（N5 design.md），这里只是提前在下发前发现；没有对应的场景，用 rc_n1_t14_space_deleted_mid_import 钉住
- design: `TabService::send_cookies` 的返回值从 `Option<Receiver>` 改为 `CookieSend { Sent, NoShell, SpaceGone }`；等待方记下连接与批大小（CR2、CR3）

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
- Scope: alter `git diff 70c7e41...HEAD`（不含 Cargo.lock 与 .dev-flow/；没有未提交的改动），基准为批准后、第一个任务之前的提交
- Reviewers: codex (gpt-5.6-sol): 7 findings; claude (opus-5): unavailable (unknown model)
- Tasks: T1-T11
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR1 | minor | security | crates/alterd/src/import/mod.rs:374 | Cookies 库经 `snapshot()` 整份写进 `ALTER_HOME/import-tmp`：`value` 列为明文的行（R3.S2）会在 ALTER_HOME 下多出一份明文，删除失败时只记 WARN（R5.S1） | accepted: T13 已把常规路径改为内存备份；只有源库被锁时才退回拷贝到 import-tmp，读进内存后立即删除，删除失败记 WARN，与 history 的既有做法一致 |
| CR2 | minor | security | crates/alterd/src/tabs/mod.rs:443 | `cookiesImported` 只按 requestId 交给等待方：不核对回报是否来自收到该批的连接，也不检查 imported + rejected 是否等于这一批的条数，同一用户的其他 Shell 连接能伪造计数（R1.S1、R1.S2） | fixed in R3 |
| CR3 | minor | correctness | crates/alterd/src/tabs/mod.rs:180 | 只在开始时校验目标 Space：取口令或解密期间 Space 被删除，仍会下发，Main 会在已删除 Space 的 partition 里写入 Cookie，直到下次启动 sweep 才被删掉 | fixed in R3 |
| CR4 | major | tests | apps/browser/e2e/import.spec.ts:161 | R6.S1 的用例只等「完成」按钮，而 partial 也会显示这个按钮；Main 写入后完全不回报 `cookiesImported` 时用例照样通过，没有证明回报这条链路（R6.S1） | fixed in R3 |
| CR5 | minor | tests | crates/alterd/tests/import.rs:443 | 成功路径只有一个 Shell 连接：实现改为下发给第一个 cdpReady 连接，或在它断开后退回旧连接，现有测试都测不出来（R1、R4.S1） | accepted: T14 已用两个连接钉住「最近一次」与「不退回旧连接」，变异「只认第一个」能被测出；B 断开后等 200 ms 不是确定性屏障，但尚未察觉断开时发给 B，结果同样是 partial / shell_unavailable，也符合 R4 |
| CR6 | minor | tests | crates/alter-core/tests/export_ts.rs:173 | `SameSite.ts` 的字面量集合，以及库里 samesite 取值 -1/0/1/2 的映射，都没有测试（fixture 全是 -1）（R9.S2、R1） | fixed in R3 |
| CR7 | minor | tests | crates/alterd/src/import/keychain.rs:9 | 钥匙串替身不看 account：Arc 的账户错写成 Chrome 时测试照样通过，release 构建却会取不到口令（R1.S3） | fixed in R3 |
| CR8 | major | correctness | apps/browser/src/main/cookie-import.ts:15 | host-only Cookie（`host_key` 不以点开头）连同 `domain` 一起传给 `cookies.set`，Electron 会补上前导点，把它变成对子域名也生效的 domain Cookie；`__Host-` 前缀的 Cookie 必须不带 Domain，会被直接拒绝（R6.S1、R1.S1） | fixed in R3 |

Review: CHANGES REQUESTED

### Round 2
- Revision: 1
- Scope: alter `git diff 30587c4...HEAD`（第 1 轮之后的收敛修复，不含 .dev-flow/；没有未提交的改动）
- Reviewers: codex (gpt-5.6-sol): 3 findings; claude (opus-5): unavailable (unknown model，本会话不再重试)
- Tasks: T12, T13, T14, T15
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR9 | minor | correctness | crates/alterd/src/tabs/mod.rs:200 | `space_exists` 查询出错（库损坏、I/O）也返回 `CookieSend::SpaceGone`，导入的 error 会误报为「目标 Space 已被删除」 | fixed in R3 |
| CR10 | minor | correctness | crates/alterd/src/import/mod.rs:247 | Space 被删除时级联删掉导入行，`finish_import` 返回 false，收尾逻辑无条件把 error 覆盖为「导入记录已不存在…」，「目标 Space 已被删除」这个原因丢了 | fixed in R3 |
| CR11 | minor | maintainability | crates/alter-core/src/protocol/shell.rs:133 | `ImportedCookie` 的注释仍写着「Main 原样传入」，与 host-only Cookie 去掉 domain 的处理矛盾 | fixed in R3 |

Review: CHANGES REQUESTED

### Round 3
- Revision: 1
- Scope: alter `git diff d1c97d9...HEAD`（T16 的修复，不含 .dev-flow/；没有未提交的改动）。第 2 轮已经由 codex 确认 CR2、CR3、CR4、CR6、CR7、CR8 修复（T12–T15）
- Reviewers: codex (gpt-5.6-sol): 1 finding; claude (opus-5): unavailable (unknown model，本会话不再重试)
- Tasks: T16
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR12 | minor | correctness | crates/alterd/src/import/mod.rs:257 | `error.get_or_insert(reason)` 同时作用于写库失败和 TabService 已停止：某一类数据已出错时，收尾写库失败的原因不会覆盖它 | accepted: 这种情况下导入无论如何都是 failed，写库失败本身已单独记 ERROR「导入结果没有写进数据库」（带原因），排查信息不丢；只有导入行被级联删除时才需要保留更具体的原因，这正是 CR10 修复的目的 |

Review: PASS

## 验证报告
<!-- 每次验证追加一份新报告，放在最近一轮评审之后。以 "- Revision: <当前 revision>" 和 "- Review: Round <最近一轮>" 开头。由验证步骤填写：本仓库的测试结果，每个场景一行 "| R<n>.S<m> | <path::test> | PASS |"，成功标准对照表，收敛缺口，fix 类型还要写 "Fix: verified | partial | failed"，最后恰好一行 "Overall: READY" 或 "Overall: NOT READY"。 -->

### 节点报告 1
- Revision: 1
- Review: Round 3
- 运行环境：本机 macOS（Apple Silicon），分支 feature/add-release-and-cookies，HEAD 900fe04，工作区干净

| Repo | Command | Result |
|---|---|---|
| alter | `bun run ci:check` | PASS（exit 0；cargo 191 passed / 0 failed，eslint，tsc，bun test 10 pass，版本号一致：0.1.0） |
| alter | `bun run e2e --grep "RC N1\|N5 R10"` | PASS（5 passed (6.5s)） |
| alter | `cargo test -p alterd --test import_perf -- --ignored --nocapture` | PASS（中位数 29.5 ms，阈值 2000 ms，父级 SC3） |
| alter | 改动行覆盖率（rustc `-C instrument-coverage` + `xcrun llvm-cov`，`git diff 70c7e41..HEAD -- crates/alterd/src`） | PASS（256/314 = 81.5% ≥ 80%） |

| Scenario | Test | Result |
|---|---|---|
| R1.S1 | crates/alterd/tests/import.rs::rc_n1_r1_s1_import_unexpired_cookies | PASS |
| R1.S2 | crates/alterd/tests/import.rs::rc_n1_r1_s2_rejected_cookies_are_skipped | PASS |
| R1.S3 | crates/alterd/tests/import.rs::rc_n1_r1_s3_arc_uses_arc_safe_storage | PASS |
| R1.S4 | crates/alterd/tests/import.rs::rc_n1_r1_s4_host_prefix_of_new_db | PASS |
| R2.S1 | crates/alterd/tests/import.rs::rc_n1_r2_s1_keychain_denied_is_partial | PASS |
| R3.S1 | crates/alterd/tests/import.rs::rc_n1_r3_s1_truncated_ciphertext_is_skipped | PASS |
| R3.S2 | crates/alterd/tests/import.rs::rc_n1_r3_s2_plaintext_value | PASS |
| R4.S1 | crates/alterd/tests/import.rs::rc_n1_r4_s1_no_shell_is_partial | PASS |
| R4.S2 | crates/alterd/tests/import.rs::rc_n1_r4_s2_no_answer_is_partial | PASS |
| R5.S1 | crates/alterd/tests/import.rs::rc_n1_r5_s1_no_plaintext_left | PASS |
| R6.S1 | apps/browser/e2e/import.spec.ts::RC N1 R6.S1 只有目标 Space 带上导入的 Cookie | PASS |
| R7.S1 | apps/browser/e2e/import.spec.ts::N5 R10.S1 从向导导入 | PASS |
| R7.S2 | apps/browser/e2e/import.spec.ts::RC N1 R7.S2 Cookie 可选且默认勾选 | PASS |
| R7.S3 | apps/browser/e2e/import.spec.ts::RC N1 R7.S3 拒绝授权时显示原因 | PASS |
| R8.S1 | crates/alter-cli/tests/import.rs::n5_r9_s1_import_prints_counts | PASS |
| R8.S2 | crates/alter-cli/tests/import.rs::n5_r9_s2_missing_source_exits_1 | PASS |
| R8.S3 | crates/alter-cli/tests/import.rs::rc_n1_r8_s3_keychain_denied_warns | PASS |
| R8.S4 | crates/alter-cli/tests/import.rs::rc_n1_r8_s4_help_lists_cookies | PASS |
| R9.S1 | crates/alter-core/tests/export_ts.rs::n5_r12_s1_export_error_code_and_health_file | PASS |
| R9.S2 | crates/alter-core/tests/export_ts.rs::rc_n1_r9_s2_export_shell_messages | PASS |
| R9.S3 | crates/alter-core/tests/export_ts.rs::n5_r12_s3_export_gateway_types | PASS |

收敛检查：
- 对照 N1 requirement.md 与 design.md 逐条核对，没有 missing / partial / contradicts。
- unrequested：每批下发前确认 Space 还在（Space 已删时 failed），以及回报校验，这两条是评审收敛时加的。都在 design.md 的「接口契约」里，并由 T14 的测试钉住；它们只收紧已有行为，不新增对外能力。
- 宪章：
  - 只有 alterd 读源数据、写 SQLite；Main 只写 Chromium 自己的 Cookie 存储。
  - 新增运行时依赖都在 design.md 写明了理由。
  - 测试不触发真实钥匙串：Daemon 与 e2e 的默认值都是 `ALTER_KEYCHAIN_FAKE=denied`。
- NFR：口令与明文不落盘、不写日志由 R5.S1 证明；性能见上表 SC3。

Gaps: none.

Overall: READY
