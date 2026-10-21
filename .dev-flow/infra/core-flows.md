# 核心流程

<!-- 承载业务的少数几条请求 / 事件路径。每条流程一节、一张时序图。 -->

## 流程：用户在 Space 里打开页面
<!-- dflow:diagram assets/seq-open-tab.mmd -->
```mermaid
sequenceDiagram
  autonumber
  participant U as 用户
  participant R as Renderer (CommandBar)
  participant M as Electron Main (ShellControl)
  participant D as alterd (TaskSpaceManager / Store)
  participant C as Chromium (BrowserView)
  U->>R: Cmd+T 输入 "github.com" 回车
  R->>M: IPC tab.open {spaceId, input}
  M->>D: shell.event.openRequested {spaceId, url}
  D->>D: 解析输入（URL / 搜索 / 已有标签）
  D->>D: INSERT tabs(space_id, url, position)
  D->>M: shell.createTab {tabId, spaceId, url}
  M->>C: new BrowserView(partition=persist:space-<id>) 并 loadURL
  C-->>M: did-finish-load / 标题 / favicon
  M->>D: shell.event.tabUpdated {tabId, title, favicon}
  D->>D: UPDATE tabs SET title, favicon
  D-->>R: 事件 tabs.changed（经 Main 转发）
  R-->>U: 侧边栏出现新标签并激活
  alt 输入匹配已打开标签
    D->>M: shell.activateTab {tabId}
    M-->>R: 事件 tabs.changed
  end
  alt Space 不存在或已归档
    D-->>M: 错误 E_SPACE_NOT_FOUND
    M-->>R: 命令栏提示「Space 不存在」
  end
```
<!-- /dflow:diagram -->

- 触发：用户在某个 Space 内按 Cmd+T 打开命令栏，输入 URL、搜索词或已打开标签的名字后回车。
- 步骤：1–2 Renderer 经 IPC 把输入交给 Electron Main；3 Main 经 Shell Control Channel 上报 alterd；4–5 alterd 解析输入（URL / 搜索 / 匹配已有标签），落库分配 tab id；6–7 alterd 下发 `shell.createTab`，Main 用该 Space 的 partition 创建 BrowserView 并加载；8–10 加载事件回流 alterd 更新标题与 favicon；11–12 变更事件推到 Renderer，侧边栏出现并激活新标签。
- 失败处理：输入匹配已打开标签时只激活不新建；Space 不存在或已归档返回 `E_SPACE_NOT_FOUND`，命令栏内联提示；页面加载失败由 Chromium 自己的错误页承担，alterd 只记录 `tabs.title` 为错误标题。
- 代码入口：TBD — `alter:apps/browser/src/renderer/command-bar/`、`alter:apps/browser/src/main/shell-control.ts`、`alter:crates/alterd/src/shell/`、`alter:crates/alterd/src/tabs/`（建侧边栏需求归档时填真实路径）。

## 流程：agent 用脚本驱动页面
<!-- dflow:diagram assets/seq-agent-script.mmd -->
```mermaid
sequenceDiagram
  autonumber
  participant A as Agent
  participant CLI as alter CLI (Rust)
  participant S as alter-sdk (Bun)
  participant G as alterd Gateway
  participant T as alterd TaskSpaceManager / CdpMux
  participant M as Electron Main
  participant C as Chromium
  participant O as Renderer AgentOverlay
  A->>CLI: alter nodejs（heredoc 脚本）
  CLI->>CLI: 读取 gateway.json（端口 + token）
  CLI->>S: 启动 Bun，注入 ALTER_GATEWAY_URL / ALTER_TOKEN
  S->>G: WS 连接 + Bearer token
  G-->>S: hello {version, pageBudget}
  S->>G: taskSpace.create {name, spaceId?}
  G->>T: 创建 TaskSpace（ownership=agent）
  T->>M: shell.createTab {tabId, label=p1, agentOwned}
  M->>C: new BrowserView(partition)
  T-->>S: {spaceId, pages:[p1]}
  S->>G: page.goto {label=p1, url}
  G->>T: 校验 ownership=agent
  T->>C: CDP Page.navigate（直连，不经 Main）
  C-->>T: Page.loadEventFired
  T->>O: overlay.action {label=p1, text="打开页面"}
  T-->>S: receipt {ok, durationMs}
  S->>G: page.snapshot {label=p1}
  T->>C: CDP Accessibility.getFullAXTree
  T-->>S: snapshot 文本（带 @ref）
  S-->>CLI: stdout 打印 snapshot
  CLI-->>A: 退出码 0
  alt token 无效或缺失
    G-->>S: 错误 E_UNAUTHORIZED
    S-->>CLI: 退出码 2，提示重新打开 alter.app
  end
  alt 页面预算用尽
    T-->>S: 错误 E_PAGE_BUDGET {budget, open}
  end
  alt 选择器匹配多个元素
    T-->>S: 错误 E_AMBIGUOUS_SELECTOR {matches}
  end
```
<!-- /dflow:diagram -->

- 触发：agent 运行 `alter nodejs`（heredoc 或 `-e`）执行一段使用 SDK 的脚本。
- 步骤：1–3 CLI 读 `gateway.json`，启动打包的 Bun 并注入网关地址与 token；4–5 SDK 建立 WS 连接并通过鉴权；6–10 `taskSpace()` 让 alterd 创建 ownership=agent 的 TaskSpace，并经 ShellLink 让 Electron 建一个带 `p1` 标签的页面；11–16 `page.goto` 经 Gateway 校验 ownership 后由 CdpMux 直接发 CDP，完成后向 AgentOverlay 推送动作文字，并返回 receipt；17–20 `page.snapshot` 取无障碍树，打印到 stdout；21 CLI 以退出码 0 结束，TaskSpace 与页面保留到 `finish()`。
- 失败处理：token 无效 → `E_UNAUTHORIZED`，CLI 退出码 2 并提示重开 alter.app；页面预算用尽 → `E_PAGE_BUDGET`，附当前预算与打开数；选择器多匹配 → `E_AMBIGUOUS_SELECTOR`，附匹配数；CDP 超时 → `E_TIMEOUT`，TaskSpace 保持可用。每个动作的 receipt 都写入 alterd 日志。
- 代码入口：TBD — `alter:crates/alter-cli/src/nodejs.rs`、`alter:packages/sdk/src/task-space.ts`、`alter:crates/alterd/src/gateway/`、`alter:crates/alterd/src/task_space/`、`alter:crates/alterd/src/cdp/`。

## 流程：交还与接管
<!-- dflow:diagram assets/seq-handoff.mmd -->
```mermaid
sequenceDiagram
  autonumber
  participant A as Agent 脚本 (alter-sdk)
  participant G as alterd Gateway
  participant T as alterd TaskSpaceManager
  participant M as Electron Main
  participant R as Renderer
  participant U as 用户
  A->>G: taskSpace.handOff {spaceId}
  G->>T: ownership agent -> user
  T->>T: UPDATE task_spaces SET ownership='user'
  T->>M: shell.setOverlay {spaceId, mode="user"}
  M->>R: 隐藏 agent 光标，显示「等待你操作」横幅
  T-->>A: ok
  A-->>A: 脚本结束，agent 向用户说明要做什么
  U->>R: 在页面上完成登录 / 验证码
  U->>R: 点击「交还给 Agent」
  R->>M: IPC taskSpace.returnToAgent
  M->>G: shell.event.ownershipReturned {spaceId}
  G->>T: ownership user -> agent
  T-->>R: 事件 taskSpace.ownershipChanged
  A->>G: takeOverTaskSpace {spaceId}（新一轮脚本）
  G-->>A: {ownership: "agent", userPage: p1}
  alt 用户在 agent 操作中途接管（点击页面或「停止 Agent」）
    R->>M: IPC taskSpace.takeControl
    M->>G: shell.event.userTookControl {spaceId}
    G->>T: ownership -> user，挂起 CDP 会话
    A->>G: page.click {label=p1}
    G-->>A: 错误 E_USER_CONTROL（脚本必须停止，不重试）
  end
```
<!-- /dflow:diagram -->

- 触发：agent 遇到必须由人完成的步骤（登录、验证码、权限弹窗）调用 `task.handOff()`；或用户在 agent 操作中途主动接管。
- 步骤：1–5 handOff 把 ownership 切到 user，落库，并让 Electron 隐藏 agent 光标、显示「等待你操作」横幅；6–8 脚本结束，用户在页面上操作；9–13 用户点「交还给 Agent」，Electron 上报，alterd 切回 agent 并广播 `taskSpace.ownershipChanged`；14–15 agent 新一轮脚本用 `takeOverTaskSpace(id)` 接回，得到 `userPage`。
- 失败处理：ownership=user 期间 agent 的任何 `page.*` 调用返回 `E_USER_CONTROL`，SDK 抛异常，脚本必须停止，不得重试或换 TaskSpace 绕开；用户主动接管时 alterd 挂起该 TaskSpace 的 CDP 会话，正在执行的命令以 `E_USER_CONTROL` 结束；TaskSpace 在 user 状态超过配置阈值未交还则标记 `inactive`，`listTaskSpaces()` 可见。
- 代码入口：TBD — `alter:crates/alterd/src/task_space/ownership.rs`、`alter:apps/browser/src/renderer/agent-overlay/`、`alter:packages/sdk/src/task-space.ts`。

## 流程：从 Chrome / Arc 导入 profile
<!-- dflow:diagram assets/seq-import-profile.mmd -->
```mermaid
sequenceDiagram
  autonumber
  participant U as 用户
  participant R as Renderer (导入向导) / alter CLI
  participant G as alterd Gateway
  participant I as alterd ImportService
  participant K as macOS Keychain
  participant F as Chrome / Arc 数据目录
  participant S as alterd Store
  participant C as Chromium (Space partition)
  U->>R: 选择来源 chrome|arc、源 profile、目标 Space
  R->>G: import.run {source, profilePath, spaceId, items:[bookmarks,history,cookies]}
  G->>I: 启动导入任务 INSERT imports(status='running')
  I->>F: 拷贝 Bookmarks(JSON) / History(SQLite) / Cookies(SQLite) 到临时目录
  I->>I: 解析书签树、最近 90 天历史
  I->>S: 批量 INSERT bookmarks / history
  I->>K: security find-generic-password "Chrome Safe Storage"
  K-->>U: 系统弹窗请求授权
  U-->>K: 允许
  K-->>I: 密钥（仅内存）
  I->>I: PBKDF2 + AES-128-CBC 解密 encrypted_value
  I->>C: CDP Network.setCookies（按域名批量，走该 Space 的 partition）
  I->>S: UPDATE imports SET status='done', counts
  G-->>R: 事件 import.progress / import.done {bookmarks, history, cookies}
  R-->>U: 显示导入结果
  alt 用户拒绝 Keychain 授权
    K-->>I: errSecAuthFailed
    I->>S: UPDATE imports SET status='partial', error='keychain_denied'
    G-->>R: import.done {cookies: 0, warning: "未授权，已导入书签与历史"}
  end
  alt 源浏览器正在运行导致 SQLite 锁
    I->>I: 已先拷贝文件，不受锁影响
  end
```
<!-- /dflow:diagram -->

- 触发：首次启动向导、设置页的「导入」，或 `alter import --from chrome|arc --space <id>`。
- 步骤：1–3 用户选来源、源 profile 和目标 Space，Gateway 创建 `imports` 记录；4–6 ImportService 先把 Bookmarks（JSON）、History、Cookies（SQLite）拷到临时目录再读，解析书签树与最近 90 天历史并批量写库；7–11 经 Keychain 取「Chrome Safe Storage」密钥（Arc 为「Arc Safe Storage」），系统弹窗由用户授权，PBKDF2 派生后 AES-128-CBC 解密 `encrypted_value`；12 经 CDP `Network.setCookies` 写入目标 Space 的 partition；13–15 更新计数与状态，推送进度与结果。
- 失败处理：用户拒绝 Keychain 授权 → 状态 `partial`，书签与历史已导入，Cookie 为 0，结果里带警告；源浏览器运行中的 SQLite 锁因先拷贝文件而不受影响；解析失败的单条记录跳过并计数到 `error`，不中断整个导入；同一来源重复导入按 URL + 标题去重。
- 代码入口：TBD — `alter:crates/alterd/src/import/{chromium.rs,keychain.rs,cookies.rs}`、`alter:apps/browser/src/renderer/import-wizard/`、`alter:crates/alter-cli/src/import.rs`。

## 流程：app 启动并托管 alterd
<!-- dflow:diagram assets/seq-app-startup.mmd -->
```mermaid
sequenceDiagram
  autonumber
  participant M as Electron Main
  participant D as alterd
  participant H as ALTER_HOME
  participant R as Renderer
  M->>M: 解析 alterd 路径<br/>ALTER_ALTERD_PATH, resources/bin, target/debug
  alt 二进制不存在
    M-->>R: daemon.status missing
    Note over R: 显示「alterd 未找到」, 窗口保持打开
  else 找到二进制
    M->>D: spawn alterd run, 透传 ALTER_HOME
    D->>H: 对 alterd.lock 取排他锁
    alt 锁被另一个 alterd 持有
      D-->>M: 退出码 3, JSON 日志 already running
      M-->>R: daemon.status exited
    else 取到锁
      D->>H: Store::open, alter.db 迁移到 version 1
      D->>H: 覆盖写 alterd.json (version, pid, startedAt, dbPath)
      loop 每 100 ms, 直到看见健康文件或子进程退出
        M->>H: 读取 alterd.json
      end
      M-->>R: daemon.status running, pid
      Note over R: 显示「alterd 运行中」
    end
  end
  M->>D: app 退出, 发送 SIGTERM
  D->>H: 删除 alterd.json
  D-->>M: 退出码 0 (2 s 内, 超时则 SIGKILL)
```
<!-- /dflow:diagram -->

- 触发：用户打开 alter.app（开发时为 `bun run dev`）。
- 步骤：1 Electron Main 依次在 `ALTER_ALTERD_PATH`、`process.resourcesPath/bin/alterd`、`<repo>/target/debug/alterd` 找 alterd；2 找不到时推 `daemon.status missing`，窗口保持打开；3–4 找到就 spawn `alterd run`，alterd 先对 `alterd.lock` 取排他锁；5–6 锁被占用时 alterd 以退出码 3 退出，Main 推 `exited`；7–8 取到锁后 `Store::open` 迁移 `alter.db`，覆盖写 `alterd.json`；9–10 Main 每 100 ms 读一次健康文件，不设截止时间，直到看到它或子进程退出，然后推 `running` 与 pid；11–13 app 退出时 Main 拦下 `before-quit`，发 SIGTERM，alterd 删除健康文件并以 0 退出，2 秒内没退出则 SIGKILL 并删除 pid 匹配的健康文件。
- 失败处理：alterd 的 stderr 经管道由 Main 转发，不继承主进程 stdio，避免拖住 app 退出；alterd 其他错误（打开数据库失败等）以退出码 1 退出并写 `code=1` 的 JSON 日志，Main 推 `exited`。app 被强制结束时 alterd 不会收到 SIGTERM，修复见进行中的需求 `fix-orphan-alterd`。
- 代码入口：`alter:apps/browser/src/main/daemon.ts`（`resolveAlterdPath`、`DaemonSupervisor`）、`alter:apps/browser/src/main/index.ts`（`before-quit`）、`alter:crates/alterd/src/lifecycle.rs`（`run`）、`alter:crates/alter-core/src/health.rs`。

## 未展开的流程
- 自动归档与 Peek、分屏布局：纯 Electron + Renderer 内的交互，无跨进程契约，在各自需求的 design.md 里画图。
- `install.sh` 与 `alter upgrade`：线性脚本，见 api.md 的 CLI 表和发布需求的 design.md。
