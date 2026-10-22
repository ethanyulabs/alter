//! 导入记录：每次导入一行，记状态与各类计数。

use rusqlite::{OptionalExtension, params};

use super::Store;

/// `imports` 表的一行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRow {
    pub id: i64,
    pub status: String,
    pub bookmarks: i64,
    pub history: i64,
    pub cookies: i64,
    pub error: Option<String>,
}

impl Store {
    /// 记一次开始的导入（`running`），返回它的 id。
    pub fn create_import(
        &self,
        space_id: &str,
        source: &str,
        profile_path: &str,
        now: i64,
    ) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO imports (space_id, source, profile_path, status, started_at)
             VALUES (?1, ?2, ?3, 'running', ?4)",
            params![space_id, source, profile_path, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// 导入结束：写最终状态、计数、错误与结束时间。返回是否恰好更新了一行（导入行已随目标 Space 被删除时为 false）。
    pub fn finish_import(
        &self,
        id: i64,
        status: &str,
        counts: [i64; 3],
        error: Option<&str>,
        now: i64,
    ) -> rusqlite::Result<bool> {
        let n = self.conn.execute(
            "UPDATE imports SET status = ?2, bookmarks_count = ?3, history_count = ?4,
               cookies_count = ?5, error = ?6, finished_at = ?7 WHERE id = ?1",
            params![id, status, counts[0], counts[1], counts[2], error, now],
        )?;
        Ok(n == 1)
    }

    /// 启动时调用：上次退出时还在进行的导入（`running`）已经没有任务在做，记为 `failed`，`error` 为 `interrupted`。
    /// 返回改了几行。
    pub fn fail_interrupted_imports(&self, now: i64) -> rusqlite::Result<usize> {
        self.conn.execute(
            "UPDATE imports SET status = 'failed', error = 'interrupted', finished_at = ?1
             WHERE status = 'running'",
            [now],
        )
    }

    /// 编号为 `id` 的导入记录。
    pub fn import_row(&self, id: i64) -> rusqlite::Result<Option<ImportRow>> {
        self.conn
            .query_row(
                "SELECT id, status, bookmarks_count, history_count, cookies_count, error
                 FROM imports WHERE id = ?1",
                [id],
                |r| {
                    Ok(ImportRow {
                        id: r.get(0)?,
                        status: r.get(1)?,
                        bookmarks: r.get(2)?,
                        history: r.get(3)?,
                        cookies: r.get(4)?,
                        error: r.get(5)?,
                    })
                },
            )
            .optional()
    }
}
