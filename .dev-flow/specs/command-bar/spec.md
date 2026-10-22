# command-bar Specification

## Purpose
一个输入框完成打开网址、搜索、切换标签与 Space。

## Requirements

### Requirement: 打开与关闭命令栏
系统 SHALL 在 Cmd+T 时打开命令栏并聚焦输入框，此时内容区不显示页面；按 Esc 关闭命令栏，恢复显示原来的激活标签。

#### Scenario: 打开与关闭
- WHEN app 激活标签显示本地测试页 `/a`，在主进程点击菜单项 `tab.new`
- THEN 1 秒内 `command-bar` 可见且 Renderer 的 `document.activeElement` 是它的输入框，窗口上挂载的 WebContentsView 数量为 0
- AND 按 Esc 后 1 秒内 `command-bar` 不可见，窗口上挂载的视图的 URL 为 `/a` 的地址

### Requirement: 搜索结果
系统 SHALL 对命令栏的输入，按「标签、Space、书签、历史」的顺序返回标题或 URL 中包含输入（不区分大小写）的项，合计不超过 8 项，并在最后附加一项「打开或搜索该输入」。

#### Scenario: 结果的来源与顺序
- WHEN Space `Personal` 有标签 `Docs Home`（`https://docs.example.com/`），存在名为 `Docs` 的 Space，`Personal` 的历史里有 `Docs Archive`（`https://docs.example.com/old`），客户端发送 `{"type":"shell.event.searchRequested","requestId":1,"query":"docs"}`
- THEN 客户端收到 `shell.searchResults`，`requestId` 为 1，`items` 依次为 `kind` 为 `tab`（`Docs Home`）、`space`（`Docs`）、`history`（`Docs Archive`）、`open`（`input` 为 `docs`）的 4 项

#### Scenario: 已打开的 URL 不重复出现在历史结果里
- WHEN 标签 `Docs Home` 的 URL 也在历史里，客户端搜索 `docs home`
- THEN `items` 中 `kind` 为 `history` 的项没有 `https://docs.example.com/`

#### Scenario: 最多 8 项
- WHEN 当前 Space 有 12 个标题含 `Page` 的标签，客户端搜索 `page`
- THEN `items` 共 9 项：前 8 项的 `kind` 为 `tab`，最后一项的 `kind` 为 `open`

#### Scenario: 空输入
- WHEN 客户端搜索 `"  "`
- THEN `items` 为空数组

#### Scenario: 书签排在历史之前
- WHEN 当前 Space 的书签里有 `Rust`（`https://rust-lang.org/`），历史里有 `Rust Blog`（`https://blog.rust-lang.org/`），客户端搜索 `rust`
- THEN `items` 依次为 `kind` 为 `bookmark`（`Rust`）、`history`（`Rust Blog`）、`open` 的 3 项

### Requirement: 执行结果项
系统 SHALL 在用户回车或点击某个结果项时执行它：标签项切到其 Space 并激活该标签，Space 项切到该 Space，历史项与「打开或搜索」项在当前 Space 新建标签打开。

#### Scenario: 切到另一个 Space 的标签
- WHEN app 的 `Personal` 有标签 `Page A`，当前 Space 为 `Work`，用户按 Cmd+T（菜单项 `tab.new`），输入 `Page A`，回车
- THEN 1 秒内底部的 Space 名称为 `Personal`，`Page A` 处于激活样式，命令栏不可见

#### Scenario: 输入网址打开新标签
- WHEN app 当前 Space 有 `Page A`，用户打开命令栏，输入 `127.0.0.1:<port>/b`，回车
- THEN 2 秒内侧边栏依次为 `Page A`、`Page B`，`Page B` 处于激活样式

#### Scenario: 打开历史项
- WHEN app 当前 Space 访问过 `/c` 后关闭了它，用户打开命令栏，输入 `Page C`，用方向键选中 `data-kind="history"` 的项并回车
- THEN 2 秒内侧边栏出现 `Page C` 且处于激活样式
