# import-wizard Specification

## Purpose
app 内的导入入口。

## Requirements

### Requirement: 导入向导
系统 SHALL 在侧边栏菜单的「导入…」中提供向导，让用户选择来源浏览器、profile、目标 Space 和数据类别，开始后显示进度与各类计数。

#### Scenario: 从向导导入
- WHEN `ALTER_CHROME_DIR` 下的 `Default` 是标准 fixture profile，app 有 Space `Personal` 与 `Work`，用户打开「导入…」，选 `Chrome`、`Default`、`Work`，勾选书签与历史，点「开始导入」
- THEN 5 秒内向导显示 `书签 3`、`历史 2`，并出现「完成」按钮
- AND `bookmarks` 中 `Work` 有 3 行
