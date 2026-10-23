---
change: add-release-and-cookies
---

# 执行文档：从 Chrome / Arc 导入 Cookie

<!-- 执行文档。每个任务是一个薄的垂直切片，能由一个测试单独证明。
行语法：- [ ] T<n> [covers:R1.S1] [after:T<k>] [P] <做什么> — seam: <测试驱动的公开入口> — verify: <命令>
有变更的场景用 RED/GREEN，已有行为用 BASELINE，没有 covers 的任务用 GREEN/VERIFY。
对应的证据写进 log.md 之后才能勾选 [x]。执行时做不完的任务标 [blocked:<原因>]，问题写进 log.md。
完整语法见 references/formats.md -->

## Repo: alter

全节点约定：
- 新测试的命名一律带本需求前缀，免得和 add-core-features 的同名场景混淆。
  - Rust 用 `rc_n1_r<n>_s<m>_<描述>`。
  - e2e 标题以 `RC N1 R<n>.S<m>` 开头，`--grep "RC N1"` 只选中本节点。
- 任何测试都不得弹出真实钥匙串授权框。`crates/alterd/tests/common/mod.rs` 的 `Daemon::spawn*` 与 `apps/browser/e2e/app.ts` 的 `launchApp` 默认注入 `ALTER_KEYCHAIN_FAKE=denied`，调用方传入的 env 可以覆盖。这一条在 T3 落地，之后的任务都依赖它。
- Cookie fixture 由测试构造。`crates/alterd/tests/common/fixture_profile.rs` 新增 `write_cookies(dir, version, rows)` 与 `standard_cookies(dir)`，用口令 `peanuts` 按 design.md「解密」一节的算法加密。fixture 的加密实现放在测试代码里（dev 依赖与运行时依赖同一批 RustCrypto crate），不复用被测的解密函数，否则测不出算法错。e2e 用的 TS 版本放在 `apps/browser/e2e/fixtures/profile.ts`，加密用 Node 的 `crypto`。
- 安全分类器：T3、T4 涉及读取钥匙串与解密。用途是迁移用户本机自己的浏览器 profile，经 macOS 授权弹窗，明文不出本机。如果生成代码被拦，该任务标 `[blocked:classifier]`，`- blocked:` 里写明被拦的位置，其余不依赖它的任务（T2、T8 的界面部分、T9 的 help）照常做。
- 单向门（宪章执行策略）：Shell 协议字段与 `ImportedCookie` 字段以 design.md「接口契约」为准，不得另加字段；不改 SQLite 表结构；不改 `ErrorCode` 集合。

- [x] T1 [baseline] [covers:R9.S1,R9.S3,R7.S1,R8.S1,R8.S2] 钉住保留下来的场景：导出错误码 / 健康文件 / Gateway 类型、向导导入书签与历史、CLI 导入成功与来源不存在 — seam: `export-ts`、导入向导、`alter import` — verify: `cargo test -p alter-core --test export_ts n5_r12_s && cargo build -p alterd && cargo test -p alter-cli --test import n5_r9 && bun run e2e --grep "N5 R10.S1"`
  - 现有测试：`crates/alter-core/tests/export_ts.rs:54,191`、`crates/alter-cli/tests/import.rs:64,88`、`apps/browser/e2e/import.spec.ts:17`。在当前代码上全部 PASS，记 BASELINE。
  - R7.S1 的文字改成了「取消勾选 Cookie」。现在 Cookie 复选框不可用且未勾选（`import.spec.ts:44`），所以现有用例已经满足；T8 让复选框默认勾选后，同一个用例要先取消勾选，那时再跑一遍。

- [x] T2 [covers:R9.S2] 协议类型：`ShellCommand::ImportCookies`、`ShellEvent::CookiesImported`、`ImportedCookie` 及导出 — seam: `cargo run -p alter-core --bin export-ts` — verify: `cargo test -p alter-core --test export_ts`
  - 先改 `crates/alter-core/tests/export_ts.rs:118` 的期望集合，加上 `shell.importCookies` / `shell.event.cookiesImported`，再加一个对 `ImportedCookie.ts` 字段集合的断言，确认 RED。
  - 类型写在 `crates/alter-core/src/protocol/shell.rs`，仿照 `ImportProgress` 变体（`:154`），用 `#[serde(rename_all = "camelCase")]`。`expirationDate` 为 `Option<i64>`（整数秒，见 design.md），`skip_serializing_if = "Option::is_none"`；`sameSite` 是枚举，取值 `unspecified|no_restriction|lax|strict`（与 Electron 一致）。
  - 跑 `bun run protocol:export` 重新生成 `packages/protocol`，不得手改生成文件。
  - Main 的 `apps/browser/src/main/index.ts:120-130` 对未知 `cmd.type` 本来就忽略，加类型不会让 typecheck 失败；`bun run typecheck` 在本任务内跑一次确认。

- [x] T3 [covers:R1.S1,R1.S2] [after:T2] 端到端最薄切片：取口令（debug 替身）、读 `Cookies` 副本、解密 v10、下发给 cdpReady 连接、按回报计数 — seam: Gateway `import.run`（`crates/alterd/tests/import.rs`，真实 alterd 子进程 + 假 Electron 端） — verify: `cargo test -p alterd --test import rc_n1_r1`
  - 新文件：`crates/alterd/src/import/keychain.rs`、`crates/alterd/src/import/cookies.rs`。
  - 改动的文件：
    - `import/mod.rs`：删掉 `:72-77` 的拒绝分支，`:151` 的分支接 Cookie，`in_order` 加第三类。
    - `Cargo.toml`：workspace 依赖加 `security-framework`、`pbkdf2`（默认特性自带 HMAC）、`sha1`、`sha2`、`aes`、`cbc`，版本取 2026-10 的最新稳定版并钉住。
  - 假 Electron 端：用 `tests/common/fake_cdp.rs` 起假 CDP，它会写 `DevToolsActivePort` 并发 `shell.event.cdpReady`。再在同一个 Shell `Client` 上读 `shell.importCookies`，回 `shell.event.cookiesImported`。建一个辅助函数 `answer_cookies(client, imported, rejected)`。
  - alterd 的 Shell 分发（`crates/alterd/src/tabs/` 里处理 `ShellEvent::CdpReady` 的地方）记下发 `cdpReady` 的那个连接的 `Outbox`，连接断开时清掉。`CookiesImported` 按 `requestId` 交给 `HashMap<u32, oneshot::Sender>` 里等待的导入任务。这个 map 放在 TabService 的单线程循环里，不加锁。
  - 解密细节以 design.md「解密」为准：`key = PBKDF2-HMAC-SHA1(pw, "saltysalt", 1003, 16)`，`iv = [0x20; 16]`，PKCS#7。本任务先只覆盖 `meta.version` < 24 的路径，T4 再加主机名前缀。
  - 过期判断用 `expires_utc` 的 Chrome 时间，换算方式复用 `history.rs` 里现有的 1601 偏移，不另写常量。
  - 一批 500 项，一批等到回报后再发下一批。本任务的 fixture 只有 2 项有效，边界情况（恰好 500 项、501 项）由 T10 的 1000 条基准顺带覆盖，在那里断言 Electron 端收到 2 批。
  - `Daemon::spawn*` 默认注入 `ALTER_KEYCHAIN_FAKE=denied`，见全节点约定；`start_with_env` 传入的值覆盖它。
  - 代码会逼出的选择：
    - `requestId` 由 TabService 分配，从 1 开始自增（见 design.md）。
    - 解密函数签名 `fn decrypt(key: &[u8; 16], db_version: i64, host_key: &str, value: &str, encrypted: &[u8]) -> Option<String>`，返回 `None` 即跳过。

- [x] T4 [covers:R1.S3,R1.S4,R3.S1,R3.S2] [after:T3] Arc 的服务名、日志行、DB version ≥24 的主机名前缀、截断密文与明文值 — seam: Gateway `import.run` — verify: `cargo test -p alterd --test import rc_n1_r`
  - R1.S3 断言 stderr 里 JSON 日志 `取 Safe Storage 口令` 的 `service` 字段。日志只带 `service` 和 `ok`，不带口令。
  - R1.S4：明文前 32 字节与 `SHA256(host_key)` 比较，不等就跳过；fixture 的 `bad` 行专门造一个不相等的前缀。
  - R3.S1：截断密文让 CBC 解密报长度或填充错误，返回 `None`，计入 skipped。
  - R3.S2：`encrypted_value` 为空时取 `value` 列。没有 `v10` 前缀的非空密文跳过；这一条没有单独的场景，用单元测试 `cookies::tests` 钉住。
  - 边界：全部 Cookie 都过期或被跳过时，不发 `shell.importCookies`，`counts.cookies` 为 0，`status` 为 `done`。用一个单元测试钉住，不新增场景。

- [x] T5 [covers:R2.S1,R4.S1,R4.S2] [after:T3] 部分完成：钥匙串拒绝、没有 cdpReady 连接、Electron 超时 — seam: Gateway `import.run` / `import.status` — verify: `cargo test -p alterd --test import rc_n1_r2 && cargo test -p alterd --test import rc_n1_r4`
  - 实现 `ImportStatus::Partial`：现在没有任何代码会设它。
    - `set_state` 在 `mod.rs:349` 把 `warning` 写死成 `None`，改成写入 `imports.error`。
    - `ImportDone.warning` 在 `:230` 写死成 `None`，改成带上 warning。
    - `status()`（`:128`）已经会从 `error` 读 warning，不用改。
  - 优先级：先取口令，再判断有没有连接。所以拒绝授权时是 `keychain_denied`，而且根本不去找连接。
  - `ALTER_IMPORT_COOKIE_TIMEOUT_MS` 只用于测试缩短时限，默认 10000，release 构建同样读取（这不是安全边界）。
  - R4.S1 用 `Daemon::start_with_env` 而不建 Shell 客户端，用 Gateway 发起导入。现有的 `connected()` 会建 Shell 连接，但不发 `cdpReady`，所以它同样算没有连接。用例里两种情况都断言一次。
  - Shell 发起的导入（向导）走同样的 `partial`：`shell.importProgress` 最后一条带 `warning`，`run_for_shell` 已经在透传 `d.warning`（`mod.rs:519`）。

- [x] T6 [covers:R5.S1] [after:T3] 明文与口令不落盘、不写日志 — seam: Gateway `import.run` + alterd stderr + `ALTER_HOME` 文件扫描 — verify: `cargo test -p alterd --test import rc_n1_r5`
  - 用 `Daemon::stop_and_stderr()` 取全文。遍历 `ALTER_HOME` 下所有文件，按字节查找值和口令，包括 `alter.db`、`-wal` 和 `import-tmp/`。
  - 变异检查：故意在 `导入完成一类` 日志里带上第一个 Cookie 的 `value`，确认测试变红，再恢复。记为 `- RED (mutation: …)`。
  - 注意：临时副本 `import-tmp/<id>/Cookies` 里是密文，不含明文，所以按明文查找本来就查不到。这里断言的是导入结束后目录已删除或为空，与 R5.S1 的第三个 AND 对应。

- [x] T7 [covers:R6.S1] [after:T3] Electron Main 写入 partition 并回报 — seam: Electron e2e（`apps/browser/e2e/import.spec.ts`），本地测试服务器 `/echo-cookie` — verify: `bun run e2e --grep "RC N1 R6"`
  - 新文件 `apps/browser/src/main/cookie-import.ts`：导出 `importCookies(cmd): Promise<{imported, rejected}>`。对每一项调用 `session.fromPartition(\`persist:space-${spaceId}\`).cookies.set(details)`，`await` 成功的计 imported，抛错的计 rejected。
  - `index.ts:120-130` 的分发里加 `shell.importCookies`，结果经 `shell-link.ts` 发回 `shell.event.cookiesImported`。
  - partition 名称必须与 `tab-views.ts:148` 的 `persist:space-<id>` 完全一致。抽一个共享函数 `partitionFor(spaceId)` 放进 `apps/browser/src/main/` 已有的模块，`tab-views.ts`、`peek.ts:32` 和本文件都用它。这只是消掉重复的字符串，不是新抽象。
  - `apps/browser/e2e/fixtures/server.ts` 加 `/echo-cookie` 路由，把请求头 `Cookie` 原样写进页面 `<pre id="cookie">`。
  - e2e：`launchApp({ALTER_KEYCHAIN_FAKE: "peanuts", ALTER_CHROME_DIR: …})`，用 `fixtures/profile.ts` 新增的 `standardCookies(dir)` 写 Cookie 库，从向导导入 Cookie 到 `Work`。然后分别在两个 Space 打开 `/echo-cookie`，读主进程里对应 `WebContentsView` 的页面内容。读法沿用 AGENTS.md「内容区 URL 在主进程里读」的约定。
  - 前提：`ALTER_KEYCHAIN_FAKE` 只在 debug 构建的 alterd 里生效。e2e 启动的 app 不设 `ALTER_ALTERD_PATH` 时，按 `daemon.ts:28` 用的是 `target/debug/alterd`，满足这个前提。`bun run e2e` 另外构建的 release 版 alterd 只给 perf 用例用；本用例不得改用它。
  - 明文不进 Main 的日志。`stdio.ts` 的日志转发只打 alterd 的 stderr，本文件不 `console.log` 任何 Cookie 值。

- [x] T8 [covers:R7.S2,R7.S3] [after:T5] 向导：Cookie 复选框可用且默认勾选、说明文字、部分完成的原因 — seam: Electron e2e（导入向导） — verify: `bun run e2e --grep "RC N1 R7" && bun run e2e --grep "N5 R10.S1"`
  - `apps/browser/src/renderer/src/ImportWizard.tsx:17`：去掉 disabled 和「（暂不支持）」，默认勾选，旁边加说明「macOS 会请求钥匙串授权」。
  - `partial` 的原因文案用一张映射表：`keychain_denied` →「未获得钥匙串授权，Cookie 未导入」，`shell_unavailable` →「浏览器窗口未就绪，Cookie 未导入」。未知的 warning 原样显示。N2 的更新提示条复用这张表所在的样式。
  - 同步改 `import.spec.ts` 里 N5 R10.S1 的用例：先取消勾选 Cookie，再断言（对应 R7.S1 的新文字），并重跑一次。
  - R7.S3 用 `launchApp({ALTER_KEYCHAIN_FAKE: "denied"})`。

- [x] T9 [covers:R8.S3,R8.S4] [after:T5] [P] CLI：`partial` 时 stderr 打 warning、退出码为 0；help 去掉「暂不支持」 — seam: `alter import`（`assert_cmd`） — verify: `cargo build -p alterd && cargo test -p alter-cli --test import rc_n1_r8`
  - `crates/alter-cli/src/import.rs:61-67` 已经会在 stderr 打 `warning: …`，退出码也是 0。R8.S3 可能一写就通过：要做变异（让 `Partial` 返回 1），确认测试会变红。
  - `crates/alter-cli/src/main.rs:34` 的 help 文字改为列出 `bookmarks,history,cookies`。
  - 与 T8 独立：本任务只改 `crates/alter-cli/`，T8 只改 `apps/browser/`。

- [x] T10 [after:T5] 基准：1000 条 Cookie 的导入耗时（父级 SC3） — seam: `crates/alterd/tests/import_perf.rs`（`#[ignore]`） — verify: `cargo test -p alterd --test import_perf -- --ignored --nocapture`
  - 假 Electron 端立即回报。计时从 `import.run` 返回开始，到 `import.done` 为止，跑 5 次取中位数，断言 < 2000 ms。同时断言收到恰好 2 条 `shell.importCookies`（500 + 500），用来钉住批大小这个边界。
  - 基准用 debug 构建：`ALTER_KEYCHAIN_FAKE` 只在 debug 构建生效（design.md），不为基准另开 feature 开关。debug 构建更慢，结果只会高估，阈值不变。这一点在 log 里记为 `decided:`。

- [x] T11 [after:T1,T2,T3,T4,T5,T6,T7,T8,T9,T10] 节点门禁：全部质量命令 — seam: 仓库门禁 — verify: `bun run ci:check && bun run e2e --grep "RC N1|N5 R10"`
  - 覆盖率：`cargo llvm-cov -p alterd --test import` 查看 `import/cookies.rs`、`import/keychain.rs` 的行覆盖率，`bun test --coverage` 不涉及。改动行的覆盖率 ≥ 80%。
  - 钥匙串里 release 分支的代码（真实 `get_generic_password` 调用）测试中不会执行。这几行算进未覆盖，并在 log 里注明「系统边界，宪章允许 mock」。
  - 本机没有 `cargo llvm-cov` 时，按宪章把这一项标 blocked，其余继续。

## Convergence

- [x] T12 [covers:R6.S1] host-only Cookie 不带 domain 写入；R6.S1 的用例断言导入「完成」且 Cookie 2，并检查 partition 里 sid 是 host-only（CR4、CR8） — seam: Electron e2e（`apps/browser/e2e/import.spec.ts`） — verify: `bun run e2e --grep "RC N1 R6"`
  - `cookie-import.ts`：`domain` 不以 `.` 开头时，调用 `cookies.set` 前去掉 `domain`（Chromium 用 url 的主机名，记成 host-only）；以 `.` 开头时原样传。协议字段不变：`ImportedCookie.domain` 仍是源库的 `host_key`，design.md 的接口契约里补一句 Main 的这条处理。
  - 用例：关闭向导前，在同一次 `win.evaluate` 里断言结果区有「导入完成」「Cookie 2」「跳过 0」。再用 `app.evaluate` 读 `session.fromPartition(partition).cookies.get({name: "sid"})`，断言 `hostOnly === true`、`domain === "127.0.0.1"`。
  - 先 RED：只加断言、不改实现时，hostOnly 应当为 false。
- [x] T13 [covers:R5.S1] Cookies 库改为在内存里备份，不写 `import-tmp`（CR1） — seam: Gateway `import.run` + `ALTER_HOME` 文件扫描 — verify: `cargo test -p alterd --test import rc_n1_r5`
  - `import/mod.rs`：Cookies 用 `rusqlite::Connection::open_in_memory()`，再用 backup API 从源库只读备份进来，`cookies::read` 改为接收 `&Connection`；源库被锁、备份失败时，按现有 `snapshot` 的做法退回文件拷贝，但拷完立即备份进内存并删除这份拷贝，再开始解密。
  - 用例：R5.S1 的 fixture 加一行 `value` 列为明文的 `plain2=pl41n-v4lue-9c1e`（encrypted 为空）。导入过程中，假 Electron 端在收到 `shell.importCookies` 时、回报之前，扫描 `ALTER_HOME`，断言没有文件含这个明文值。
- [x] T14 [covers:R1.S1] 回报只认收到该批的连接、计数之和必须等于批大小；每批下发前确认 Space 仍存在；下发给最近一次 cdpReady 的连接，它断开后不退回旧连接（CR2、CR3、CR5） — seam: Gateway `import.run` + 两个假 Electron 端 — verify: `cargo test -p alterd --test import rc_n1_t14`
  - TabService：`cookie_waiters` 的值改为 `(Outbox, 批大小, Sender)`；`CookiesImported` 只在事件来自同一个发件箱（`Outbox::same_channel`）且 `imported + rejected == 批大小` 时交回。不符合时记一行 WARN（`忽略不匹配的 Cookie 写入回报`），不交回，等待方最终超时，结果为 `shell_unavailable`。
  - `send_cookies` 先查 `store.space_exists(space_id)`：Space 已不存在时不下发，返回一个可区分的结果，导入以 `failed` 结束，error 为 `目标 Space 已被删除`。这条没有现成的场景，只用测试钉住，并记 `decided:`：它与「导入行被级联删除时记 failed」是同一条已有规则。
  - 用例：
    - A、B 两个连接先后上报 cdpReady，断言命令只发给 B。
    - B 伪造一条 requestId 相同、计数不对的回报，或由 A 回报，都不被接受。
    - 关闭 B 后再次导入，断言 partial / shell_unavailable，而且 A 收不到命令。
    - 导入开始后、下发之前删除目标 Space：钥匙串替身里没有延迟，所以用 1000 条 Cookie 的两批，在第一批回报之前删除 Space，断言没有第二批，结果为 failed。
- [x] T15 [covers:R9.S2] SameSite 的字面量与映射、钥匙串条目（服务名、账户）的测试（CR6、CR7） — seam: `export-ts` + `import.run` + keychain 单元测试 — verify: `cargo test -p alter-core --test export_ts rc_n1_r9_s2 && cargo test -p alterd --test import rc_n1_t15 && cargo test -p alterd --bin alterd import::keychain`
  - `export_ts.rs`：`SameSite.ts` 的字面量集合恰好为 `unspecified`、`no_restriction`、`lax`、`strict`。
  - import 用例：库里 samesite 为 -1/0/1/2 的四行，断言 `shell.importCookies` 里依次是 unspecified、no_restriction、lax、strict。
  - keychain 单元测试：`entry(Chrome) == ("Chrome Safe Storage","Chrome")`，`entry(Arc) == ("Arc Safe Storage","Arc")`。
- [x] T16 [covers:R1.S1] 收敛第 2 轮：查询 Space 出错时不当成已删除；保留「目标 Space 已被删除」这个原因；改正协议注释（CR9、CR10、CR11） — seam: Gateway `import.run` / `import.status` — verify: `cargo test -p alterd --test import rc_n1_t14 && cargo test -p alter-core --test export_ts`
  - `CookieSend` 加 `Internal(String)`；`space_exists` 出错时返回它，导入以 failed 结束，error 为 `查询目标 Space 失败：<原因>`。
  - `run()` 的收尾：发现导入行已不在时，只有在原先没有 error 时才填泛化原因；已有的 error 保留。
  - `rc_n1_t14_space_deleted_mid_import` 再调用 `import.status`，断言 error 恰好为 `目标 Space 已被删除`（先 RED）。
  - `shell.rs` 里 `ImportedCookie` 的注释改为：`domain` 保存源库的 host_key，Main 对 host-only Cookie 去掉它再调用 `cookies.set`。改完重新运行 `bun run protocol:export`。
