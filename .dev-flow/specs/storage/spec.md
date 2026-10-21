# storage Specification

## Purpose
SQLite 单文件存储，alterd 是唯一写者。

## Requirements

### Requirement: 数据库初始化与迁移
系统 SHALL 在打开数据目录时创建 SQLite 数据库文件并应用全部尚未应用的迁移，使表结构与 `data-model.md` 一致。

#### Scenario: 空目录首次打开
- WHEN `ALTER_HOME` 指向一个空的临时目录，并打开存储
- THEN `<ALTER_HOME>/alter.db` 文件存在
- AND `sqlite_master` 中存在 `schema_version`、`spaces`、`tabs`、`bookmarks`、`history`、`task_spaces`、`task_pages`、`imports` 共 8 张表
- AND `SELECT MAX(version) FROM schema_version` 返回 1

#### Scenario: 重复打开不重复迁移
- WHEN 同一个 `ALTER_HOME` 下的存储已经打开并迁移过一次，再次打开
- THEN 打开成功，没有错误
- AND `SELECT COUNT(*) FROM schema_version` 返回 1

#### Scenario: WAL 与外键生效
- WHEN 存储打开后执行 `PRAGMA journal_mode` 与 `PRAGMA foreign_keys`
- THEN 两者分别返回 `wal` 与 `1`
- AND 向 `tabs` 插入一行 `space_id` 为 `no-such-space` 的记录被拒绝，错误信息包含 `FOREIGN KEY constraint failed`
