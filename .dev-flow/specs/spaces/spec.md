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

### Requirement: 新建 Space
系统 SHALL 在收到新建 Space 的请求时，用去掉首尾空白后长度为 1–32 个字符的名称和一个预设颜色，在全部 Space 的末尾新建一个 Space 并把它设为当前 Space。

#### Scenario: 新建后成为当前 Space
- WHEN alterd 只有 `Personal` 一个 Space，客户端发送 `{"type":"shell.event.spaceCreated","name":"  Work ","color":"#f97316"}`
- THEN 新快照的 `spaces` 依次为 `Personal`、`Work`，`Work` 的 `color` 为 `#f97316`
- AND 快照的 `activeSpaceId` 等于 `Work` 的 `id`

#### Scenario: 名称为空被拒绝
- WHEN 客户端发送 `spaceCreated`，`name` 为 `"   "`，`color` 为 `#f97316`
- THEN 客户端收到 `shell.error`，`code` 为 `E_INVALID_ARGUMENT`
- AND 之后 1 秒内没有收到 `shell.stateChanged`

#### Scenario: 名称超过 32 个字符被拒绝
- WHEN 客户端发送 `spaceCreated`，`name` 为 33 个 `a`，`color` 为 `#f97316`
- THEN 客户端收到 `shell.error`，`code` 为 `E_INVALID_ARGUMENT`

#### Scenario: 非预设颜色被拒绝
- WHEN 客户端发送 `spaceCreated`，`name` 为 `Work`，`color` 为 `#123456`
- THEN 客户端收到 `shell.error`，`code` 为 `E_INVALID_ARGUMENT`

#### Scenario: 在侧边栏新建 Space
- WHEN 以空的临时目录为 `ALTER_HOME` 启动 app，用户点击侧边栏底部的「+」，在面板的名称框输入 `Work`，点第 3 个颜色（`#f97316`），再点「创建」
- THEN 2 秒内侧边栏底部有 2 个 Space 圆点，第 2 个处于激活样式
- AND 底部的 Space 名称显示 `Work`，标签列表为空，内容区显示 `没有打开的标签`

### Requirement: 编辑 Space
系统 SHALL 在收到编辑请求时，按与新建相同的名称和颜色规则更新指定 Space 的名称或颜色，不改变它的 `id`、顺序和当前 Space。

#### Scenario: 改名
- WHEN 当前 Space 为 `Personal`，另有 Space `Work`，客户端发送 `{"type":"shell.event.spaceUpdated","spaceId":"<Work 的 id>","name":"Office"}`
- THEN 新快照的 `spaces` 依次为 `Personal`、`Office`，`Office` 的 `id` 与原 `Work` 相同，`color` 不变
- AND `activeSpaceId` 仍等于 `Personal` 的 `id`

#### Scenario: Space 不存在
- WHEN 客户端发送 `spaceUpdated`，`spaceId` 为 `01JZZZZZZZZZZZZZZZZZZZZZZZ`，`name` 为 `X`
- THEN 客户端收到 `shell.error`，`code` 为 `E_SPACE_NOT_FOUND`

#### Scenario: 在侧边栏改名
- WHEN app 只有 `Personal` 一个 Space，用户右键它的圆点，在面板里把名称改成 `Home` 并点「保存」
- THEN 2 秒内底部的 Space 名称显示 `Home`
- AND `ALTER_HOME/alter.db` 中该 Space 的 `spaces.name` 为 `Home`

### Requirement: 切换 Space
系统 SHALL 在用户切换到某个 Space 后，把它设为当前 Space，侧边栏只列出它的标签，内容区显示它自己记住的激活标签。

#### Scenario: 切换改变当前 Space
- WHEN 有 Space `Personal` 与 `Work`，当前为 `Work`，客户端发送 `{"type":"shell.event.spaceActivated","spaceId":"<Personal 的 id>"}`
- THEN 新快照的 `activeSpaceId` 等于 `Personal` 的 `id`

#### Scenario: Space 不存在
- WHEN 客户端发送 `spaceActivated`，`spaceId` 为 `01JZZZZZZZZZZZZZZZZZZZZZZZ`
- THEN 客户端收到 `shell.error`，`code` 为 `E_SPACE_NOT_FOUND`

#### Scenario: 每个 Space 记住自己的激活标签
- WHEN app 的 `Personal` 里打开了本地测试页 `/a`，用户新建 Space `Work` 并在其中打开 `/b`，然后点击 `Personal` 的圆点
- THEN 1 秒内侧边栏标签列表恰好为 `Page A`，内容区页面的 URL 为 `/a` 的地址
- AND 再点击 `Work` 的圆点后 1 秒内，标签列表恰好为 `Page B`，内容区页面的 URL 为 `/b` 的地址

### Requirement: 重启后回到上次的 Space
系统 SHALL 在 alterd 重启后，把上次关闭前的当前 Space 作为当前 Space；从未切换过 Space 的数据库以排在最前的 Space 为当前 Space。

#### Scenario: 重启保留当前 Space
- WHEN 以同一个临时 `ALTER_HOME` 运行 `alterd run`，客户端发送 `spaceCreated`（`Work`、`#f97316`）使 `Work` 成为当前 Space，然后以 SIGTERM 结束 alterd 并再次运行
- THEN 新连接收到的第一份快照中 `activeSpaceId` 等于 `Work` 的 `id`

#### Scenario: 旧数据库没有记录时取第一个 Space
- WHEN `ALTER_HOME/alter.db` 是一个只应用了迁移 0001 的数据库，其中有 `position` 为 0 的 `Personal` 和 `position` 为 1 的 `Work`，运行 `alterd run`
- THEN 第一份快照的 `activeSpaceId` 等于 `Personal` 的 `id`

### Requirement: 删除 Space
系统 SHALL 在收到删除请求时，从数据库中删除该 Space 及其全部标签；如果删的是当前 Space，就把它后面相邻的 Space 设为当前 Space，没有后面的就取前面相邻的；只剩一个 Space 时拒绝删除。

#### Scenario: 删除中间的当前 Space
- WHEN 有按顺序排列的 Space `Personal`、`Work`、`Play`，当前为 `Work`，`Work` 里有 1 个标签，客户端发送 `{"type":"shell.event.spaceDeleted","spaceId":"<Work 的 id>"}`
- THEN 新快照的 `spaces` 依次为 `Personal`、`Play`，`activeSpaceId` 等于 `Play` 的 `id`
- AND `ALTER_HOME/alter.db` 的 `spaces` 中没有 `Work` 的 `id`，`tabs` 中没有 `space_id` 为 `Work` 的 `id` 的行

#### Scenario: 删除末尾的当前 Space
- WHEN 有按顺序排列的 Space `Personal`、`Work`，当前为 `Work`，客户端发送 `spaceDeleted`（`Work` 的 `id`）
- THEN 新快照的 `activeSpaceId` 等于 `Personal` 的 `id`

#### Scenario: 删除非当前 Space
- WHEN 有 Space `Personal`、`Work`，当前为 `Personal`，客户端发送 `spaceDeleted`（`Work` 的 `id`）
- THEN 新快照的 `spaces` 只有 `Personal`，`activeSpaceId` 仍等于 `Personal` 的 `id`

#### Scenario: 不能删除最后一个 Space
- WHEN 只有 `Personal` 一个 Space，客户端发送 `spaceDeleted`（`Personal` 的 `id`）
- THEN 客户端收到 `shell.error`，`code` 为 `E_INVALID_ARGUMENT`
- AND 之后 1 秒内没有收到 `shell.stateChanged`

#### Scenario: 在侧边栏删除 Space 并清掉 Cookie
- WHEN app 有 `Personal` 与 `Work`，当前为 `Work`，`Work` 里打开了本地测试页 `/set-cookie`（响应头 `Set-Cookie: k=v`），用户右键 `Work` 的圆点，点「删除」，在确认框里点「删除」
- THEN 2 秒内底部只剩 1 个 Space 圆点，底部的 Space 名称显示 `Personal`
- AND 主进程里 `session.fromPartition("persist:space-<Work 的 id>").cookies.get({})` 返回空数组

#### Scenario: 取消确认不删除
- WHEN app 有 `Personal` 与 `Work`，用户右键 `Work` 的圆点，点「删除」，在确认框里点「取消」
- THEN 1 秒后底部仍有 2 个 Space 圆点，`ALTER_HOME/alter.db` 的 `spaces` 中仍有 `Work`

### Requirement: 启动时清理已删除 Space 的数据目录
系统 SHALL 在 alterd 启动时删除 `ALTER_HOME/chromium/Partitions/` 下名为 `space-<id>`、而 `<id>`（不区分大小写）不属于任何现存 Space 的目录，其他目录保持不动。

#### Scenario: 只删除孤儿目录
- WHEN `ALTER_HOME` 里已有 Space `Personal`（数据库中的 id 例如 `01JABCDEFGHJKMNPQRSTVWXYZ0`），`ALTER_HOME/chromium/Partitions/` 下有 `space-01jabcdefghjkmnpqrstvwxyz0`、`space-01jzzzzzzzzzzzzzzzzzzzzzzz`、`default` 三个目录，运行 `alterd run`
- THEN `ALTER_HOME/alterd.json` 出现时，`space-01jzzzzzzzzzzzzzzzzzzzzzzz` 已不存在
- AND `space-01jabcdefghjkmnpqrstvwxyz0` 与 `default` 仍然存在

### Requirement: Space 之间 Cookie 隔离
系统 SHALL 让每个 Space 的页面只能读写自己 Space 的 Cookie。

#### Scenario: 另一个 Space 看不到 Cookie
- WHEN app 的 `Personal` 里打开本地测试页 `/set-cookie`（响应头 `Set-Cookie: k=v`），然后新建 Space `Work` 并在其中打开 `/echo-cookie`（页面正文为 `cookie: ` 加请求的 Cookie 头）
- THEN `Work` 的页面正文为 `cookie: `
- AND 切回 `Personal` 并新开标签打开 `/echo-cookie` 后，页面正文为 `cookie: k=v`
