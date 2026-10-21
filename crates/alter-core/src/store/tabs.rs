//! tabs 表的读写。

use rusqlite::{OptionalExtension, params};

use super::Store;

/// 一个未归档的标签。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabRow {
    pub id: String,
    pub space_id: String,
    pub url: String,
    pub title: String,
    pub favicon_url: Option<String>,
    pub last_active_at: i64,
}

impl Store {
    /// 全部未归档 Space 里的未归档标签：先按 Space 的 position，再按标签的 position 升序。
    pub fn list_open_tabs(&self) -> rusqlite::Result<Vec<TabRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.space_id, t.url, t.title, t.favicon_url, t.last_active_at
             FROM tabs t JOIN spaces s ON s.id = t.space_id
             WHERE t.archived_at IS NULL AND s.archived_at IS NULL
             ORDER BY s.position, t.position",
        )?;
        stmt.query_map([], |r| {
            Ok(TabRow {
                id: r.get(0)?,
                space_id: r.get(1)?,
                url: r.get(2)?,
                title: r.get(3)?,
                favicon_url: r.get(4)?,
                last_active_at: r.get(5)?,
            })
        })?
        .collect()
    }

    /// 未归档的标签是否存在。
    pub fn tab_exists(&self, id: &str) -> rusqlite::Result<bool> {
        self.conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM tabs WHERE id = ?1 AND archived_at IS NULL)",
            [id],
            |r| r.get(0),
        )
    }

    /// 在 Space 末尾新建标签并让它成为激活标签：
    /// `last_active_at` 取 `now` 与同 Space 未归档标签最大值 + 1 中较大的，保证严格最大。
    pub fn insert_tab(
        &self,
        id: &str,
        space_id: &str,
        url: &str,
        now: i64,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO tabs (id, space_id, url, position, last_active_at)
             VALUES (?1, ?2, ?3,
                     (SELECT COALESCE(MAX(position), -1) + 1 FROM tabs WHERE space_id = ?2),
                     MAX(?4, (SELECT COALESCE(MAX(last_active_at), 0) + 1 FROM tabs
                              WHERE space_id = ?2 AND archived_at IS NULL)))",
            params![id, space_id, url, now],
        )?;
        Ok(())
    }

    /// 更新未归档标签的 url / title / favicon_url 中给出的字段。
    /// 标签不存在（或已归档）返回 `None`；否则返回是否真的有值变化。
    pub fn update_tab(
        &self,
        id: &str,
        url: Option<&str>,
        title: Option<&str>,
        favicon_url: Option<&str>,
    ) -> rusqlite::Result<Option<bool>> {
        if !self.tab_exists(id)? {
            return Ok(None);
        }
        let changed = self.conn.execute(
            "UPDATE tabs SET url = COALESCE(?2, url), title = COALESCE(?3, title),
                             favicon_url = COALESCE(?4, favicon_url)
             WHERE id = ?1 AND archived_at IS NULL
               AND (url IS NOT COALESCE(?2, url) OR title IS NOT COALESCE(?3, title)
                    OR favicon_url IS NOT COALESCE(?4, favicon_url))",
            params![id, url, title, favicon_url],
        )?;
        Ok(Some(changed > 0))
    }

    /// 让未归档的标签成为所在 Space 的激活标签：`last_active_at` 取 `now` 与
    /// 同 Space 其他未归档标签最大值 + 1 中较大的。标签不存在返回 `false`。
    pub fn activate_tab(&self, id: &str, now: i64) -> rusqlite::Result<bool> {
        let changed = self.conn.execute(
            "UPDATE tabs SET last_active_at = MAX(?2, (
                 SELECT COALESCE(MAX(o.last_active_at), 0) + 1 FROM tabs o
                 WHERE o.space_id = tabs.space_id AND o.archived_at IS NULL AND o.id <> tabs.id))
             WHERE id = ?1 AND archived_at IS NULL",
            params![id, now],
        )?;
        Ok(changed > 0)
    }

    /// 关闭（归档）未归档的标签。若它是所在 Space 的激活标签，则激活它下方相邻的标签，
    /// 没有下方标签时激活上方相邻的。标签不存在返回 `false`。
    pub fn close_tab(&self, id: &str, now: i64) -> rusqlite::Result<bool> {
        let tx = self.conn.unchecked_transaction()?;
        let row: Option<(String, i64, i64)> = tx
            .query_row(
                "SELECT space_id, position, last_active_at FROM tabs
                 WHERE id = ?1 AND archived_at IS NULL",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let Some((space_id, position, last_active_at)) = row else {
            return Ok(false);
        };
        let latest: i64 = tx.query_row(
            "SELECT MAX(last_active_at) FROM tabs WHERE space_id = ?1 AND archived_at IS NULL",
            [&space_id],
            |r| r.get(0),
        )?;
        tx.execute(
            "UPDATE tabs SET archived_at = ?2 WHERE id = ?1",
            params![id, now],
        )?;
        if last_active_at == latest {
            let neighbor: Option<String> = tx
                .query_row(
                    "SELECT id FROM tabs WHERE space_id = ?1 AND archived_at IS NULL
                     ORDER BY position > ?2 DESC, ABS(position - ?2) LIMIT 1",
                    params![space_id, position],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(next) = neighbor {
                tx.execute(
                    "UPDATE tabs SET last_active_at = MAX(?2, (
                         SELECT COALESCE(MAX(last_active_at), 0) + 1 FROM tabs
                         WHERE space_id = ?3 AND archived_at IS NULL))
                     WHERE id = ?1",
                    params![next, now, space_id],
                )?;
            }
        }
        tx.commit()?;
        Ok(true)
    }
}
