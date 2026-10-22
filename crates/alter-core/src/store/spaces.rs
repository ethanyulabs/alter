//! spaces 表的读写。

use rusqlite::{OptionalExtension, params};

use super::Store;

/// `delete_space` 的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpaceDeletion {
    Deleted,
    NotFound,
    /// 只剩这一个未归档的 Space，不能删
    LastSpace,
}

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

    /// 在全部 Space 末尾新建一个，并让它成为当前 Space（`last_active_at` 严格最大）。
    pub fn create_space(
        &self,
        id: &str,
        name: &str,
        color: &str,
        now: i64,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO spaces (id, name, color, position, created_at, last_active_at)
             VALUES (?1, ?2, ?3, (SELECT COALESCE(MAX(position), -1) + 1 FROM spaces), ?4,
                     MAX(?4, (SELECT COALESCE(MAX(last_active_at), 0) + 1 FROM spaces)))",
            params![id, name, color, now],
        )?;
        Ok(())
    }

    /// 更新未归档 Space 的 name / color 中给出的字段。
    /// Space 不存在返回 `None`；否则返回是否真的有值变化。
    pub fn update_space(
        &self,
        id: &str,
        name: Option<&str>,
        color: Option<&str>,
    ) -> rusqlite::Result<Option<bool>> {
        if !self.space_exists(id)? {
            return Ok(None);
        }
        let changed = self.conn.execute(
            "UPDATE spaces SET name = COALESCE(?2, name), color = COALESCE(?3, color)
             WHERE id = ?1 AND archived_at IS NULL
               AND (name IS NOT COALESCE(?2, name) OR color IS NOT COALESCE(?3, color))",
            params![id, name, color],
        )?;
        Ok(Some(changed > 0))
    }

    /// 让未归档的 Space 成为当前 Space（`last_active_at` 严格最大）。不存在返回 `false`。
    pub fn activate_space(&self, id: &str, now: i64) -> rusqlite::Result<bool> {
        let changed = self.conn.execute(
            "UPDATE spaces SET last_active_at = MAX(?2, (
                 SELECT COALESCE(MAX(o.last_active_at), 0) + 1 FROM spaces o WHERE o.id <> spaces.id))
             WHERE id = ?1 AND archived_at IS NULL",
            params![id, now],
        )?;
        Ok(changed > 0)
    }

    /// 硬删 Space，外键级联删除它的标签等数据。删的是当前 Space 时，先让后面相邻的
    /// Space 成为当前，没有后面的就取前面相邻的。
    pub fn delete_space(&self, id: &str, now: i64) -> rusqlite::Result<SpaceDeletion> {
        let tx = self.conn.unchecked_transaction()?;
        let position: Option<i64> = tx
            .query_row(
                "SELECT position FROM spaces WHERE id = ?1 AND archived_at IS NULL",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        let Some(position) = position else {
            return Ok(SpaceDeletion::NotFound);
        };
        let open: i64 = tx.query_row(
            "SELECT COUNT(*) FROM spaces WHERE archived_at IS NULL",
            [],
            |r| r.get(0),
        )?;
        if open <= 1 {
            return Ok(SpaceDeletion::LastSpace);
        }
        if self.active_space_id()?.as_deref() == Some(id) {
            let neighbor: String = tx.query_row(
                "SELECT id FROM spaces WHERE archived_at IS NULL AND id <> ?1
                 ORDER BY position > ?2 DESC, ABS(position - ?2) LIMIT 1",
                params![id, position],
                |r| r.get(0),
            )?;
            tx.execute(
                "UPDATE spaces SET last_active_at = MAX(?2, (
                     SELECT COALESCE(MAX(last_active_at), 0) + 1 FROM spaces))
                 WHERE id = ?1",
                params![neighbor, now],
            )?;
        }
        tx.execute("DELETE FROM spaces WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(SpaceDeletion::Deleted)
    }

    /// 当前 Space：`last_active_at` 最大的未归档 Space，全为 NULL 时取 position 最小的。
    pub fn active_space_id(&self) -> rusqlite::Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT id FROM spaces WHERE archived_at IS NULL
                 ORDER BY last_active_at IS NULL, last_active_at DESC, position LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()
    }

    /// 库里全部 Space 的 id（含已归档的）。
    pub fn list_space_ids(&self) -> rusqlite::Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT id FROM spaces")?;
        stmt.query_map([], |r| r.get(0))?.collect()
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
