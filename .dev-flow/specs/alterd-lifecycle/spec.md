# alterd-lifecycle Specification

## Purpose
守护进程的启动、单实例与退出；健康文件是壳与 CLI 判断它是否在运行的唯一依据。

## Requirements

### Requirement: 守护进程启动与退出
系统 SHALL 在 `alterd run` 启动后把进程信息写入数据目录的健康文件 `alterd.json`，并在收到终止信号后删除它再退出。

#### Scenario: 启动后写出健康文件
- WHEN `ALTER_HOME` 指向一个空的临时目录，启动 `alterd run`
- THEN 1 秒内出现 `<ALTER_HOME>/alterd.json`
- AND 文件是 JSON，`pid` 等于该进程的 pid，`version` 为 `0.1.0`，`dbPath` 为 `<ALTER_HOME>/alter.db`，`startedAt` 是毫秒级 Unix 时间戳整数
- AND `<ALTER_HOME>/alter.db` 存在，且 `SELECT MAX(version) FROM schema_version` 返回 1

#### Scenario: SIGTERM 后清理退出
- WHEN 一个已写出健康文件的 `alterd run` 收到 SIGTERM
- THEN 进程在 2 秒内退出，退出码为 0
- AND `<ALTER_HOME>/alterd.json` 已被删除

#### Scenario: 拒绝第二个实例
- WHEN 同一个 `ALTER_HOME` 下已有一个 `alterd run` 在运行，再启动一个 `alterd run`
- THEN 第二个进程在 2 秒内退出，退出码为 3
- AND 第二个进程的 stderr 包含 `already running`
- AND 第一个进程仍在运行，`alterd.json` 里的 `pid` 仍是第一个进程的 pid

#### Scenario: 接管残留的健康文件
- WHEN `<ALTER_HOME>/alterd.json` 已存在且其中 `pid` 为 999999（不存在的进程），启动 `alterd run`
- THEN 1 秒内 `alterd.json` 被改写，`pid` 等于新进程的 pid
- AND 启动后 1 秒时新进程仍在运行
