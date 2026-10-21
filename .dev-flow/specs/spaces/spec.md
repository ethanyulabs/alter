# spaces Specification

## Purpose
用户侧的工作区：一组标签与独立的 Chromium partition。

## Requirements

### Requirement: 首次启动建默认 Space
系统 SHALL 在数据库里没有任何未归档的 Space 时，建立一个名为 `Personal`、颜色为 `#6366f1` 的 Space。

#### Scenario: 重启不重复创建
- WHEN 以同一个临时 `ALTER_HOME` 先后两次运行 `alterd run`（第一次以 SIGTERM 结束），第二次启动后连接 `shell.sock`
- THEN 快照中的 `spaces` 恰好 1 项，`name` 为 `Personal`，`id` 与第一次启动时快照里的相同

#### Scenario: 侧边栏显示默认 Space
- WHEN 以空的临时目录为 `ALTER_HOME` 启动 app
- THEN 3 秒内侧边栏底部的 Space 名称显示 `Personal`
- AND 标签列表为空，内容区显示 `没有打开的标签`
