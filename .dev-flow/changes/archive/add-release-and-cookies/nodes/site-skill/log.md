---
change: add-release-and-cookies
---

# 执行日志：主页与 agent skill

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

### T1 英文主页与共用样式
- Revision: 1
- RED: `bun test packages/sdk/tests/distribution.test.ts` → FAIL: ENOENT site/index.html (R1.S1)
- GREEN: `bun test packages/sdk/tests/distribution.test.ts` → 2 pass (R1.S1)
- RED: 同上 → FAIL: ENOENT scandir site（还没有站点目录） (R2.S1)
- GREEN: 同上 → 2 pass (R2.S1)
- VERIFY: 变异「样式表改为 https://cdn.example.com/x.css」→ R2.S1 FAIL（引用了外部资源）；变异「安装命令的 -fsSL 改为 -sSL」→ R1.S1 FAIL；恢复后 2 pass。widen.sh 各项 exit 都为 0。页面在手机宽度下不横向滚动，这一点按任务备注只是人工检查，没有写成场景

### T2 中文主页
- Revision: 1
- RED: `bun test packages/sdk/tests/distribution.test.ts` → FAIL: ENOENT site/zh/index.html (R1.S2)
- GREEN: `bun test packages/sdk/tests/distribution.test.ts` → 3 pass (R1.S2)
- VERIFY: R2.S1 自动覆盖了新页面，它的 `../style.css`、`../favicon.svg` 都能解析；widen.sh 各项 exit 都为 0

### T3 skill 正文与 references
- Revision: 1
- RED: `bun test packages/sdk/tests/distribution.test.ts` → FAIL: RC N3 R4.S1（N2 的占位 SKILL.md 没有 bash 代码块，references 也不存在） (R4.S1)
- GREEN: `bun test packages/sdk/tests/distribution.test.ts` → 5 pass (R4.S1)
- RED: 同上 → FAIL: RC N3 R6.S1（references/handoff.md 不存在）；写好后第一次仍 FAIL，因为原文是「Do not retry.」，场景要求的是 `do not retry`，已改写措辞 (R6.S1)
- GREEN: 同上 → 5 pass (R6.S1)
- VERIFY: 文档里的 SDK 签名对照 packages/sdk/src/page.ts、task-space.ts 写成；上传文件写的是 `setInputFiles`（`waitForFileChooser` 仍推迟）；install.md 里 bun 的报错按 CLI 实际的「找不到 bun」写。widen.sh 各项 exit 都为 0；N2 的 `check-bundle.sh` 只检查 SKILL.md 存在，不受影响

### T4 references/api.md 与 SDK 公开 API 双向一致
- Revision: 1
- RED: `bun test packages/sdk/tests/distribution.test.ts` → FAIL: RC N3 R5.S1，undocumented 列出 48 个调用名（api.md 只有占位标题） (R5.S1)
- GREEN: `bun test packages/sdk/tests/distribution.test.ts` → 7 pass (R5.S1)
- RED: 同上 → FAIL: RC N3 R5.S2（文档缺得太多，差集不止 page.zzzProbe 这一项） (R5.S2)
- GREEN: 同上 → 7 pass（差集恰好为 ["page.zzzProbe"]） (R5.S2)
- VERIFY: TS 的 private 成员（Page.call、Page.action、TaskSpace.call）在运行时也挂在原型上，所以测试从源码识别 `private` 声明并排除；N3 design.md 的「公开 API 集合的求法」已同步。tabs() 的字段、taskSpace.cdp 的作用范围对照 infra/api.md 写成。widen.sh 各项 exit 都为 0

### T5 README 的安装一节，以及四处安装命令一致
- Revision: 1
- RED: `bun test packages/sdk/tests/distribution.test.ts` → FAIL: RC N3 R3.S1 — README.md 里没有安装命令 (R3.S1)
- GREEN: `bun test packages/sdk/tests/distribution.test.ts` → 8 pass (R3.S1)
- VERIFY: 变异「README 里的 `| bash` 改成 `| sh`」→ R3.S1 FAIL，恢复后 8 pass。README 新增「安装」（安装命令、`alter upgrade`、`alter skill install`、卸载），「现状」改成描述已有的功能，「开发」加上 `bun run dist` 与发布步骤，目录表加上 skills/、site/、scripts/。install.sh 头部的用法注释本来就与常量一致。widen.sh 各项 exit 都为 0

### T6 真实执行 SKILL.md 的快速上手示例
- Revision: 1
- RED: `bun run e2e --grep "RC N3 R7"` → FAIL: 退出码 1，stderr「AlterError: E_PAGE_NOT_FOUND: TaskSpace 1 没有页面 main」。T3 写的示例用了 `ts.page("main")`，并声称「对新标签第一次调用就会开一个标签页」，这是错的：TaskSpace 只自带 `p1`，之后的页面由 newPage() 分配 (R7.S1)
- 中间一次失败：改正示例后用例超时 30 秒。原因是 `spawnSync` 阻塞了测试进程的事件循环，而本地测试服务器在同一个进程里，脚本里的 goto 等不到响应。改为异步 spawn
- GREEN: `bun run e2e --grep "RC N3 R7"` → ✓ (1.9s) (R7.S1)
- VERIFY: SKILL.md、references/api.md、references/handoff.md、site 两个页面里的示例都改为 `p1`，并写明 newPage() 依次分配 p2、p3…；`bun test packages/sdk/tests/distribution.test.ts` → 8 pass；widen.sh 各项 exit 都为 0

### T7 GitHub Pages 发布工作流
- Revision: 1
- VERIFY: `ruby -ryaml -e 'YAML.load_file(".github/workflows/pages.yml")'` → yaml-ok；`bun run lint` 通过。push 到 main 且改了 site/** 时触发，也可以手动触发；权限 pages: write、id-token: write；action 钉到 commit SHA：configure-pages v5 → 983d7736…、upload-pages-artifact v3 → 56afc609…、deploy-pages v4 → d6db9016…，checkout 与 release 工作流一致。工作流只能在 GitHub 上验证，对应父级 SC6

### T8 节点门禁
- Revision: 1
- VERIFY: `bun run ci:check` exit 0（cargo 238 passed / 0 failed，bun test 19 pass，lint、typecheck、version:check、lint:sh 都通过）；`bun run e2e --grep "RC N3"` → 1 passed
- VERIFY: `bun test --coverage`：本节点没有改动 packages/sdk/src（只新增测试与文档），覆盖率仍是原来的水平（All files 行覆盖率 82.53%）；page.ts 等的行由 e2e 覆盖，Electron 里没有行覆盖率工具

### T9 收敛第 1 轮：按 SDK 的实际行为改正文档；front matter 用 YAML 解析；references 改为链接；去掉 metadata.version
- Revision: 1
- RED: `bun test packages/sdk/tests/distribution.test.ts` → FAIL: RC N3 R4.S1 — 「正文应当以 Markdown 链接指向 references/api.md」（正文用的是代码样式的路径） (R4.S1)
- GREEN: `bun test packages/sdk/tests/distribution.test.ts` → 8 pass (R4.S1)
- VERIFY: 变异「front matter 里加一行非法 YAML `bad: [unclosed`」→ R4.S1 FAIL（Bun.YAML 解析失败）；变异「把 api.md 的链接改回代码路径」→ FAIL；恢复后 8 pass。`bun run e2e --grep "RC N3 R7"` → ✓。逐条对照代码改正了文档：
  - waitFor：`page.rs:347` 只返回 `matched: true`，`actions.rs:700` 用 querySelector。
  - listTaskSpaces：`TaskSpaceSummary` 没有 pages，包括已结束的。
  - screenshot：要求绝对路径，`page.rs:305`。
  - waitForEvent：超时抛普通 Error，`page.ts:147`。
  - 原始 CDP：只要有用户控制的 TaskSpace 就拒绝，`ownership.rs:32`。
  - SKILL.md 与 handoff.md 关于 E_USER_CONTROL 的说法已统一。
  - design.md 的示例改为 `p1`。
  - widen.sh 各项 exit 都为 0。

### T10 收紧 R2.S1：路径必须在 site/ 内、单双引号都认、扫描 CSS 的 url() 与 @import
- Revision: 1
- RED (mutation: style.css 开头加一条 @import 外部地址 https://fonts.example/x.css): `bun test packages/sdk/tests/distribution.test.ts` → FAIL: style.css 引用了外部资源 https://fonts.example/x.css (R2.S1)
- GREEN: 恢复后 `bun test packages/sdk/tests/distribution.test.ts` → 8 pass (R2.S1)
- VERIFY: 第二个变异「页面里用单引号写 `src='../README.md'`」→ FAIL（../README.md 不在 site/ 里），恢复后 PASS。收紧之前的测试对这两个变异都测不出来（CR10）。widen.sh 各项 exit 都为 0

### T11 收敛整体第 1 轮：看到更新提示只转告用户；文档补上 ~/Applications 的兜底位置
- Revision: 1
- VERIFY: 新增守卫测试「看到更新提示时只转告用户，不自己升级」，不对应场景。改文档之前运行 → FAIL，SKILL.md 里没有 `[alter:notice]`；改文档之后 → 9 pass。变异「把 Never run 改成 Feel free to run」→ FAIL，恢复后 PASS。
- VERIFY: 文档改动：
  - SKILL.md 的 Rules 加了一条：看到 `[alter:notice]` 只转告用户、继续手上的任务，绝不自己运行 `alter upgrade` 或安装脚本。
  - install.md 写明只有用户运行 `alter upgrade`，以及它会退出并重开 alter。
  - SKILL.md 剩下三处对 references 的代码路径改为 Markdown 链接，这也是 N3 第 2 轮接受的 CR11。
  - 两个主页、README、install.md 的安装与卸载说明都写上了 `~/Applications` 的兜底位置。
- VERIFY: widen.sh 各项 exit 都为 0

### T12 收敛第 3 轮：错误表改为请用户重装；守卫测试绑定 runtime.ts 的提示原文；design.md 同步
- Revision: 1
- VERIFY: 收紧后的守卫测试在改 install.md 之前运行 → FAIL，「references/install.md 让 agent 自己重装或升级」，即错误表里 Reinstall with the command above 那一行；改为 Ask the user to reinstall alter 之后 → 9 pass
- VERIFY: 变异测试：
  - 变异「runtime.ts 的提示改成 has been released」→ FAIL：SKILL.md 应当原样引用提示。第一版只绑定了 `${` 之前的前缀，这个变异测不出来，已改为把整句提示转成正则比对。
  - 变异「install.md 改回 Reinstall with …」→ FAIL。
  - 恢复后 9 pass。
- VERIFY: N3 design.md 的 Rules 加上「看到 [alter:notice] 只转告用户」，install.md 的描述加上「升级与重装只由用户来做」；widen.sh 各项 exit 都为 0

## 决策与偏差

- decided: 起点状态 — N3 从 HEAD 7e58339 开始（N1、N2 done），工作区干净

- design: 公开 API 的定义补上「TS 源码里声明为 private 的不算」，因为 private 方法在运行时也挂在原型上，只靠 `_` 前缀排除会把它们也算进来。N3 design.md 已同步
- decided: `taskSpace.setUserPage` 是公开方法（没有 private），文档里写明「内部使用，由 takeOverTaskSpace 设置」，不为了迎合文档去改 SDK — from: tasks.md 全节点约定「不改 SDK 的行为来迎合文档」

- assumed: README 里主页的地址写的是 GitHub Pages 的默认项目地址 `https://ethanyulabs.github.io/alter/` — alternatives: 自定义域名（需求范围外） — reverts: 改 README 里的一行

- decided: 按 SDK 的实际行为改正 skill 与主页里的示例（`ts.page("p1")`），不改 SDK — from: tasks.md 全节点约定「文档与 SDK 不一致时改文档」，以及 task-space 活规格「taskSpace.create 新建空白页面 p1」

- follow-up: 维护者需要在仓库 Settings → Pages 把 Source 设为 GitHub Actions，pages 工作流才能发布主页
- decided: pages 工作流的 action 也钉到 commit SHA — from: N2 评审 CR10 对 release 工作流的同类处理

- design: SKILL.md 不再写 `metadata.version`（CR8），skill 的版本以 app 包为准，由 `alter skill install` 写入的 `.alter-skill` 记录。不把它加进 version-check.sh，因为那会偏离 R3「三处版本号」。N3 design.md 已同步

- decided: 不改 SDK 打出的提示文案。R13.S1 写死了 `[alter:notice] alter <version> is available, run: alter upgrade`，改文案要走 update。改为在 skill 里约束 agent 只转告用户，这样已经能防止 agent 擅自升级 — from: 整体评审第 1 轮 CR1，用户故事 3「不要擅自替我升级」

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
- Scope: alter `git diff 7e58339...HEAD`（N3 的全部改动，不含 .dev-flow/；没有未提交的改动）
- Reviewers: codex (gpt-5.6-sol): unavailable (N2 第 2 轮没有作答，本会话不再重试); claude (opus-5): unavailable (unknown model); claude (claude-opus-5-5, session): 11 findings（配置的评审者都不可用，按 reviewer 为 null 的方式，委托会话内一个全新的子代理独立评审）
- Tasks: T1-T8
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR1 | major | spec | skills/alter-browser/references/api.md:131 | `page.waitFor` 实际只返回 `{matched: true}`，超时抛 `E_TIMEOUT`；`{selector}` 只认原生 CSS（不认 `@e12` 和 `text=`）；`{ms}` 超过 timeoutMs 立即失败。文档写成「Returns whether it matched」，SKILL.md 还在 `@e` 引用之后建议用 `waitFor({selector})` | fixed in R2 |
| CR2 | major | spec | skills/alter-browser/references/api.md:26 | `listTaskSpaces()` 返回 `{id, name, spaceId, ownership, status}`，包括已结束的，文档写成「open TaskSpaces」并带 `pages` | fixed in R2 |
| CR3 | minor | spec | skills/alter-browser/references/api.md:97 | `screenshot` 的 path 必须是绝对路径，相对路径回 `E_INVALID_ARGUMENT`，文档没写 | fixed in R2 |
| CR4 | minor | spec | skills/alter-browser/references/api.md:148 | `waitForEvent` 超时抛的是普通 Error（不是 AlterError、没有 code），默认 30 秒，与开头「errors are AlterError」矛盾 | fixed in R2 |
| CR5 | minor | spec | skills/alter-browser/references/api.md:72 | 原始 CDP（page.cdp / taskSpace.cdp）只要有任何 TaskSpace 由用户控制就回 `E_USER_CONTROL`，与 SKILL.md 对这个错误码的解释（你的 TaskSpace 被接管）不同，agent 可能因此误判甚至循环 | fixed in R2 |
| CR6 | minor | spec | skills/alter-browser/SKILL.md:56 | SKILL.md 说只有主动 handOff 时才 waitForControl，handoff.md（R6）说收到 E_USER_CONTROL 后要等用户交还，两处说法相反 | fixed in R2 |
| CR7 | minor | maintainability | .dev-flow/changes/add-release-and-cookies/nodes/site-skill/design.md:96 | design.md 接口契约里的最小示例还是 `ts.page("main")`，没有随 T6 同步 | fixed in R2 |
| CR8 | minor | spec | skills/alter-browser/SKILL.md:4 | `metadata.version` 写死为 0.1.0，没有任何机制让它跟着发布版本走，与设计「等于发布版本号」不符 | fixed in R2 |
| CR9 | minor | tests | packages/sdk/tests/distribution.test.ts:98 | R4.S1 的「front matter 能被解析」用手写正则：description 里出现非法 YAML 时测试照样通过 | fixed in R2 |
| CR10 | minor | tests | packages/sdk/tests/distribution.test.ts:80 | R2.S1 不检查解析后的路径是否在 site/ 内（如 `../README.md`），只匹配双引号属性，不扫 style.css 里的 url() / @import | fixed in R2 |
| CR11 | nit | spec | skills/alter-browser/SKILL.md:15 | 指向 references 的是代码样式的路径，不是 Markdown 链接，测试也只做子串匹配 | accepted: 开头一段与 Rules 已改为 Markdown 链接，Hand off、Playwright、Troubleshooting 三节仍是代码样式的路径，agent 照样能找到文件 |

Review: CHANGES REQUESTED

### Round 2
- Revision: 1
- Scope: alter `git diff 34d1418...HEAD`（T9、T10 的修复）
- Reviewers: codex (gpt-5.6-sol): unavailable (本会话不再重试); claude (opus-5): unavailable (unknown model); claude (claude-opus-5-5, session): 1 finding（委托会话内一个全新的子代理；它逐条对照 packages/sdk/src、crates/alterd/src/gateway 与 tabs 核实：CR1–CR10 已修复，CR11 部分修复）
- Tasks: T9, T10
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR12 | nit | tests | packages/sdk/tests/distribution.test.ts:92 | R2.S1 不扫描内联 style 里的 @import / url()、元素的 style 属性、srcset，以及不带引号的属性；现在的两个页面都没有这样写 | accepted: 页面是手写的静态 HTML，只有一段内联脚本、没有内联样式；以后要用这些写法时再收紧测试 |

Review: PASS

### Round 3
- Revision: 1
- Scope: alter `git show 2b4b47b`（T11，整体评审第 1 轮的 CR1、CR2）
- Reviewers: codex (gpt-5.6-sol): unavailable (本会话不再重试); claude (opus-5): unavailable (unknown model); claude (claude-opus-5-5, session): 3 findings（委托会话内一个全新的子代理；CR1 的主路径与 CR2 已修复，核对了 install.sh:93 的兜底与 upgrade.rs 的原地升级）
- Tasks: T11
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR13 | minor | spec | skills/alter-browser/references/install.md:44 | 错误表里「找不到 bun」的处理是 Reinstall with the command above，与 SKILL.md「绝不自己运行安装脚本」矛盾，agent 照做会退出用户正在用的 alter | fixed in R4 |
| CR14 | nit | tests | packages/sdk/tests/distribution.test.ts:148 | 守卫测试只查几个子串：没有把 SKILL.md 里的提示前缀与 runtime.ts 实际打出的绑定，也查不出 references 里让 agent 自己重装或升级的措辞 | fixed in R4 |
| CR15 | nit | maintainability | .dev-flow/changes/add-release-and-cookies/nodes/site-skill/design.md:98 | 接口契约里 SKILL.md 的 Rules 与 install.md 的内容描述没有写进「只转告更新提示、只由用户升级」这条约束 | fixed in R4 |

Review: CHANGES REQUESTED

### Round 4
- Revision: 1
- Scope: alter `git show ec974c1`（T12）
- Reviewers: codex (gpt-5.6-sol): unavailable (本会话不再重试); claude (opus-5): unavailable (unknown model); claude (claude-opus-5-5, session): 1 finding（委托会话内一个全新的子代理，用 bun 实测了生成的正则：`\[alter:notice\] alter \S+ is available, run: alter upgrade`）
- Tasks: T12
| ID | Severity | Dimension | Location | Finding | Action |
|---|---|---|---|---|---|
| CR16 | nit | tests | packages/sdk/tests/distribution.test.ts:159 | 禁止祈使句的正则只挡住行首或单元格开头的 reinstall / re-run the installer，「Run the installer again」「Please reinstall」「Run `alter upgrade`」这类同义写法照样能通过；SKILL.md 的禁令删掉「or the installer」半句，测试也不会失败 | accepted: 文字守卫无法穷举同义改写，它只负责挡住原样回退；skill 现在的内容已经由三轮评审逐句核对，以后改 skill 的评审照样要看这条规则 |

Review: PASS

## 验证报告
<!-- 每次验证追加一份新报告，放在最近一轮评审之后。以 "- Revision: <当前 revision>" 和 "- Review: Round <最近一轮>" 开头。由验证步骤填写：本仓库的测试结果，每个场景一行 "| R<n>.S<m> | <path::test> | PASS |"，成功标准对照表，收敛缺口，fix 类型还要写 "Fix: verified | partial | failed"，最后恰好一行 "Overall: READY" 或 "Overall: NOT READY"。 -->

### 节点报告 1
- Revision: 1
- Review: Round 2
- 运行环境：本机 macOS（Apple Silicon），分支 feature/add-release-and-cookies，HEAD d0b4b3e，工作区干净

| Repo | Command | Result |
|---|---|---|
| alter | `bun run ci:check` | PASS（exit 0；cargo 238 passed / 0 failed，bun test 19 pass，lint、typecheck、version:check、lint:sh） |
| alter | `bun run e2e --grep "RC N3"` | PASS（1 passed） |

| Scenario | Test | Result |
|---|---|---|
| R1.S1 | packages/sdk/tests/distribution.test.ts::RC N3 R1.S1 英文主页给出安装方式 | PASS |
| R1.S2 | packages/sdk/tests/distribution.test.ts::RC N3 R1.S2 中文主页给出安装方式 | PASS |
| R2.S1 | packages/sdk/tests/distribution.test.ts::RC N3 R2.S1 相对引用都能解析，不从外部加载资源 | PASS |
| R3.S1 | packages/sdk/tests/distribution.test.ts::RC N3 R3.S1 四处安装命令逐字相同 | PASS |
| R4.S1 | packages/sdk/tests/distribution.test.ts::RC N3 R4.S1 front matter 与链接 | PASS |
| R5.S1 | packages/sdk/tests/distribution.test.ts::RC N3 R5.S1 文档与 SDK 导出双向一致 | PASS |
| R5.S2 | packages/sdk/tests/distribution.test.ts::RC N3 R5.S2 SDK 新增方法而文档没写时失败 | PASS |
| R6.S1 | packages/sdk/tests/distribution.test.ts::RC N3 R6.S1 交接控制权的规则写明 | PASS |
| R7.S1 | apps/browser/e2e/skill.spec.ts::RC N3 R7.S1 对本地页面跑 SKILL.md 的快速上手示例 | PASS |

收敛检查：
- 对照 N3 requirement.md 与 design.md 逐条核对，没有 missing / partial / contradicts / unrequested。
- 文档里写的 SDK 行为已对照 packages/sdk/src 与 crates/alterd/src 改正（评审第 1 轮的 CR1–CR8）。
- 本节点没有生产代码改动，覆盖率不适用；SDK 的覆盖率保持原来的水平。
- 父级 SC6（Pages 与首次发布）是发布后监控项；维护者还需要在 Settings → Pages 把 Source 设为 GitHub Actions，已记为 follow-up。

Gaps: none.

Overall: READY

### 节点报告 2
- Revision: 1
- Review: Round 4
- 运行环境：本机 macOS（Apple Silicon），分支 feature/add-release-and-cookies，HEAD 63f7111；`bun run dist:dir` 用当前代码重新打了包

| Repo | Command | Result |
|---|---|---|
| alter | `bun run ci:check` | PASS（exit 0；cargo 238 passed / 0 failed，bun test 20 pass，lint、typecheck、version:check、lint:sh） |
| alter | `bun run dist:dir && bun run e2e` | PASS（109 passed，含打包后 app 与 skill 示例的用例） |

| Scenario | Test | Result |
|---|---|---|
| R1.S1 | packages/sdk/tests/distribution.test.ts::RC N3 R1.S1 英文主页给出安装方式 | PASS |
| R1.S2 | packages/sdk/tests/distribution.test.ts::RC N3 R1.S2 中文主页给出安装方式 | PASS |
| R2.S1 | packages/sdk/tests/distribution.test.ts::RC N3 R2.S1 相对引用都能解析，不从外部加载资源 | PASS |
| R3.S1 | packages/sdk/tests/distribution.test.ts::RC N3 R3.S1 四处安装命令逐字相同 | PASS |
| R4.S1 | packages/sdk/tests/distribution.test.ts::RC N3 R4.S1 front matter 与链接 | PASS |
| R5.S1 | packages/sdk/tests/distribution.test.ts::RC N3 R5.S1 文档与 SDK 导出双向一致 | PASS |
| R5.S2 | packages/sdk/tests/distribution.test.ts::RC N3 R5.S2 SDK 新增方法而文档没写时失败 | PASS |
| R6.S1 | packages/sdk/tests/distribution.test.ts::RC N3 R6.S1 交接控制权的规则写明 | PASS |
| R7.S1 | apps/browser/e2e/skill.spec.ts::RC N3 R7.S1 对本地页面跑 SKILL.md 的快速上手示例 | PASS |

收敛检查：整体评审第 1 轮的 CR1、CR2 由 T11、T12 收敛（第 3、4 轮）：skill 写明看到 `[alter:notice]` 只转告用户，升级与重装只由用户来做；安装与卸载说明写上了 `~/Applications`。守卫测试绑定 SDK 实际打出的提示原文。没有生产代码改动。

Gaps: none.

Overall: READY
