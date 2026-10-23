# sdk Specification

## Purpose
`@alter/sdk`：在 Bun 里运行的 TaskSpace / Page API。

## Requirements

### Requirement: SDK 请求与错误映射
系统 SHALL 让 SDK 的每个方法发出与 Gateway 契约一致的 JSON-RPC 请求，并把错误响应转换为带 `code`、`message`、`details` 的 `AlterError` 异常，且不自动重试。

#### Scenario: taskSpace 按名称创建、按数字恢复
- WHEN 对一个记录请求的假 Gateway 调用 `taskSpace("demo")` 和 `taskSpace(7)`
- THEN 假 Gateway 依次收到 `taskSpace.create {"name":"demo"}` 与 `taskSpace.resume {"id":7}`

#### Scenario: 错误映射且不重试
- WHEN 假 Gateway 对 `page.click` 返回 `error.data` 为 `{"code":"E_USER_CONTROL","message":"user has control"}`，SDK 调用 `page.click("text=Go")`
- THEN 抛出的异常是 `AlterError`，`code` 为 `E_USER_CONTROL`
- AND 假 Gateway 恰好收到 1 次 `page.click` 请求

#### Scenario: 连接失败
- WHEN `gateway.json` 指向一个没有监听的端口，调用 `taskSpace("demo")`
- THEN 抛出的 `AlterError` 的 `code` 为 `E_INTERNAL`，`message` 包含 `alter.app`

### Requirement: 等待用户交还
系统 SHALL 让 SDK 的 `task.waitForControl({timeoutMs})` 在收到该 TaskSpace 的 `taskSpace.ownershipChanged` 且 `ownership` 为 `agent` 时返回，超时则抛出 `code` 为 `E_TIMEOUT` 的 `AlterError`。

#### Scenario: 收到交还后返回
- WHEN 假 Gateway 在 SDK 调用 `task.waitForControl({timeoutMs: 2000})` 后 200 ms 推送 `{"method":"taskSpace.ownershipChanged","params":{"id":3,"ownership":"agent","reason":"returned"}}`
- THEN `waitForControl` 在 1 秒内返回

#### Scenario: 超时
- WHEN 假 Gateway 不推送任何事件，SDK 调用 `task.waitForControl({timeoutMs: 300})`
- THEN 抛出 `AlterError`，`code` 为 `E_TIMEOUT`

#### Scenario: E_USER_CONTROL 不被重试
- WHEN 假 Gateway 对 `page.goto` 返回 `E_USER_CONTROL`，SDK 调用 `page.goto("https://example.com")`
- THEN 抛出 `code` 为 `E_USER_CONTROL` 的 `AlterError`，假 Gateway 恰好收到 1 次 `page.goto`

### Requirement: 打印更新提示
系统 SHALL 在 `alter nodejs` 运行的脚本收到 `app.updateAvailable` 时，向 stderr 打印恰好一次 `[alter:notice] alter <version> is available, run: alter upgrade`。

#### Scenario: 收到两次通知只打印一次
- WHEN 假 Gateway 在 SDK 连接后推送两次 `app.updateAvailable {"version":"9.0.0","url":"…"}`
- THEN SDK 运行时的 stderr 恰好一行 `[alter:notice] alter 9.0.0 is available, run: alter upgrade`
