-- 迁移 0001：初始表结构，与 .dev-flow/infra/data-model.md 的「表结构」一致。
-- PRAGMA journal_mode / foreign_keys 不在这里：它们由 Store::open 在迁移事务之外设置。

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
