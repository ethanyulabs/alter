---
id: add-core-features
node: N3
title: agent 叠层与接管交还
repos: [alter]
revision: 1
approved_revision: 1
---

# N3 agent 叠层与接管交还

<!-- 流程路线型需求中一个节点（子流程）的需求文档。保留字保持英文。语法见 references/formats.md §3 和 §14 -->

## 范围
- 交付：
  - 页面上的 agent 光标与动作文字。
  - TaskSpace 的 ownership 状态机：`handOff`、用户交还、`takeOverTaskSpace`、用户主动接管。
  - `E_USER_CONTROL` 的强制执行，SDK 的 `waitForControl`。
  - 超时未交还标记 `inactive`。
  - 交给 N6：定型的 SDK 接管 API，skill 照它写。
- 依赖：N2 的 Gateway 分发器与事件推送、每个 TaskSpace 的串行命令队列、CdpMux 按标签取会话，以及快照里的 `taskSpaces` 与标签的 `taskSpaceId`。
- 不做：多个 agent 同时操作同一 TaskSpace 的仲裁（同一 TaskSpace 的命令本来就串行）；语音或通知中心提醒。

术语：「接管横幅」是侧边栏顶部在 ownership 为 `user` 时显示的条，正文 `等待你操作：<TaskSpace 名称>`，带按钮「交还给 Agent」；「停止 Agent」按钮在 ownership 为 `agent` 的 TaskSpace 有页面位于当前 Space 时显示在侧边栏顶部。

## Spec Delta: task-space

### ADDED Requirement: R1 交给用户
系统 SHALL 在 `taskSpace.handOff` 时把该 TaskSpace 的 ownership 设为 `user`，向它的 Gateway 订阅者推送 `taskSpace.ownershipChanged`，并在侧边栏显示接管横幅。

#### Scenario: R1.S1 handOff 改变 ownership 并推送事件
- WHEN 客户端 A 创建了 TaskSpace（id 3），客户端 B 已 `taskSpace.resume {"id":3}`，A 调用 `taskSpace.handOff {"id":3}`
- THEN A 收到 `{}` 响应
- AND B 收到通知 `taskSpace.ownershipChanged`，参数为 `{"id":3,"ownership":"user","reason":"handOff"}`
- AND 新快照的 `taskSpaces` 中 id 3 的 `ownership` 为 `user`

#### Scenario: R1.S2 侧边栏显示接管横幅
- WHEN app 中经 `alter nodejs` 运行 `const t = await taskSpace("login"); await t.handOff()`
- THEN 2 秒内侧边栏出现 `data-testid="handoff-banner"`，文本包含 `等待你操作：login` 与按钮「交还给 Agent」

### ADDED Requirement: R2 用户控制期间拒绝 agent 命令
系统 SHALL 在 TaskSpace 的 ownership 为 `user` 时，对它的 `page.*`、`taskSpace.newPage`、`taskSpace.adopt` 调用返回 `E_USER_CONTROL`，不向 Chromium 发出任何 CDP 命令。

#### Scenario: R2.S1 handOff 之后点击被拒绝
- WHEN TaskSpace 已 `handOff`，客户端调用 `page.click {"id":<id>,"label":"p1","selector":"body"}`
- THEN `error.data.code` 为 `E_USER_CONTROL`
- AND alterd 的回执日志中这次调用的 `ok` 为 `false`、`error_code` 为 `E_USER_CONTROL`

#### Scenario: R2.S2 handOff 之后不能新开页面
- WHEN TaskSpace 已 `handOff`，客户端调用 `taskSpace.newPage {"id":<id>}`
- THEN `error.data.code` 为 `E_USER_CONTROL`

### ADDED Requirement: R3 用户交还给 agent
系统 SHALL 在用户点击「交还给 Agent」后把 ownership 设为 `agent`，推送 `reason` 为 `returned` 的 `taskSpace.ownershipChanged`，并隐藏接管横幅。

#### Scenario: R3.S1 交还事件
- WHEN TaskSpace（id 3）的 ownership 为 `user`，Electron 发送 `{"type":"shell.event.ownershipReturned","taskSpaceId":3}`
- THEN 订阅者收到 `taskSpace.ownershipChanged`，参数为 `{"id":3,"ownership":"agent","reason":"returned"}`
- AND 新快照中 id 3 的 `ownership` 为 `agent`

#### Scenario: R3.S2 在侧边栏交还
- WHEN app 中 TaskSpace `login` 已 `handOff`，接管横幅可见，用户点击「交还给 Agent」
- THEN 1 秒内接管横幅消失
- AND 之后经 `alter nodejs` 运行的 `(await takeOverTaskSpace(<id>)).ownership` 输出 `agent`

### ADDED Requirement: R4 接回
系统 SHALL 在 `taskSpace.takeOver` 时，对 ownership 已回到 `agent` 的 TaskSpace 返回 `{ownership:"agent", userPage}`，`userPage` 是该 TaskSpace 所在 Space 当前激活标签的 Page 标签（未托管时先托管它）；对 ownership 仍为 `user` 的返回 `E_USER_CONTROL`。

#### Scenario: R4.S1 用户已交还
- WHEN TaskSpace 已被交还，所在 Space 的激活标签 A 未被托管，TaskSpace 已有 `p1`，客户端调用 `taskSpace.takeOver {"id":<id>}`
- THEN 响应为 `{ownership:"agent", userPage:"p2"}`，`taskSpace.tabs` 中 A 的 `label` 为 `p2`

#### Scenario: R4.S2 用户尚未交还
- WHEN TaskSpace 已 `handOff` 且用户没有交还，客户端调用 `taskSpace.takeOver`
- THEN `error.data.code` 为 `E_USER_CONTROL`

### ADDED Requirement: R5 用户主动接管
系统 SHALL 在用户点击「停止 Agent」后把该 TaskSpace 的 ownership 设为 `user`，让正在执行的命令以 `E_USER_CONTROL` 结束，并推送 `reason` 为 `userTookControl` 的 `taskSpace.ownershipChanged`。

#### Scenario: R5.S1 中断正在等待的命令
- WHEN 客户端对 `p1` 调用 `page.waitFor {"selector":"#never","timeoutMs":10000}`，1 秒后 Electron 发送 `{"type":"shell.event.userTookControl","taskSpaceId":<id>}`
- THEN `waitFor` 在 Electron 发送事件后 1 秒内返回 `error.data.code` 为 `E_USER_CONTROL`
- AND 订阅者收到 `{"id":<id>,"ownership":"user","reason":"userTookControl"}`

#### Scenario: R5.S2 在侧边栏停止 Agent
- WHEN app 中 TaskSpace `demo` 的 `p1` 在当前 Space，ownership 为 `agent`，用户点击侧边栏的「停止 Agent」
- THEN 1 秒内接管横幅出现
- AND 之后经 `alter nodejs` 运行 `page.click("body")` 时脚本以退出码 1 结束，stderr 包含 `E_USER_CONTROL`

### ADDED Requirement: R6 超时未交还标记为不活跃
系统 SHALL 把 ownership 为 `user` 且持续时间超过阈值（默认 30 分钟，可由 `ALTER_HOME/config.toml` 的 `handoff_timeout_minutes` 配置）的 TaskSpace 的状态设为 `inactive`。

#### Scenario: R6.S1 超时后状态变化
- WHEN 环境变量 `ALTER_HANDOFF_TIMEOUT_MS` 为 300，TaskSpace `login` 已 `handOff`，等待 1 秒后客户端调用 `taskSpace.list`
- THEN `login` 那一项的 `status` 为 `inactive`，`ownership` 仍为 `user`

#### Scenario: R6.S2 交还后恢复活跃
- WHEN 上一个场景中 `status` 为 `inactive` 的 TaskSpace 收到 `shell.event.ownershipReturned`
- THEN `taskSpace.list` 中它的 `status` 为 `active`，`ownership` 为 `agent`

## Spec Delta: sdk

### ADDED Requirement: R7 等待用户交还
系统 SHALL 让 SDK 的 `task.waitForControl({timeoutMs})` 在收到该 TaskSpace 的 `taskSpace.ownershipChanged` 且 `ownership` 为 `agent` 时返回，超时则抛出 `code` 为 `E_TIMEOUT` 的 `AlterError`。

#### Scenario: R7.S1 收到交还后返回
- WHEN 假 Gateway 在 SDK 调用 `task.waitForControl({timeoutMs: 2000})` 后 200 ms 推送 `{"method":"taskSpace.ownershipChanged","params":{"id":3,"ownership":"agent","reason":"returned"}}`
- THEN `waitForControl` 在 1 秒内返回

#### Scenario: R7.S2 超时
- WHEN 假 Gateway 不推送任何事件，SDK 调用 `task.waitForControl({timeoutMs: 300})`
- THEN 抛出 `AlterError`，`code` 为 `E_TIMEOUT`

#### Scenario: R7.S3 E_USER_CONTROL 不被重试
- WHEN 假 Gateway 对 `page.goto` 返回 `E_USER_CONTROL`，SDK 调用 `page.goto("https://example.com")`
- THEN 抛出 `code` 为 `E_USER_CONTROL` 的 `AlterError`，假 Gateway 恰好收到 1 次 `page.goto`

## Spec Delta: agent-overlay
页面上显示 agent 正在做什么。

### ADDED Requirement: R8 动作叠层
系统 SHALL 在 ownership 为 `agent` 的 Page 上执行带 `actionLabel` 的元素动作时，在页面上方显示 agent 光标（位于目标元素中心）与动作文字，叠层不拦截用户的鼠标事件。

#### Scenario: R8.S1 点击时显示光标与文字
- WHEN `p1` 已打开 `/form`，脚本执行 `page.click("text=Submit", {actionLabel: "提交表单"})`
- THEN 1 秒内经 CDP 转发端点读取该页面时，`document.querySelector("alter-agent-overlay").shadowRoot` 中 `[data-role=action]` 的文本为 `提交表单`
- AND `[data-role=cursor]` 的中心与 `Submit` 按钮中心的距离不超过 4 px

#### Scenario: R8.S2 叠层不拦截点击
- WHEN `p1` 上显示着叠层，测试经 CDP 转发端点在叠层覆盖的 `Submit` 按钮中心派发一次鼠标点击
- THEN 页面正文出现 `submitted:`

#### Scenario: R8.S3 用户控制期间隐藏叠层
- WHEN `p1` 上显示着叠层，TaskSpace 执行 `handOff`
- THEN 1 秒内 `alter-agent-overlay` 的计算样式 `display` 为 `none`

## Spec Delta: protocol

### MODIFIED Requirement: 协议类型导出
系统 SHALL 把 Rust 里标记为导出的协议类型生成为 TypeScript 声明文件写入指定目录。

#### Scenario: R9.S1 导出错误码与健康文件类型
- WHEN 执行 `cargo run -p alter-core --bin export-ts -- <空的临时目录>`
- THEN 退出码为 0
- AND 目录中存在 `ErrorCode.ts`，其类型是字符串字面量联合，恰好包含 `E_UNAUTHORIZED`、`E_PROTOCOL_VERSION`、`E_SPACE_NOT_FOUND`、`E_TASK_SPACE_NOT_FOUND`、`E_PAGE_NOT_FOUND`、`E_PAGE_BUDGET`、`E_USER_CONTROL`、`E_AMBIGUOUS_SELECTOR`、`E_NO_MATCH`、`E_TIMEOUT`、`E_DIALOG_OPEN`、`E_IMPORT_SOURCE_NOT_FOUND`、`E_KEYCHAIN_DENIED`、`E_INTERNAL`、`E_TAB_NOT_FOUND`、`E_INVALID_ARGUMENT` 这 16 个字面量
- AND 目录中存在 `HealthFile.ts`，其字段恰好为 `version`、`pid`、`startedAt`、`dbPath`

#### Scenario: R9.S2 导出 Shell Control 消息类型
- WHEN 执行 `cargo run -p alter-core --bin export-ts -- <空的临时目录>`
- THEN 目录中存在 `ShellCommand.ts`，其 `type` 字段的字符串字面量恰好为 `shell.stateChanged`、`shell.navigate`、`shell.error`、`shell.discard`
- AND 目录中存在 `ShellEvent.ts`，其 `type` 字段的字符串字面量恰好为 `shell.event.openRequested`、`shell.event.tabUpdated`、`shell.event.tabActivated`、`shell.event.tabClosed`、`shell.event.spaceCreated`、`shell.event.spaceUpdated`、`shell.event.spaceActivated`、`shell.event.spaceDeleted`、`shell.event.tabPinned`、`shell.event.tabMoved`、`shell.event.cdpReady`、`shell.event.ownershipReturned`、`shell.event.userTookControl`
- AND 目录中存在 `TabView.ts`，其字段包含 `pinned`、`taskSpaceId`、`label`

#### Scenario: R9.S3 导出 Gateway 类型
- WHEN 执行 `cargo run -p alter-core --bin export-ts -- <空的临时目录>`
- THEN 目录中存在 `GatewayFile.ts`，其字段恰好为 `port`、`token`、`cdpPort`、`version`
- AND 目录中存在 `Receipt.ts`，其字段包含 `ok`、`durationMs`、`popups`
- AND 目录中存在 `OwnershipChanged.ts`，其字段恰好为 `id`、`ownership`、`reason`

## 变更记录
- 2026-10-21: 创建。
