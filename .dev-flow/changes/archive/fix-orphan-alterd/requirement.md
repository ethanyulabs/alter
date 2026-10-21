---
id: fix-orphan-alterd
title: app 异常结束后 alterd 残留，重启显示「已退出」
type: fix
status: archived
revision: 2
approved_revision: 2
repos: [alter]
created: 2026-10-20
skip_specs: false
---

# app 异常结束后 alterd 残留，重启显示「已退出」

## 现象
用户在本机启动 app，窗口里 alterd 的状态显示「alterd 已退出」，而不是「alterd 运行中」。只要 app 曾经没走正常退出流程就结束（崩溃、在活动监视器里强制退出、开发时 `bun run dev` 的 Electron 被直接杀掉），之后每次启动都是这样，直到手动杀掉残留的 alterd 进程。严重程度：高。app 骨架唯一的状态显示是错的，后续依赖 alterd 的功能（网关、标签）都会跟着不可用。从 add-workspace-skeleton 的 T8（app 托管 alterd）起就存在。

## 复现
1. 用一个空的临时目录作 `ALTER_HOME`，启动打包前的 app（`electron apps/browser/out/main/index.js`），等窗口显示「alterd 运行中」。
2. 对 Electron 主进程发 `SIGKILL`。
3. 用同一个 `ALTER_HOME` 再启动一次 app。
- 期望：第 2 步后 alterd 在 2 秒内随之退出并删除 `alterd.json`；第 3 步的窗口显示「alterd 运行中」。
- 实际：第 2 步后 alterd 继续运行（父进程变成 launchd），`alterd.json` 仍在；第 3 步新拉起的 alterd 拿不到 `alterd.lock`，以退出码 3 退出，窗口显示「alterd 已退出」。

## 反馈循环
`ALTER_HOME=$H electron out/main/index.js & ; kill -9 <Electron 主进程 pid>; pgrep -fl "alterd run"; 再用同一 $H 启动 app 并读页面文本`（临时脚本，不入库；修复后由 R2.S1、R3.S1 的 e2e 用例取代）：

```
孤儿 alterd: 59930 /…/target/debug/alterd run
{"level":"WARN","fields":{"message":"alterd: already running (pid 59930)","code":3,"pid":"59930"}}
[text] alteralterd 已退出
```

## 根因
- 假设（按可能性排序，每条给出预测）：
  1. alterd 无从得知父进程已死：它只在收到 SIGTERM / SIGINT 时退出，而 Electron 被 SIGKILL 时 `before-quit` 不会执行，没有人给 alterd 发信号。预测：让 alterd 在 app 持有的管道关闭时退出，第 2 步之后就不会有残留进程。
  2. app 把 alterd 的任何退出都当成「已退出」，包括退出码 3（另一个实例正在运行）。预测：退出码 3 时改为读 `alterd.json` 并确认 pid 存活，第 3 步就会显示「alterd 运行中」，哪怕残留进程还在。
  3. 崩溃后 `alterd.lock` 没被释放，新实例被锁住。预测：直接 `kill -9` alterd 本身后再启动，仍会退出码 3。已证伪：`flock` 随进程结束释放，R3.S4（残留健康文件）用例和手动实验里新实例都能正常启动。
  4. （T2 执行中发现）宿主被强制结束后 alterd 的 stderr 管道也断了，退出路径上写日志失败，tracing-subscriber 默认用 `eprintln!` 报告这个内部错误，`eprintln!` 写 stderr 失败即 panic，跳过删除健康文件。预测：同时关闭 alterd 的 stdin 与 stderr，退出码为 101 且健康文件仍在。
- 已确认：假设 1、2、4 同时成立。
  - 假设 1：`apps/browser/src/main/daemon.ts:65` 以 `stdio: ["ignore", "ignore", "pipe"]` 启动 alterd，alterd 的 stdin 不连着 app。`crates/alterd/src/lifecycle.rs:74` 的 `tokio::select!` 只等 SIGTERM / SIGINT，所以父进程被 SIGKILL 后它一直运行。
  - 假设 2：`daemon.ts:71-72` 的 `exit` 处理对任何退出码都报 `exited`。
  - 假设 4：Python 拉起 `alterd run --exit-on-stdin-close` 后同时关闭 stdin 与 stderr，returncode 101、健康文件仍在；源码见 tracing-subscriber 0.3.23 `src/fmt/fmt_layer.rs:1051`。

## 修复范围
- alterd：`run` 新增参数 `--exit-on-stdin-close`。带上它时，stdin 读到 EOF（写端关闭，包括持有写端的进程被 SIGKILL）或读取出错（宿主通道已不可用），就按收到 SIGTERM 的流程退出：删除 `alterd.json`，退出码 0。不带它时行为不变，终端里手动运行、stdin 是 `/dev/null` 都不受影响。
- alterd：日志写 stderr 失败时不再报告内部错误（`log_internal_errors(false)`），stderr 管道断开不会让退出清理 panic。
- app：spawn alterd 时把 stdin 改成管道并带上 `--exit-on-stdin-close`，整个运行期间不关闭写端。
- app：alterd 以退出码 3 退出时，读 `alterd.json`。其中 pid 存活就复用这个实例，状态报 `running` 和它的 pid；app 退出时不去停一个不是自己拉起的实例。读不到文件或 pid 已死时，仍报 `exited`。
- 刻意不动：
  - 单实例判定，仍用 `alterd.lock` 排他锁。
  - 健康文件格式。
  - 正常退出路径，`before-quit` 仍发 SIGTERM，2 秒后 SIGKILL。
  - CLI `alter status`。
  - 修复前已经残留的孤儿进程：由上面的「复用」覆盖，不另做清理。

## Spec Delta: alterd-lifecycle

### ADDED Requirement: R1 随宿主退出
系统 SHALL 在 `alterd run --exit-on-stdin-close` 的 stdin 到达 EOF 或读取出错后 2 秒内退出：退出码为 0，并删除 `ALTER_HOME/alterd.json`。不带该参数时，stdin 的状态不影响 alterd 的运行。

#### Scenario: R1.S1 stdin 写端关闭后退出
- WHEN 以空的临时目录为 `ALTER_HOME`，用管道作 stdin 运行 `alterd run --exit-on-stdin-close`，等 `ALTER_HOME/alterd.json` 出现后关闭管道写端
- THEN alterd 在 2 秒内退出，退出码为 0
- AND `ALTER_HOME/alterd.json` 不存在

#### Scenario: R1.S2 不带参数时忽略 stdin
- WHEN 以空的临时目录为 `ALTER_HOME`，stdin 为 `/dev/null`（立即 EOF），运行不带参数的 `alterd run`
- THEN 3 秒后 alterd 仍在运行，`ALTER_HOME/alterd.json` 存在且其中 pid 为该进程

#### Scenario: R1.S3 stdin 读取出错时退出
- WHEN 以空的临时目录为 `ALTER_HOME`，以只读打开的一个目录作 stdin（读取立即返回 EISDIR 错误）运行 `alterd run --exit-on-stdin-close`
- THEN alterd 在 2 秒内退出，退出码为 0
- AND `ALTER_HOME/alterd.json` 不存在

## Spec Delta: app-shell

### ADDED Requirement: R2 app 被强制结束时 alterd 随之退出
系统 SHALL 在 app 主进程被强制结束（SIGKILL）后 2 秒内让它拉起的 alterd 退出，并删除 `ALTER_HOME/alterd.json`。

#### Scenario: R2.S1 SIGKILL app 后 alterd 退出
- WHEN 以空的临时目录为 `ALTER_HOME` 启动 app，窗口显示「alterd 运行中」，记下 `alterd.json` 中的 pid（例如 41237），然后对 app 主进程发送 SIGKILL
- THEN 2 秒内 pid 41237 不再存活
- AND `ALTER_HOME/alterd.json` 不存在

### ADDED Requirement: R3 复用已在运行的 alterd
系统 SHALL 在同一 `ALTER_HOME` 已有一个存活的 alterd 时复用它：页面状态显示「alterd 运行中」与该实例的 pid；app 退出时不终止这个实例。

#### Scenario: R3.S1 已有实例时复用而不报已退出
- WHEN 以空的临时目录为 `ALTER_HOME`，先在 app 之外运行 `alterd run`（pid 例如 52001），等 `alterd.json` 出现后启动 app
- THEN 3 秒内页面文本包含「alterd 运行中」，`daemon.status` 的 pid 为 52001
- AND 正常退出 app 后 2 秒，pid 52001 仍存活，`ALTER_HOME/alterd.json` 仍存在且 pid 为 52001

## 风险与回滚
- 影响范围：
  - alterd 的启动参数，Electron 托管 alterd 的 spawn 方式和 `exit` 处理。
  - 不涉及数据库、协议类型和 CLI。
- `alterd run` 在终端里手动运行不受影响，因为参数默认关闭。
- 读 stdin 的线程会一直阻塞到 EOF，不能放进 tokio 的阻塞线程池：tokio 文档说明 `tokio::io::stdin` 的读取无法取消，会拖住运行时关闭。所以改用独立的 std 线程，进程退出时随之结束。
- 回滚：还原这个需求的提交即可，没有持久化状态的变化。

## 假设
- 用 stdin 管道作「宿主存活」信号，不用轮询 `getppid()` 或 kqueue `NOTE_EXIT`。管道写端随进程结束由内核关闭，SIGKILL 也覆盖得到；不需要轮询，也不引入平台专用代码。
- 复用的实例由 app 外部拉起，app 退出时不终止它：它的生命周期属于拉起它的人（终端里的开发者），与 R7.S2「app 退出时停掉自己拉起的 alterd」不冲突。

## 待确认问题
- 无。

## 澄清记录
### Session 2026-10-21
- Q: 带 `--exit-on-stdin-close` 时，stdin 读取出错（非 EOF）怎么处理？（验证时 codex 指出实现会清理退出，而规格只写了 EOF）→ A: 补进规格：读取出错按 EOF 处理，宿主通道不可用时退出更安全。

## 变更记录
- 2026-10-20: 创建。
- 2026-10-21: amended: R1 补上「stdin 读取出错也退出」并新增场景 R1.S3、任务 T4（验证时发现实现已有此行为而规格未写）；根因补充假设 4（stderr 断开导致退出清理 panic），修复范围补上 `log_internal_errors(false)`; revision: 2; re-approved by the user
