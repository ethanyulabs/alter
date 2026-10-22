# peek Specification

## Purpose
不离开当前页面地预览链接。

## Requirements

### Requirement: Shift+点击打开 Peek
系统 SHALL 在用户按住 Shift 点击页面中的链接时，在当前页面之上的浮层里加载该链接，不新建标签。

#### Scenario: 打开 Peek
- WHEN 激活标签显示本地测试页 `/links`（其中有指向 `/b` 的链接 `Open B`），在该页面上按住 Shift 点击 `Open B`
- THEN 2 秒内主进程中有一个 URL 为 `/b` 的地址、`data-peek` 为真的视图挂在窗口上，其宽高分别为内容区的 80%（误差 1 px）
- AND 侧边栏标签列表仍然恰好 1 行

### Requirement: 关闭 Peek 或转为标签
系统 SHALL 在用户按 Esc 或点击浮层外的区域时关闭 Peek，在用户点「在新标签打开」时关闭 Peek 并在当前 Space 新建标签打开 Peek 当前的 URL。

#### Scenario: Esc 关闭
- WHEN Peek 正显示 `/b`，在 Renderer 按 Esc
- THEN 1 秒内窗口上不再挂载 Peek 视图，主进程中没有 URL 为 `/b` 的地址的 webContents

#### Scenario: 转为标签
- WHEN Peek 正显示 `/b`，用户点击「在新标签打开」
- THEN 2 秒内侧边栏依次为 `Links`、`Page B`，`Page B` 处于激活样式，Peek 已关闭
