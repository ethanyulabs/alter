# 数据模型

<!-- dflow:diagram assets/er-overview.mmd -->
```mermaid
erDiagram
  spaces ||--o{ tabs : "space_id"
  spaces ||--o{ bookmarks : "space_id"
  spaces ||--o{ history : "space_id"
  spaces ||--o{ task_spaces : "space_id"
  spaces ||--o{ imports : "space_id"
  task_spaces ||--o{ task_pages : "task_space_id"
  tabs |o--o| task_pages : "tab_id"
  spaces {
    text id PK "ULID；partition 名为 persist:space-<id>"
    text name "显示名"
    text color "侧边栏主题色"
    integer position "排序"
    integer created_at "UTC 毫秒"
    integer archived_at "归档时间，NULL 表示活跃"
    integer last_active_at "0002：最大者为当前 Space"
  }
  tabs {
    text id PK "ULID"
    text space_id FK
    text url
    text title
    text favicon_url
    integer pinned "0/1"
    text pinned_url "0002：固定时的 URL，关闭后回到它"
    integer position
    text split_group "同组标签并排显示，NULL 单页"
    integer last_active_at "UTC 毫秒"
    integer archived_at "自动归档时间，NULL 活跃"
  }
  bookmarks {
    text id PK
    text space_id FK
    text parent_id "文件夹，NULL 根"
    text title
    text url "文件夹为 NULL"
    integer position
    text source "manual|chrome|arc"
  }
  history {
    integer id PK
    text space_id FK
    text url
    text title
    integer visited_at "UTC 毫秒"
    text source "alter|chrome|arc"
  }
  task_spaces {
    integer id PK "自增，CLI 用它 resume"
    text space_id FK
    text name
    text ownership "agent|user"
    text status "active|finished|inactive"
    integer page_budget
    integer created_at
    integer finished_at
  }
  task_pages {
    integer id PK
    integer task_space_id FK
    text label "p1 p2 … 在 TaskSpace 内唯一"
    text tab_id FK
    text opened_by "agent|user|unknown"
    integer closed_at
  }
  imports {
    integer id PK
    text space_id FK
    text source "chrome|arc"
    text profile_path
    text status "running|done|partial|failed"
    integer bookmarks_count
    integer history_count
    integer cookies_count
    text error
    integer started_at
    integer finished_at
  }
  schema_version {
    integer version PK
    integer applied_at
  }
```
<!-- /dflow:diagram -->

## 表结构
数据库文件 `~/Library/Application Support/alter/alter.db`，SQLite，WAL 模式，只有 alterd 持有写连接。Cookie、localStorage、缓存不在这里：它们由 Chromium 按 partition 存在 app 的 userData 目录下。

```sql
-- 迁移 0001：初始表结构（crates/alter-core/migrations/0001_init.sql）
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE schema_version (
  version    INTEGER PRIMARY KEY,
  applied_at INTEGER NOT NULL                      -- UTC 毫秒
);

CREATE TABLE spaces (
  id          TEXT PRIMARY KEY,                    -- ULID；partition 名为 persist:space-<id>
  name        TEXT NOT NULL,
  color       TEXT NOT NULL DEFAULT '#6366f1',
  position    INTEGER NOT NULL,
  created_at  INTEGER NOT NULL,
  archived_at INTEGER                              -- NULL 表示活跃
);
CREATE UNIQUE INDEX idx_spaces_position ON spaces(position) WHERE archived_at IS NULL;

CREATE TABLE tabs (
  id             TEXT PRIMARY KEY,                 -- ULID
  space_id       TEXT NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
  url            TEXT NOT NULL,
  title          TEXT NOT NULL DEFAULT '',
  favicon_url    TEXT,
  pinned         INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0, 1)),
  position       INTEGER NOT NULL,
  split_group    TEXT,                             -- 同组并排显示；NULL 单页
  last_active_at INTEGER NOT NULL,
  archived_at    INTEGER                           -- 自动归档时间；NULL 活跃
);
CREATE INDEX idx_tabs_space_active ON tabs(space_id, archived_at, position);
CREATE INDEX idx_tabs_last_active ON tabs(archived_at, pinned, last_active_at);

CREATE TABLE bookmarks (
  id        TEXT PRIMARY KEY,
  space_id  TEXT NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
  parent_id TEXT REFERENCES bookmarks(id) ON DELETE CASCADE,   -- NULL 为根
  title     TEXT NOT NULL,
  url       TEXT,                                  -- 文件夹为 NULL
  position  INTEGER NOT NULL,
  source    TEXT NOT NULL DEFAULT 'manual' CHECK (source IN ('manual', 'chrome', 'arc'))
);
CREATE INDEX idx_bookmarks_parent ON bookmarks(space_id, parent_id, position);
CREATE UNIQUE INDEX idx_bookmarks_dedupe ON bookmarks(space_id, parent_id, url, title) WHERE url IS NOT NULL;

CREATE TABLE history (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  space_id   TEXT NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
  url        TEXT NOT NULL,
  title      TEXT NOT NULL DEFAULT '',
  visited_at INTEGER NOT NULL,
  source     TEXT NOT NULL DEFAULT 'alter' CHECK (source IN ('alter', 'chrome', 'arc'))
);
CREATE INDEX idx_history_space_time ON history(space_id, visited_at DESC);
CREATE UNIQUE INDEX idx_history_dedupe ON history(space_id, url, visited_at);

CREATE TABLE task_spaces (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,   -- CLI 用它 resume
  space_id    TEXT NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
  name        TEXT NOT NULL,
  ownership   TEXT NOT NULL DEFAULT 'agent' CHECK (ownership IN ('agent', 'user')),
  status      TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'finished', 'inactive')),
  page_budget INTEGER NOT NULL DEFAULT 8,
  created_at  INTEGER NOT NULL,
  finished_at INTEGER
);
CREATE INDEX idx_task_spaces_status ON task_spaces(status, space_id);

CREATE TABLE task_pages (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  task_space_id INTEGER NOT NULL REFERENCES task_spaces(id) ON DELETE CASCADE,
  label         TEXT NOT NULL,                     -- p1, p2 …
  tab_id        TEXT REFERENCES tabs(id) ON DELETE SET NULL,
  opened_by     TEXT NOT NULL CHECK (opened_by IN ('agent', 'user', 'unknown')),
  closed_at     INTEGER,
  UNIQUE (task_space_id, label)
);
CREATE UNIQUE INDEX idx_task_pages_tab ON task_pages(tab_id) WHERE tab_id IS NOT NULL AND closed_at IS NULL;

CREATE TABLE imports (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  space_id        TEXT NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
  source          TEXT NOT NULL CHECK (source IN ('chrome', 'arc')),
  profile_path    TEXT NOT NULL,
  status          TEXT NOT NULL CHECK (status IN ('running', 'done', 'partial', 'failed')),
  bookmarks_count INTEGER NOT NULL DEFAULT 0,
  history_count   INTEGER NOT NULL DEFAULT 0,
  cookies_count   INTEGER NOT NULL DEFAULT 0,
  error           TEXT,
  started_at      INTEGER NOT NULL,
  finished_at     INTEGER
);
```

```sql
-- 迁移 0002：记住当前 Space、固定标签的 URL（crates/alter-core/migrations/0002_spaces_tabs.sql，add-core-features N1）
ALTER TABLE spaces ADD COLUMN last_active_at INTEGER;  -- 最大者为当前 Space；全为 NULL 时取 position 最小的
ALTER TABLE tabs ADD COLUMN pinned_url TEXT;           -- 固定时的 URL；关闭固定标签后回到它
UPDATE tabs SET pinned_url = url WHERE pinned = 1;
-- 每个 Space 的未归档标签按 (pinned DESC, position) 重新编号为 0..n-1；已归档的 position 为 -1
```

## 规则
- 一个 Space 一个 partition：partition 名由 `spaces.id` 派生（`persist:space-<id>`），不单独存储。删除 Space 是硬删（级联删除它的标签、书签、历史、TaskSpace、导入记录）；Electron 立即 `clearStorageData` 与清缓存，alterd 下次启动时删除不再对应任何 Space 的 partition 目录（运行中 Chromium 可能还持有文件）。
- `tabs.space_id` 不可变：标签不能在 Space 之间移动（避免 Cookie 隔离被绕过）；需要时复制 URL 新建。
- 激活标签不单独存储：每个 Space 中未归档且 `last_active_at` 最大的标签即激活标签；写入时取「同 Space 最大值 + 1」与当前时间中较大者，保证严格递增。当前 Space 由 `spaces.last_active_at` 最大者决定（迁移 0002），全为 NULL 时取 `position` 最小的；重启后回到上次的 Space。
- 关闭标签即归档：只置 `tabs.archived_at`（`position` 置 -1），不删除行；关闭固定标签不归档，只卸载页面并回到 `pinned_url`。同一 Space 的未归档标签按（固定在前，`position`）连续编号 0..n-1，固定、拖动、关闭时重排。
- 首次启动（没有未归档的 Space）时建默认 Space `Personal`（颜色取列默认值 `#6366f1`）。
- partition 的磁盘目录是 `ALTER_HOME/chromium/Partitions/space-<id 小写>`（Electron 44 实测会转成小写），清理时按小写匹配。
- 固定标签（`pinned = 1`）不参与自动归档；归档只设 `archived_at`，不删除；恢复即清空该字段并追加到列表末尾。
- `split_group` 内的标签必须属于同一 Space，由代码保证；分屏解散即置 NULL。
- `task_pages.label` 在 TaskSpace 内唯一，关闭后不复用；一个活跃标签同一时刻只能被一个 TaskSpace 托管（部分唯一索引）。
- TaskSpace 不跨 Space：`task_pages.tab_id` 指向的标签必须与 `task_spaces.space_id` 一致，由代码在 adopt 时校验。
- 时间戳全部 UTC 毫秒整数；没有软删除之外的删除，`ON DELETE CASCADE` 只在删除 Space 时触发。
- 浏览历史：主框架每次完成导航写一行（`source = alter`，`visited_at` 取当前时间与该 Space 最新一条 + 1 中较大者），之后的标题回写只更新该标签最近一次导航写入的那一行；搜索按 Unicode 小写折叠（SQL 函数 `alter_fold`）后字面包含，按 URL 去重。
- 导入去重：书签按（Space、父节点、标题、URL）先查再插（根级条目与文件夹的 NULL 不能靠唯一索引）；历史按（Space、URL、访问时间）唯一，重复导入是幂等的。`imports` 一次导入一行；alterd 启动时把遗留的 `running` 记为 `failed`（`error = interrupted`）。
- 写入只经 alterd 的单写连接；Electron 和 CLI 不打开数据库文件。
- 迁移：`crates/alter-core/migrations/NNNN_<name>.sql` 按序号执行，每个文件一个事务，执行后写 `schema_version`；alterd 启动时自动迁移，降级不支持。有待执行的迁移时，先把数据库备份为 `alter.db.bak-<迁移前的版本>`（例如从 1 升到 2 时 `alter.db.bak-1`），备份失败则不迁移；全新库或已是最新时不备份。
- 数据目录（`ALTER_HOME`）里除 `alter.db`（及 WAL 文件）外还有 alterd 的健康文件 `alterd.json` 与实例锁 `alterd.lock`，二者都不是数据库内容，格式见 api.md「进程状态」。
- Cookie 导入不改表结构：`imports.cookies_count` 记 Main 回报写入的数；`status = 'partial'` 时 `error` 列存 warning（`keychain_denied` / `shell_unavailable`）。导入的 Cookie 本身只写进 Chromium 的 partition，不进数据库，Cookies 库只读进内存，`import-tmp` 里不留它的副本。
- 不在数据库里的其他落盘内容：`ALTER_HOME/config.toml` 的可选键 `update_check`（false 关闭更新检查）；skill 安装目录里的 `.alter-skill` 标记（内容是安装它的 CLI 版本，不带换行），`alter skill uninstall` 只删带标记的目录，`alter upgrade` 据它刷新。是否有新版本只在 alterd 内存里，重启后重新检查。
