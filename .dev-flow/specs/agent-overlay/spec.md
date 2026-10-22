# agent-overlay Specification

## Purpose
页面上显示 agent 正在做什么。

## Requirements

### Requirement: 动作叠层
系统 SHALL 在 ownership 为 `agent` 的 Page 上执行带 `actionLabel` 的元素动作时，在页面上方显示 agent 光标（位于目标元素中心）与动作文字，叠层不拦截用户的鼠标事件。

#### Scenario: 点击时显示光标与文字
- WHEN `p1` 已打开 `/form`，脚本执行 `page.click("text=Submit", {actionLabel: "提交表单"})`
- THEN 1 秒内经 CDP 转发端点读取该页面时，`document.querySelector("alter-agent-overlay").shadowRoot` 中 `[data-role=action]` 的文本为 `提交表单`
- AND `[data-role=cursor]` 的中心与 `Submit` 按钮中心的距离不超过 4 px

#### Scenario: 叠层不拦截点击
- WHEN `p1` 上显示着叠层，测试经 CDP 转发端点在叠层覆盖的 `Submit` 按钮中心派发一次鼠标点击
- THEN 页面正文出现 `submitted:`

#### Scenario: 用户控制期间隐藏叠层
- WHEN `p1` 上显示着叠层，TaskSpace 执行 `handOff`
- THEN 1 秒内 `alter-agent-overlay` 的计算样式 `display` 为 `none`
