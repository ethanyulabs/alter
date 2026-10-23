# shell-control Specification

## Purpose
alterd 与 Electron Main 之间的本地控制通道：alterd 下发状态与指令，Electron 上报用户事件。

## Requirements

### Requirement: 连接即收到全量快照
系统 SHALL 在 `ALTER_HOME/shell.sock` 上接受本机连接，并在每个连接建立后立即向它发送一条 `shell.stateChanged` 全量状态快照。

#### Scenario: 新连接收到快照
- WHEN 以空的临时目录为 `ALTER_HOME` 运行 `alterd run`，等 `alterd.json` 出现后用 Unix socket 连接 `ALTER_HOME/shell.sock`
- THEN 1 秒内收到一行 JSON，`type` 为 `shell.stateChanged`
- AND 其中 `spaces` 恰好 1 项，`name` 为 `Personal`，`color` 为 `#6366f1`，`activeSpaceId` 等于该项的 `id`
- AND `tabs` 为空数组

#### Scenario: socket 只允许当前用户访问
- WHEN alterd 已启动且 `ALTER_HOME/alterd.json` 已出现
- THEN `ALTER_HOME/shell.sock` 的权限位为 `0600`

### Requirement: 事件生效后广播快照
系统 SHALL 在接受一个 Electron 上报的事件并改变状态后，向所有已连接的客户端各发送一条新的 `shell.stateChanged` 快照；某个连接积压的未送达快照超过 256 份时，可以跳过其中最旧的，但最后发布的那份快照一定送达。

#### Scenario: 两个连接都收到变化
- WHEN 客户端 A 与 B 都已连接并读完初始快照，A 发送 `{"type":"shell.event.openRequested","spaceId":"<Personal 的 id>","input":"https://example.com/a"}`
- THEN A 与 B 在 1 秒内各收到一条 `shell.stateChanged`，其 `tabs` 恰好 1 项，`url` 为 `https://example.com/a`

#### Scenario: 无法解析的行被丢弃且连接保持
- WHEN 已连接的客户端先发送一行 `not json`，再发送一条合法的 `shell.event.openRequested`（`input` 为 `https://example.com/b`）
- THEN 连接没有被关闭
- AND 客户端收到的下一条 `shell.stateChanged` 中有 `url` 为 `https://example.com/b` 的标签

#### Scenario: 慢连接积压时仍收到最新快照
- WHEN 客户端 B 已连接并读完初始快照后停止读取，客户端 A 依次发送 300 条 `openRequested`（`input` 为 `https://example.com/0` … `https://example.com/299`）并读完自己的 300 份快照，然后 B 开始读取
- THEN B 的连接没有被关闭
- AND B 在 2 秒内读到一份 `tabs` 恰好 300 项的快照

#### Scenario: 被拒绝的事件只回错误给发送方
- WHEN 客户端 A 与 B 都已连接，A 发送 `openRequested`，其 `spaceId` 为 `01JZZZZZZZZZZZZZZZZZZZZZZZ`（不存在）
- THEN A 在 1 秒内收到 `{"type":"shell.error","code":"E_SPACE_NOT_FOUND",…}`
- AND B 在 1 秒内没有收到任何消息

### Requirement: socket 路径过长时启动失败
系统 SHALL 在 `ALTER_HOME/shell.sock` 的路径超过 macOS Unix socket 路径上限（103 字节）时，以退出码 1 退出，不写 `alterd.json`。

#### Scenario: 路径过长
- WHEN `ALTER_HOME` 是一个路径长度为 120 字节的空目录，运行 `alterd run`
- THEN alterd 在 2 秒内退出，退出码为 1
- AND stderr 中有一行 JSON 日志带 `code` 为 1，且消息包含 `shell.sock`
- AND `ALTER_HOME/alterd.json` 不存在

### Requirement: 把 Cookie 写进 Space 的 partition
系统 SHALL 在 Electron Main 收到 `shell.importCookies` 时，把其中每个 Cookie 写进 partition `persist:space-<spaceId>`，再回一条带同一 `requestId` 的 `shell.event.cookiesImported`，其中 `imported` 为写入成功数、`rejected` 为被拒绝数。

#### Scenario: 只有目标 Space 带上导入的 Cookie
- WHEN app 有 Space `Personal` 与 `Work`，`ALTER_CHROME_DIR` 的 `Default` 有标准 Cookie fixture，alterd 带 `ALTER_KEYCHAIN_FAKE=peanuts` 运行，从导入向导把 Chrome `Default` 的 Cookie 导入 `Work`
- THEN 在 `Work` 里打开本地测试服务器的 `http://127.0.0.1:<port>/echo-cookie`，页面显示的请求头 `Cookie` 包含 `sid=abc`
- AND 在 `Personal` 里打开同一地址，页面显示的 `Cookie` 不包含 `sid=abc`
