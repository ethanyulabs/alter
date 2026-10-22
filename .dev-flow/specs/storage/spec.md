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
- AND `SELECT MAX(version) FROM schema_version` 返回 2
- AND `spaces` 有 `last_active_at` 列，`tabs` 有 `pinned_url` 列

#### Scenario: 重复打开不重复迁移
- WHEN 同一个 `ALTER_HOME` 下的存储已经打开并迁移过一次，再次打开
- THEN 打开成功，没有错误
- AND `SELECT COUNT(*) FROM schema_version` 返回 2

#### Scenario: WAL 与外键生效
- WHEN 存储打开后执行 `PRAGMA journal_mode` 与 `PRAGMA foreign_keys`
- THEN 两者分别返回 `wal` 与 `1`
- AND 向 `tabs` 插入一行 `space_id` 为 `no-such-space` 的记录被拒绝，错误信息包含 `FOREIGN KEY constraint failed`

#### Scenario: 已有标签按分区重新编号
- WHEN 一个只应用了迁移 0001 的数据库里，Space `Personal` 有未归档标签 A（`position` 0）、B（`position` 5，`pinned` 1）、C（`position` 9），打开存储
- THEN A、B、C 的 `position` 分别为 1、0、2

### Requirement: 迁移前备份
系统 SHALL 在对已有数据库应用新的迁移之前，把数据库完整复制为同目录下的 `alter.db.bak-<迁移前的版本号>`。

#### Scenario: 从版本 1 升级时生成备份
- WHEN `ALTER_HOME/alter.db` 是一个只应用了迁移 0001、含 Space `Old` 的数据库，打开存储
- THEN `ALTER_HOME/alter.db.bak-1` 存在，以 SQLite 打开它时 `SELECT MAX(version) FROM schema_version` 返回 1，`spaces` 中有 `Old`
- AND `alter.db` 的 `SELECT MAX(version) FROM schema_version` 返回 2，`spaces` 中仍有 `Old`

#### Scenario: 新数据库和已是最新的数据库不生成备份
- WHEN `ALTER_HOME` 为空目录，打开存储后关闭，再打开一次
- THEN `ALTER_HOME` 下没有名字以 `alter.db.bak-` 开头的文件
