# agent-skill Specification

## Purpose
给 AI agent 的 alter 使用说明，随 app 打包并可用 `alter skill install` 安装。

## Requirements

### Requirement: SKILL.md 的入口信息
系统 SHALL 提供 `skills/alter-browser/SKILL.md`，其 front matter 有 `name: alter-browser` 与一段说明何时使用的 `description`，正文给出用 `alter nodejs` heredoc 运行脚本的最小示例，并链接 `references/` 下的 `api.md`、`handoff.md`、`playwright.md` 与 `install.md`。

#### Scenario: front matter 与链接
- WHEN 读取 `skills/alter-browser/SKILL.md`
- THEN front matter 能被解析，`name` 为 `alter-browser`，`description` 非空且包含 `alter`
- AND 正文第一个 `bash` 代码块包含 `alter nodejs <<'EOF'`
- AND 正文链接的 `references/api.md`、`references/handoff.md`、`references/playwright.md`、`references/install.md` 都存在

### Requirement: API 参考与 SDK 一致
系统 SHALL 让 `references/api.md` 为每个 SDK 公开 API 各有一个以其调用名开头的三级标题（例如 `### page.click`），且不出现 SDK 里不存在的调用名。

#### Scenario: 文档与导出双向一致
- WHEN 用 SDK 的 `Page`、`TaskSpace` 原型与 `runtime.ts` 挂到全局的函数求出公开 API 的调用名集合，再从 `references/api.md` 的三级标题中取出调用名集合
- THEN 两个集合相等

#### Scenario: SDK 新增方法而文档没写时失败
- WHEN 测试在 `Page` 原型上临时加一个方法 `zzzProbe` 后重新求公开 API 集合
- THEN 两个集合不相等，差集恰好是 `page.zzzProbe`

### Requirement: 交接控制权的指引
系统 SHALL 在 `references/handoff.md` 中说明：收到 `E_USER_CONTROL` 时不得重试、要等用户交还；遇到登录、验证码时用 `handOff()` 交给用户，再用 `waitForControl()` 等待。

#### Scenario: 关键规则写明
- WHEN 读取 `skills/alter-browser/references/handoff.md`
- THEN 文本包含 `E_USER_CONTROL`、`do not retry`、`handOff()` 与 `waitForControl()`

### Requirement: 示例可以真实运行
系统 SHALL 让 `SKILL.md` 第一个 `bash` 代码块中的示例在 alter 运行时可以原样执行成功（只把示例里的 `https://example.com` 换成可访问的地址）。

#### Scenario: 对本地页面跑快速上手示例
- WHEN app 在运行，本地测试服务器提供标题为 `Example Domain` 的页面，把 `SKILL.md` 第一个 `bash` 代码块中的 `https://example.com` 替换为该页面地址后用 bash 执行
- THEN 退出码为 0，stdout 包含 `Example Domain`
