# task-space Specification

## Purpose
agent 的任务空间：挂在某个 Space 下，拥有若干带永久标签（`p1`、`p2`…）的 Page。

## Requirements

### Requirement: 创建 TaskSpace
系统 SHALL 在 `taskSpace.create` 时在指定 Space（省略时为当前 Space）下建一个 ownership 为 `agent` 的 TaskSpace，并在该 Space 标签列表末尾为它新建一个不抢激活状态的空白页面 `p1`。

#### Scenario: 在当前 Space 创建
- WHEN 当前 Space 为 `Personal`，其中有激活标签 A，客户端调用 `taskSpace.create {"name":"demo"}`
- THEN 响应为 `{id:<整数>, spaceId:<Personal 的 id>, ownership:"agent", pages:["p1"]}`
- AND 新快照的标签依次为 A、新标签，新标签的 `url` 为 `about:blank`、`taskSpaceId` 等于响应的 `id`、`label` 为 `p1`，A 的 `active` 仍为 `true`

#### Scenario: Space 不存在
- WHEN 客户端调用 `taskSpace.create {"name":"demo","spaceId":"01JZZZZZZZZZZZZZZZZZZZZZZZ"}`
- THEN `error.data.code` 为 `E_SPACE_NOT_FOUND`

#### Scenario: 侧边栏显示 agent 页面
- WHEN app 当前 Space 有激活标签 `Page A`，经 `alter nodejs` 运行 `await taskSpace("demo")`
- THEN 2 秒内侧边栏多出一行，带 `data-testid="agent-badge"` 标记并显示 `p1`，`Page A` 仍处于激活样式
- AND 主进程里存在一个 URL 为 `about:blank` 的 webContents

### Requirement: 恢复与列出 TaskSpace
系统 SHALL 在 `taskSpace.resume` 时按 id 返回已存在 TaskSpace 的当前 Page 标签，在 `taskSpace.list` 时返回全部 TaskSpace。

#### Scenario: 跨连接恢复
- WHEN 客户端 A 调用 `taskSpace.create {"name":"demo"}` 得到 id 7 并断开，客户端 B 调用 `taskSpace.resume {"id":7}`
- THEN 响应为 `{id:7, spaceId:<同一 Space>, ownership:"agent", pages:["p1"]}`

#### Scenario: TaskSpace 不存在
- WHEN 客户端调用 `taskSpace.resume {"id":999}`
- THEN `error.data.code` 为 `E_TASK_SPACE_NOT_FOUND`

#### Scenario: 列出
- WHEN 已创建名为 `a` 与 `b` 的两个 TaskSpace，客户端调用 `taskSpace.list`
- THEN `result.taskSpaces` 依次包含 `name` 为 `a` 与 `b` 的两项，`status` 都为 `active`

### Requirement: 新开 Page 与页面预算
系统 SHALL 在 `taskSpace.newPage` 时为 TaskSpace 新建下一个编号的空白页面，同一 TaskSpace 未关闭的页面数达到 page budget（默认 8）时拒绝。

#### Scenario: 编号递增
- WHEN TaskSpace 已有 `p1`，客户端调用 `taskSpace.newPage {"id":<id>}`
- THEN 响应为 `{label:"p2"}`

#### Scenario: 超出预算
- WHEN TaskSpace 已有 `p1`…`p8` 共 8 个未关闭的页面，客户端调用 `taskSpace.newPage`
- THEN `error.data.code` 为 `E_PAGE_BUDGET`，`error.data.details` 为 `{"budget":8,"open":8}`

#### Scenario: 关闭后标签不复用
- WHEN TaskSpace 有 `p1`、`p2`，客户端调用 `page.close {"id":<id>,"label":"p2"}` 后再调用 `taskSpace.newPage`
- THEN 响应为 `{label:"p3"}`

### Requirement: 托管与归还用户标签
系统 SHALL 在 `taskSpace.adopt` 时把同一 Space 中未被任何 TaskSpace 托管的用户标签纳入该 TaskSpace 并分配下一个 Page 标签，在 `taskSpace.release` 时解除托管但保留标签本身。

#### Scenario: 托管与列出
- WHEN Space `Personal` 有用户标签 A，TaskSpace 已有 `p1`，客户端调用 `taskSpace.adopt {"id":<id>,"tabId":"<A 的 id>"}`，再调用 `taskSpace.tabs {"id":<id>}`
- THEN `adopt` 的响应为 `{label:"p2"}`
- AND `tabs` 的结果包含 `{tabId:<A 的 id>, label:"p2", openedBy:"user"}` 与 `label` 为 `p1`、`openedBy` 为 `agent` 的一项

#### Scenario: 已被另一个 TaskSpace 托管
- WHEN 标签 A 已被 TaskSpace X 托管，客户端对 TaskSpace Y 调用 `adopt` A
- THEN `error.data.code` 为 `E_INVALID_ARGUMENT`

#### Scenario: 跨 Space 托管被拒
- WHEN TaskSpace 属于 `Personal`，标签 B 属于 `Work`，客户端调用 `adopt` B
- THEN `error.data.code` 为 `E_TAB_NOT_FOUND`

#### Scenario: 归还
- WHEN TaskSpace 以 `p2` 托管着用户标签 A，客户端调用 `taskSpace.release {"id":<id>,"label":"p2"}`
- THEN 之后 `taskSpace.tabs` 中 A 那一项没有 `label`，快照里 A 仍在且没有 `taskSpaceId`

### Requirement: 结束 TaskSpace
系统 SHALL 在 `taskSpace.finish` 时关闭该 TaskSpace 中由 agent 打开且不在 `keep` 列表里的页面，解除其余页面的托管，并把状态设为 `finished`。

#### Scenario: 保留指定页面
- WHEN TaskSpace 有 agent 打开的 `p1`、`p2` 和托管的用户标签 `p3`，客户端调用 `taskSpace.finish {"id":<id>,"keep":["p2"]}`
- THEN 响应为 `{closed:["p1"], kept:["p2","p3"]}`
- AND 快照中 `p1` 的标签已不在，`p2` 与 `p3` 的标签仍在且没有 `taskSpaceId`；`taskSpace.list` 中该项 `status` 为 `finished`

### Requirement: 页面被用户关闭
系统 SHALL 在 TaskSpace 的页面对应的标签被用户关闭后，对该 Page 标签的后续 `page.*` 调用返回 `E_PAGE_NOT_FOUND`。

#### Scenario: 用户关掉 agent 页面
- WHEN TaskSpace 的 `p1` 对应标签 T，Electron 发送 `shell.event.tabClosed`（T 的 id），然后客户端调用 `page.info {"id":<id>,"label":"p1"}`
- THEN `error.data.code` 为 `E_PAGE_NOT_FOUND`

### Requirement: 交给用户
系统 SHALL 在 `taskSpace.handOff` 时把该 TaskSpace 的 ownership 设为 `user`，向它的 Gateway 订阅者推送 `taskSpace.ownershipChanged`，并在侧边栏显示接管横幅。

#### Scenario: handOff 改变 ownership 并推送事件
- WHEN 客户端 A 创建了 TaskSpace（id 3），客户端 B 已 `taskSpace.resume {"id":3}`，A 调用 `taskSpace.handOff {"id":3}`
- THEN A 收到 `{}` 响应
- AND B 收到通知 `taskSpace.ownershipChanged`，参数为 `{"id":3,"ownership":"user","reason":"handOff"}`
- AND 新快照的 `taskSpaces` 中 id 3 的 `ownership` 为 `user`

#### Scenario: 侧边栏显示接管横幅
- WHEN app 中经 `alter nodejs` 运行 `const t = await taskSpace("login"); await t.handOff()`
- THEN 2 秒内侧边栏出现 `data-testid="handoff-banner"`，文本包含 `等待你操作：login` 与按钮「交还给 Agent」

### Requirement: 用户控制期间拒绝 agent 命令
系统 SHALL 在 TaskSpace 的 ownership 为 `user` 时，对它的 `page.*`、`taskSpace.newPage`、`taskSpace.adopt` 调用返回 `E_USER_CONTROL`，不向 Chromium 发出任何 CDP 命令。

#### Scenario: handOff 之后点击被拒绝
- WHEN TaskSpace 已 `handOff`，客户端调用 `page.click {"id":<id>,"label":"p1","selector":"body"}`
- THEN `error.data.code` 为 `E_USER_CONTROL`
- AND alterd 的回执日志中这次调用的 `ok` 为 `false`、`error_code` 为 `E_USER_CONTROL`

#### Scenario: handOff 之后不能新开页面
- WHEN TaskSpace 已 `handOff`，客户端调用 `taskSpace.newPage {"id":<id>}`
- THEN `error.data.code` 为 `E_USER_CONTROL`

### Requirement: 用户交还给 agent
系统 SHALL 在用户点击「交还给 Agent」后把 ownership 设为 `agent`，推送 `reason` 为 `returned` 的 `taskSpace.ownershipChanged`，并隐藏接管横幅。

#### Scenario: 交还事件
- WHEN TaskSpace（id 3）的 ownership 为 `user`，Electron 发送 `{"type":"shell.event.ownershipReturned","taskSpaceId":3}`
- THEN 订阅者收到 `taskSpace.ownershipChanged`，参数为 `{"id":3,"ownership":"agent","reason":"returned"}`
- AND 新快照中 id 3 的 `ownership` 为 `agent`

#### Scenario: 在侧边栏交还
- WHEN app 中 TaskSpace `login` 已 `handOff`，接管横幅可见，用户点击「交还给 Agent」
- THEN 1 秒内接管横幅消失
- AND 之后经 `alter nodejs` 运行的 `(await takeOverTaskSpace(<id>)).ownership` 输出 `agent`

### Requirement: 接回
系统 SHALL 在 `taskSpace.takeOver` 时，对 ownership 已回到 `agent` 的 TaskSpace 返回 `{ownership:"agent", userPage}`，`userPage` 是该 TaskSpace 所在 Space 当前激活标签的 Page 标签（未托管时先托管它）；对 ownership 仍为 `user` 的返回 `E_USER_CONTROL`。

#### Scenario: 用户已交还
- WHEN TaskSpace 已被交还，所在 Space 的激活标签 A 未被托管，TaskSpace 已有 `p1`，客户端调用 `taskSpace.takeOver {"id":<id>}`
- THEN 响应为 `{ownership:"agent", userPage:"p2"}`，`taskSpace.tabs` 中 A 的 `label` 为 `p2`

#### Scenario: 用户尚未交还
- WHEN TaskSpace 已 `handOff` 且用户没有交还，客户端调用 `taskSpace.takeOver`
- THEN `error.data.code` 为 `E_USER_CONTROL`

### Requirement: 用户主动接管
系统 SHALL 在用户点击「停止 Agent」后把该 TaskSpace 的 ownership 设为 `user`，让正在执行的命令以 `E_USER_CONTROL` 结束，并推送 `reason` 为 `userTookControl` 的 `taskSpace.ownershipChanged`。

#### Scenario: 中断正在等待的命令
- WHEN 客户端对 `p1` 调用 `page.waitFor {"selector":"#never","timeoutMs":10000}`，1 秒后 Electron 发送 `{"type":"shell.event.userTookControl","taskSpaceId":<id>}`
- THEN `waitFor` 在 Electron 发送事件后 1 秒内返回 `error.data.code` 为 `E_USER_CONTROL`
- AND 订阅者收到 `{"id":<id>,"ownership":"user","reason":"userTookControl"}`

#### Scenario: 在侧边栏停止 Agent
- WHEN app 中 TaskSpace `demo` 的 `p1` 在当前 Space，ownership 为 `agent`，用户点击侧边栏的「停止 Agent」
- THEN 1 秒内接管横幅出现
- AND 之后经 `alter nodejs` 运行 `page.click("body")` 时脚本以退出码 1 结束，stderr 包含 `E_USER_CONTROL`

### Requirement: 超时未交还标记为不活跃
系统 SHALL 把 ownership 为 `user` 且持续时间超过阈值（默认 30 分钟，可由 `ALTER_HOME/config.toml` 的 `handoff_timeout_minutes` 配置）的 TaskSpace 的状态设为 `inactive`。

#### Scenario: 超时后状态变化
- WHEN 环境变量 `ALTER_HANDOFF_TIMEOUT_MS` 为 300，TaskSpace `login` 已 `handOff`，等待 1 秒后客户端调用 `taskSpace.list`
- THEN `login` 那一项的 `status` 为 `inactive`，`ownership` 仍为 `user`

#### Scenario: 交还后恢复活跃
- WHEN 上一个场景中 `status` 为 `inactive` 的 TaskSpace 收到 `shell.event.ownershipReturned`
- THEN `taskSpace.list` 中它的 `status` 为 `active`，`ownership` 为 `agent`
