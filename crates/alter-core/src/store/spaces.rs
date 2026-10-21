//! spaces 表的读写。

use rusqlite::params;

use super::Store;

/// 一个未归档的 Space。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceRow {
    pub id: String,
    pub name: String,
    pub color: String,
}

impl Store {
    /// 没有任何未归档的 Space 时，建默认 Space「Personal」；返回是否新建了。
    pub fn ensure_default_space(&self, id: &str, now: i64) -> rusqlite::Result<bool> {
        let open: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM spaces WHERE archived_at IS NULL",
            [],
            |r| r.get(0),
        )?;
        if open > 0 {
            return Ok(false);
        }
        // color 用表的默认值 #6366f1；position 排在所有（含已归档的）Space 之后
        self.conn.execute(
            "INSERT INTO spaces (id, name, position, created_at)
             VALUES (?1, 'Personal', (SELECT COALESCE(MAX(position), -1) + 1 FROM spaces), ?2)",
            params![id, now],
        )?;
        Ok(true)
    }

    /// 未归档的 Space 是否存在。
    pub fn space_exists(&self, id: &str) -> rusqlite::Result<bool> {
        self.conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM spaces WHERE id = ?1 AND archived_at IS NULL)",
            [id],
            |r| r.get(0),
        )
    }

    /// 全部未归档的 Space，按 position 升序。
    pub fn list_spaces(&self) -> rusqlite::Result<Vec<SpaceRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, color FROM spaces WHERE archived_at IS NULL ORDER BY position",
        )?;
        stmt.query_map([], |r| {
            Ok(SpaceRow {
                id: r.get(0)?,
                name: r.get(1)?,
                color: r.get(2)?,
            })
        })?
        .collect()
    }
}
