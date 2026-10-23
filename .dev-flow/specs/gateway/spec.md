# gateway Specification

## Purpose
alterd 对 SDK 与 CLI 暴露的本机 WebSocket JSON-RPC 接口。

## Requirements

### Requirement: 启动时发布网关地址
系统 SHALL 在 alterd 启动后监听 127.0.0.1 上的随机端口，并把 `{port, token, cdpPort, version}` 写入权限为 0600 的 `ALTER_HOME/gateway.json`，退出时删除该文件。

#### Scenario: 写出 gateway.json
- WHEN 以空的临时目录为 `ALTER_HOME` 运行 `alterd run`
- THEN `ALTER_HOME/alterd.json` 出现时 `ALTER_HOME/gateway.json` 也已存在，权限位为 `0600`
- AND 其中 `token` 是 64 个十六进制字符，`version` 为 `0.1.0`，`port` 与 `cdpPort` 都能在 `127.0.0.1` 上建立 TCP 连接

#### Scenario: 退出时删除
- WHEN 已写出 `gateway.json` 的 `alterd run` 收到 SIGTERM
- THEN 进程退出后 `ALTER_HOME/gateway.json` 不存在

### Requirement: 连接鉴权
系统 SHALL 只接受 WebSocket 握手请求头带有 `Authorization: Bearer <gateway.json 中的 token>` 的连接，其余握手以 HTTP 401 拒绝。

#### Scenario: 缺少 token
- WHEN 客户端不带 `Authorization` 头连接 `ws://127.0.0.1:<port>/rpc`
- THEN 握手响应状态码为 401

#### Scenario: token 错误
- WHEN 客户端带 `Authorization: Bearer 0000` 连接
- THEN 握手响应状态码为 401

#### Scenario: token 一致
- WHEN 客户端带与 `gateway.json` 一致的 token 连接并发送 `{"jsonrpc":"2.0","id":1,"method":"hello","params":{"clientVersion":"0.1.0","protocolVersion":1}}`
- THEN 收到 `id` 为 1 的响应，`result.protocolVersion` 为 1，`result.pageBudget` 为 8，`result.spaces` 恰好 1 项且 `name` 为 `Personal`

### Requirement: 协议版本与错误格式
系统 SHALL 以 JSON-RPC 2.0 错误返回每个失败的调用，`error.data` 为 `{"code":"E_…","message":"<说明>"}`，并拒绝协议版本不是 1 的 `hello`。

#### Scenario: 协议版本不匹配
- WHEN 已鉴权的客户端发送 `hello`，`protocolVersion` 为 2
- THEN 响应的 `error.data.code` 为 `E_PROTOCOL_VERSION`

#### Scenario: 未知方法
- WHEN 已鉴权的客户端发送方法 `page.teleport`
- THEN 响应的 `error.code` 为 -32601，`error.data.code` 为 `E_INVALID_ARGUMENT`

#### Scenario: 参数缺失
- WHEN 已鉴权的客户端发送 `taskSpace.create`，`params` 为 `{}`
- THEN 响应的 `error.data.code` 为 `E_INVALID_ARGUMENT`，`error.data.message` 包含 `name`

### Requirement: 列出 Space
系统 SHALL 在 `space.list` 时返回全部 Space 的 `id`、`name`、`color` 与未归档标签数 `tabCount`，顺序与侧边栏相同。

#### Scenario: 列出两个 Space
- WHEN alterd 有 Space `Personal`（2 个标签）与 `Work`（0 个标签），客户端调用 `space.list`
- THEN `result.spaces` 依次为 `{name:"Personal", tabCount:2}`、`{name:"Work", tabCount:0}`（另含 `id`、`color`）

### Requirement: 更新提示通知
系统 SHALL 在 `config.toml` 没有设置 `update_check = false` 时，于 alterd 启动后与之后每 24 小时从 Release API 取一次最新版本；比自身新时，向当时已连接与之后连接的每个 Gateway 客户端各推送一次 `app.updateAvailable {version, url}`。检查失败时只记 warn 日志。

#### Scenario: 启动后推送新版本
- WHEN 假 Release 服务器的 latest 为 `{"tag_name":"v9.0.0","html_url":"https://example.invalid/r/v9.0.0"}`，以 `ALTER_RELEASE_API=<假服务器>`、`ALTER_UPDATE_CHECK_DELAY_MS=0` 启动 alterd，Gateway 客户端连接并完成鉴权
- THEN 该客户端 3 秒内收到 `{"method":"app.updateAvailable","params":{"version":"9.0.0","url":"https://example.invalid/r/v9.0.0"}}`
- AND 之后再连接的客户端在鉴权后也收到同样的一条

#### Scenario: 已是最新或关闭检查时不推送
- WHEN latest 为 alterd 自身版本，或 `config.toml` 中有 `update_check = false`（此时 latest 为 `v9.0.0`），以同样的环境启动 alterd 并连接 Gateway
- THEN 5 秒内客户端没有收到 `app.updateAvailable`
- AND `update_check = false` 时假 Release 服务器没有收到任何请求

#### Scenario: 检查失败
- WHEN `ALTER_RELEASE_API` 指向一个没有监听的本地端口，`ALTER_UPDATE_CHECK_DELAY_MS=0`，启动 alterd
- THEN alterd 的 stderr 3 秒内有一行 `level` 为 `WARN`、消息包含 `检查更新失败` 的 JSON 日志，alterd 仍在运行
