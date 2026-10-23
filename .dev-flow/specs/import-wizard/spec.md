# import-wizard Specification

## Purpose
app 内的导入入口。

## Requirements

### Requirement: 导入向导
系统 SHALL 在侧边栏菜单的「导入…」中提供向导，让用户选择来源浏览器、profile、目标 Space 和数据类别（书签、历史、Cookie，Cookie 默认勾选），开始后显示进度与各类计数；部分完成时显示原因。

#### Scenario: 从向导导入
- WHEN `ALTER_CHROME_DIR` 下的 `Default` 是标准 fixture profile，app 有 Space `Personal` 与 `Work`，用户打开「导入…」，选 `Chrome`、`Default`、`Work`，勾选书签与历史、取消勾选 Cookie，点「开始导入」
- THEN 5 秒内向导显示 `书签 3`、`历史 2`，并出现「完成」按钮
- AND `bookmarks` 中 `Work` 有 3 行

#### Scenario: Cookie 可选且默认勾选
- WHEN 用户打开「导入…」
- THEN 「Cookie」复选框可用且已勾选，旁边有说明文字「macOS 会请求钥匙串授权」

#### Scenario: 拒绝授权时显示原因
- WHEN alterd 带 `ALTER_KEYCHAIN_FAKE=denied` 运行，`Default` 有标准书签、历史与标准 Cookie fixture，用户勾选三类并开始导入
- THEN 5 秒内向导显示「部分完成」、`书签 3`、`Cookie 0`，以及「未获得钥匙串授权，Cookie 未导入」
