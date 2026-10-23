---
change: add-release-and-cookies
---

# 执行日志：补齐：Cookie 导入、发布安装、主页与 agent skill

<!-- 流程路线型需求的父级日志。任务证据写在各节点自己的 log.md（nodes/<node-slug>/log.md）；这份文件记录整体的决策，以及所有节点完成后做的整体评审和整体验证。没有亲眼看到的结果不要写。 -->

## 决策与偏差
- decided: 起点状态 — 2026-10-22 开始 run：仓库 /Users/bytedance/Documents/apps/alter，分支 `feature/add-release-and-cookies`，HEAD 70c7e41，工作区干净，单一 worktree；`cargo test`（全部 ok）与 `bun test`（10 pass）全绿 — from: 执行前检查实际运行
- decided: 执行前检查 — cargo、bun 1.3.14、shellcheck、hdiutil、codesign、ruby、gh、codex、claude 均可用，能访问 github.com；`cargo llvm-cov` 未安装，按宪章执行策略「本机缺工具时把对应任务标为 blocked」，覆盖率检查留到各节点门禁任务处理 — from: constitution.md 执行策略

## 代码评审
<!-- 由整体评审步骤填写，每次对整个需求的 diff 评审一轮：
### Round <n>
- Revision: <父级当前 revision>
- Scope: <repo> `git diff <base>...HEAD`（含未提交的改动）
- Reviewers: <agent> (<model>): <n> findings; <agent> (<model>): <n> findings | unavailable (<原因>)（配置的 reviewer 各返回了什么；没配置时写 `<agent> (<model>, session)` 或 `(<model>, self-review)`）
- Nodes: N1 (T1-T3), N2 (T1), …（全部节点，每个节点后面括号里是这一轮读过代码的任务；后续轮只列重读的，N1 (T9), N2 ()）
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
Action 写代码所属节点里的收敛任务：N1/T9。最后恰好一行 "Review: PASS" 或 "Review: CHANGES REQUESTED"。语法见 references/formats.md §14 -->

### Round 1
- Revision: 1
- Scope: alter `git diff 70c7e41...HEAD`（整个需求，不含 Cargo.lock 与 .dev-flow/），重点是 flow.md 的 Handoffs 与父级 design.md 的接口契约；各节点内部的代码已在各自的评审轮里读过
- Reviewers: codex (gpt-5.6-sol): unavailable (本会话不再重试); claude (opus-5): unavailable (unknown model); claude (claude-opus-5-5, session): 5 findings（委托会话内一个全新的子代理）
- Nodes: N1 (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16), N2 (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16, T17, T18, T19, T20, T21, T22, T23), N3 (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10)
- 已核对一致的接缝：
  - 安装命令五处逐字相同。
  - Release 资产名与下载根一致。
  - 包内布局（bin、sdk、skills）三方一致，appId 一致。
  - Shell 协议增量在 Rust、TS、Main 三处一致。
  - `app.updateAvailable` 的补推与广播。
  - 环境变量表。
  - upgrade 之后的 skill 刷新。
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR1 | major | spec | skills/alter-browser/SKILL.md:83 | SDK 的 `[alter:notice] … run: alter upgrade` 出现在 agent 脚本的 stderr 里，skill 没说不要照做；agent 自己运行 alter upgrade 会让 install.sh 结束用户正在用的 alter.app，违反用户故事 3「不要擅自替我升级」 | fixed in R2 |
| CR2 | minor | spec | site/index.html:69 | /Applications 不可写时 app 装在 ~/Applications（install.sh），主页、README、skill 的 install.md 只写 /Applications，卸载说明会让用户找不到 app | fixed in R2 |
| CR3 | minor | tests | apps/browser/e2e/packaged.spec.ts:21 | 打包后的 app 跑的是 release 构建的 alterd，默认会检查 GitHub；用例没设 ALTER_RELEASE_API 也没关 update_check，超过 5 秒就会访问外网 | fixed in R2 |
| CR4 | nit | maintainability | apps/browser/src/renderer/src/styles.css:535 | 注释说提示条「与导入向导提示文案同一种样式」，实际是另一套醒目的配色 | fixed in R2 |
| CR5 | nit | maintainability | .dev-flow/changes/add-release-and-cookies/nodes/release-install/design.md:102 | 组件表里 release 工作流那几行（check-bundle 的参数、gh release create 的资产、检查的内容）没有跟上实现 | fixed in R2 |

Review: CHANGES REQUESTED

### Round 2
- Revision: 1
- Scope: alter `git diff 0c35028...HEAD`（整体第 1 轮之后的全部改动：N2/T24、T25 与 N3/T11、T12；不含 .dev-flow/）
- Reviewers: codex (gpt-5.6-sol): unavailable (本会话不再重试); claude (opus-5): unavailable (unknown model); claude (claude-opus-5-5, session): 0 findings（这些提交已经由会话内的新子代理在 N2 第 4、5 轮和 N3 第 3、4 轮逐条审过；本轮由会话自己重读这段 diff 跨节点的部分）
- Nodes: N1 (), N2 (T24, T25), N3 (T11, T12)
- 跨节点的检查：
  - SDK 的提示原文（N2 runtime.ts）现在由 N3 的守卫测试绑定到 SKILL.md。
  - install.sh 的 `~/Applications` 兜底（N2）与主页、README、skill 的安装和卸载说明（N3）一致。
  - 主页与 README 面向人，写 `alter upgrade` 是对的；面向 agent 的 skill 写明只转告提示、升级与重装只由用户来做。
  - 打包 app 的 e2e 不再访问外网。

Review: PASS

## 验证报告
<!-- 每次验证追加一份新的整体报告，放在最近一轮评审之后。以 "- Revision: <父级当前 revision>"、"- Review: Round <最近一轮>" 和 "- Nodes: N1, N2, …"（全部节点，每个都已有自己的 READY 报告）开头。然后是每个仓库的测试 / lint / 类型检查 / 构建结果，每条 "| <repo> | <命令> | PASS |"，成功标准对照表，收敛缺口，最后恰好一行 "Overall: READY" 或 "Overall: NOT READY"。场景行留在节点报告里。 -->

### 整体报告 1
- Revision: 1
- Review: Round 2
- Nodes: N1, N2, N3
- 运行环境：本机 macOS（Apple Silicon），分支 feature/add-release-and-cookies，HEAD 03e3bc9，工作区干净；codex 的沙箱跑不了 Electron 和 Unix socket，所以全部命令都由主会话在本机执行

| Repo | Command | Result |
|---|---|---|
| alter | `cargo test && bun test` | PASS （cargo 238 个全部通过，bun test 20 个全部通过） |
| alter | `cargo fmt --check && cargo clippy --all-targets -- -D warnings && bun run lint` | PASS |
| alter | `bun run typecheck` | PASS |
| alter | `cargo build && bun run build` | PASS |
| alter | `bun run ci:check` | PASS（exit 0；另含 version:check、lint:sh） |
| alter | `bun run dist:dir && bun run e2e` | PASS（109 passed，2.1 分钟） |
| alter | `cargo test -p alter-cli --release --test perf -- --ignored --nocapture` | PASS |
| alter | `cargo test -p alterd --release --test gateway_perf -- --ignored --nocapture` | PASS |
| alter | `cargo test -p alterd --test import_perf -- --ignored --nocapture` | PASS |
| alter | `shellcheck -S style scripts/*.sh && cargo test -p alter-cli --test install_script` | PASS（无输出；19 passed） |

| 成功标准 | 结果 |
|---|---|
| SC1 CLI 冷启动中位数 < 20 ms | PASS：100 次中位数 4.90 ms，p95 8.64 ms |
| SC2 Gateway 单动作 p95 < 5 ms | PASS：1000 次 page.click，p50 0.420 ms，p95 0.462 ms |
| SC3 1000 条 Cookie 导入中位数 < 2000 ms | PASS：5 次中位数 25.8 ms |
| SC4 shellcheck 无输出，--dry-run 场景测试通过 | PASS |
| SC5 ci:check 与 e2e 退出码为 0 | PASS |
| SC6 首次发布可用 | monitored after release：推送第一个 v* tag 后才能验证；另需维护者在 Settings → Pages 把 Source 设为 GitHub Actions |

收敛检查：
- 三个节点各有 READY 报告：N1 报告 1，N2 报告 2，N3 报告 2。
- 整体评审第 1 轮的 5 条发现由 N2/T24、T25 与 N3/T11、T12 收敛，第 2 轮 PASS。
- 节点评审中被接受的发现都写明了理由。
- 改动行的覆盖率：N1 81.5%，N2 88.2%；N3 没有生产代码改动。

Gaps: none.

Overall: READY
