# history Specification

## Purpose
用户在各个 Space 里访问过的页面。

## Requirements

### Requirement: 记录浏览历史
系统 SHALL 在标签的主框架每次完成导航后，在该标签所属 Space 的历史里追加一条 `source` 为 `alter` 的记录，之后的标题回写更新这条记录的标题。

#### Scenario: 导航与标题写入历史
- WHEN Space `Personal` 有标签 T，客户端依次发送 `tabUpdated {tabId:T, url:"https://example.com/a"}`、`tabUpdated {tabId:T, title:"A"}`、`tabUpdated {tabId:T, url:"https://example.com/b"}`
- THEN `ALTER_HOME/alter.db` 的 `history` 中 `space_id` 为 `Personal` 的 id 的行按 `visited_at` 依次为 `(https://example.com/a, A)`、`(https://example.com/b, A)`，`source` 都为 `alter`

#### Scenario: iframe 导航不写历史
- WHEN 激活标签显示本地测试页 `/frame`（内嵌 iframe 改写自己的 hash）
- THEN 2 秒后 `history` 中只有 `/frame` 的 1 条记录
