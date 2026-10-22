//! tabs 表的读写。

use rusqlite::{Connection, OptionalExtension, params};

use super::Store;

/// `close_tab` 的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabClosing {
    /// 普通标签，已归档
    Archived,
    /// 固定标签，行保留、url 已回到 `pinned_url`，页面应当卸载
    Discarded,
    NotFound,
}

/// 一个已归档的标签。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchivedRow {
    pub id: String,
    pub space_id: String,
    pub url: String,
    pub title: String,
    pub archived_at: i64,
}

/// 一个未归档的标签。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabRow {
    pub id: String,
    pub space_id: String,
    pub url: String,
    pub title: String,
    pub favicon_url: Option<String>,
    pub last_active_at: i64,
    pub pinned: bool,
    /// 分屏组（左边标签的 id）；不在分屏里为 None
    pub split_group: Option<String>,
}

impl Store {
    /// 全部未归档 Space 里的未归档标签：先按 Space 的 position，再按标签的 position 升序。
    /// 把 `left`、`right` 组成分屏（两者的 `split_group` 都写成 `left`）；任一已在别的分屏里时，
    /// 先解散那个分屏。调用方保证两者是同一 Space 的不同未归档标签。
    pub fn split(&self, left: &str, right: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE tabs SET split_group = NULL
             WHERE split_group IN (SELECT split_group FROM tabs
                                   WHERE id IN (?1, ?2) AND split_group IS NOT NULL)",
            params![left, right],
        )?;
        self.conn.execute(
            "UPDATE tabs SET split_group = ?1 WHERE id IN (?1, ?2)",
            params![left, right],
        )?;
        Ok(())
    }

    /// 解散 `tab_id` 所在的分屏（同组两个标签都清空）；不在分屏里时什么也不做，返回是否有变化。
    pub fn dissolve_split(&self, tab_id: &str) -> rusqlite::Result<bool> {
        let n = self.conn.execute(
            "UPDATE tabs SET split_group = NULL
             WHERE split_group = (SELECT split_group FROM tabs WHERE id = ?1)",
            [tab_id],
        )?;
        Ok(n > 0)
    }

    /// 自动归档：`last_active_at` 早于 `now - threshold_ms` 的标签归档，固定标签、各 Space 的激活标签
    /// （`last_active_at` 最大的）与被 TaskSpace 托管的标签除外。归档的标签解散所在分屏，所在 Space
    /// 重新连续编号。返回被归档的标签 id。
    pub fn archive_stale(&self, now: i64, threshold_ms: i64) -> rusqlite::Result<Vec<String>> {
        let tx = self.conn.unchecked_transaction()?;
        let ids: Vec<(String, String)> = {
            let mut stmt = tx.prepare(
                "SELECT t.id, t.space_id FROM tabs t
                 WHERE t.archived_at IS NULL AND t.pinned = 0 AND t.last_active_at < ?1 - ?2
                   AND t.last_active_at < (SELECT MAX(o.last_active_at) FROM tabs o
                                           WHERE o.space_id = t.space_id AND o.archived_at IS NULL)
                   AND t.id NOT IN (SELECT tab_id FROM task_pages
                                    WHERE tab_id IS NOT NULL AND closed_at IS NULL)",
            )?;
            stmt.query_map(params![now, threshold_ms], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?
        };
        let mut spaces = std::collections::BTreeSet::new();
        for (id, space_id) in &ids {
            tx.execute(
                "UPDATE tabs SET split_group = NULL
                 WHERE split_group = (SELECT split_group FROM tabs WHERE id = ?1)",
                [id],
            )?;
            tx.execute(
                "UPDATE tabs SET archived_at = ?2 WHERE id = ?1",
                params![id, now],
            )?;
            spaces.insert(space_id.clone());
        }
        for space_id in &spaces {
            let (pinned_ids, normal_ids) = sections_without(&tx, space_id, "")?;
            renumber(&tx, pinned_ids.iter().chain(&normal_ids))?;
        }
        tx.commit()?;
        Ok(ids.into_iter().map(|(id, _)| id).collect())
    }

    /// 每个未归档 Space 最近归档的至多 `per_space` 个标签：先按 Space 的 position，再按归档时间降序。
    pub fn list_archived(&self, per_space: usize) -> rusqlite::Result<Vec<ArchivedRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, space_id, url, title, archived_at FROM (
                 SELECT t.id, t.space_id, t.url, t.title, t.archived_at, s.position AS space_position,
                        ROW_NUMBER() OVER (PARTITION BY t.space_id
                                           ORDER BY t.archived_at DESC, t.rowid DESC) AS n
                 FROM tabs t JOIN spaces s ON s.id = t.space_id AND s.archived_at IS NULL
                 WHERE t.archived_at IS NOT NULL)
             WHERE n <= ?1
             ORDER BY space_position, archived_at DESC, n",
        )?;
        stmt.query_map([per_space as i64], |r| {
            Ok(ArchivedRow {
                id: r.get(0)?,
                space_id: r.get(1)?,
                url: r.get(2)?,
                title: r.get(3)?,
                archived_at: r.get(4)?,
            })
        })?
        .collect()
    }

    /// 恢复一个已归档的标签：放到所在 Space 普通分区末尾并激活。标签不存在、没有归档或所在 Space
    /// 已删除时返回 false。
    pub fn restore_tab(&self, id: &str, now: i64) -> rusqlite::Result<bool> {
        let tx = self.conn.unchecked_transaction()?;
        let space: Option<String> = tx
            .query_row(
                "SELECT t.space_id FROM tabs t JOIN spaces s ON s.id = t.space_id
                 WHERE t.id = ?1 AND t.archived_at IS NOT NULL AND s.archived_at IS NULL",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        let Some(space_id) = space else {
            return Ok(false);
        };
        tx.execute(
            "UPDATE tabs SET archived_at = NULL, pinned = 0, pinned_url = NULL, split_group = NULL
             WHERE id = ?1",
            [id],
        )?;
        let (pinned_ids, mut normal_ids) = sections_without(&tx, &space_id, id)?;
        normal_ids.push(id.to_owned());
        renumber(&tx, pinned_ids.iter().chain(&normal_ids))?;
        tx.execute(
            "UPDATE tabs SET last_active_at = MAX(?2, (
                 SELECT COALESCE(MAX(o.last_active_at), 0) + 1 FROM tabs o
                 WHERE o.space_id = ?3 AND o.archived_at IS NULL AND o.id <> ?1))
             WHERE id = ?1",
            params![id, now, space_id],
        )?;
        tx.commit()?;
        Ok(true)
    }

    /// 一个未归档的标签。
    pub fn tab(&self, id: &str) -> rusqlite::Result<Option<TabRow>> {
        self.conn
            .query_row(
                "SELECT id, space_id, url, title, favicon_url, last_active_at, pinned, split_group
                 FROM tabs WHERE id = ?1 AND archived_at IS NULL",
                [id],
                |r| {
                    Ok(TabRow {
                        id: r.get(0)?,
                        space_id: r.get(1)?,
                        url: r.get(2)?,
                        title: r.get(3)?,
                        favicon_url: r.get(4)?,
                        last_active_at: r.get(5)?,
                        pinned: r.get(6)?,
                        split_group: r.get(7)?,
                    })
                },
            )
            .optional()
    }

    pub fn list_open_tabs(&self) -> rusqlite::Result<Vec<TabRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.space_id, t.url, t.title, t.favicon_url, t.last_active_at, t.pinned,
                    t.split_group
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
                pinned: r.get(6)?,
                split_group: r.get(7)?,
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
                     (SELECT COALESCE(MAX(position), -1) + 1 FROM tabs
                      WHERE space_id = ?2 AND archived_at IS NULL),
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

    /// 关闭未归档的标签。普通标签被归档；固定标签保留在列表里，url 回到固定时的地址，
    /// 页面由调用方卸载。若它是所在 Space 的激活标签，则激活它下方相邻的标签，
    /// 没有下方标签时激活上方相邻的。
    pub fn close_tab(&self, id: &str, now: i64) -> rusqlite::Result<TabClosing> {
        let tx = self.conn.unchecked_transaction()?;
        let row: Option<(String, i64, i64, bool)> = tx
            .query_row(
                "SELECT space_id, position, last_active_at, pinned FROM tabs
                 WHERE id = ?1 AND archived_at IS NULL",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        let Some((space_id, position, last_active_at, pinned)) = row else {
            return Ok(TabClosing::NotFound);
        };
        // 关闭分屏中的标签（含固定标签的卸载）时一并解散分屏：Shell 与 Gateway 的关闭都走这里
        tx.execute(
            "UPDATE tabs SET split_group = NULL
             WHERE split_group = (SELECT split_group FROM tabs WHERE id = ?1)",
            [id],
        )?;
        let latest: i64 = tx.query_row(
            "SELECT MAX(last_active_at) FROM tabs WHERE space_id = ?1 AND archived_at IS NULL",
            [&space_id],
            |r| r.get(0),
        )?;
        if pinned {
            tx.execute(
                "UPDATE tabs SET url = COALESCE(pinned_url, url) WHERE id = ?1",
                [id],
            )?;
        } else {
            tx.execute(
                "UPDATE tabs SET archived_at = ?2 WHERE id = ?1",
                params![id, now],
            )?;
        }
        if last_active_at == latest {
            let neighbor: Option<String> = tx
                .query_row(
                    "SELECT id FROM tabs WHERE space_id = ?1 AND archived_at IS NULL AND id <> ?3
                     ORDER BY position > ?2 DESC, ABS(position - ?2) LIMIT 1",
                    params![space_id, position, id],
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
        if !pinned {
            // 归档后补上空出的编号；放在选相邻标签之后，那一步要用关闭前的位置
            let (pinned_ids, normal_ids) = sections_without(&tx, &space_id, id)?;
            renumber(&tx, pinned_ids.iter().chain(&normal_ids))?;
        }
        tx.commit()?;
        Ok(if pinned {
            TabClosing::Discarded
        } else {
            TabClosing::Archived
        })
    }

    /// 固定或取消固定未归档的标签：固定时移到固定分区末尾并记下当前 url，取消时移到
    /// 普通分区开头并清空 `pinned_url`。标签不存在返回 `None`；否则返回是否真的有变化。
    pub fn set_pinned(&self, id: &str, pinned: bool) -> rusqlite::Result<Option<bool>> {
        let tx = self.conn.unchecked_transaction()?;
        let Some((space_id, was_pinned)) = tab_pin_state(&tx, id)? else {
            return Ok(None);
        };
        if was_pinned == pinned {
            return Ok(Some(false));
        }
        let (mut pinned_ids, mut normal_ids) = sections_without(&tx, &space_id, id)?;
        if pinned {
            pinned_ids.push(id.to_owned());
        } else {
            normal_ids.insert(0, id.to_owned());
        }
        set_pin_flag(&tx, id, pinned)?;
        renumber(&tx, pinned_ids.iter().chain(&normal_ids))?;
        tx.commit()?;
        Ok(Some(true))
    }

    /// 把未归档的标签移到目标分区（`pinned`）的第 `index` 位，超出时放到末尾；
    /// 跨分区时随之固定或取消固定。标签不存在返回 `false`。
    pub fn move_tab(&self, id: &str, pinned: bool, index: usize) -> rusqlite::Result<bool> {
        let tx = self.conn.unchecked_transaction()?;
        let Some((space_id, was_pinned)) = tab_pin_state(&tx, id)? else {
            return Ok(false);
        };
        let (mut pinned_ids, mut normal_ids) = sections_without(&tx, &space_id, id)?;
        let target = if pinned {
            &mut pinned_ids
        } else {
            &mut normal_ids
        };
        target.insert(index.min(target.len()), id.to_owned());
        if was_pinned != pinned {
            set_pin_flag(&tx, id, pinned)?;
        }
        renumber(&tx, pinned_ids.iter().chain(&normal_ids))?;
        tx.commit()?;
        Ok(true)
    }
}

/// 未归档标签的 (space_id, pinned)。
fn tab_pin_state(conn: &Connection, id: &str) -> rusqlite::Result<Option<(String, bool)>> {
    conn.query_row(
        "SELECT space_id, pinned FROM tabs WHERE id = ?1 AND archived_at IS NULL",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
}

/// 某 Space 未归档标签按当前顺序分成 (固定分区, 普通分区)，不含 `except`。
fn sections_without(
    conn: &Connection,
    space_id: &str,
    except: &str,
) -> rusqlite::Result<(Vec<String>, Vec<String>)> {
    let mut stmt = conn.prepare(
        "SELECT id, pinned FROM tabs WHERE space_id = ?1 AND archived_at IS NULL AND id <> ?2
         ORDER BY pinned DESC, position",
    )?;
    let rows = stmt.query_map(params![space_id, except], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?))
    })?;
    let (mut pinned, mut normal) = (Vec::new(), Vec::new());
    for row in rows {
        let (id, is_pinned) = row?;
        if is_pinned { &mut pinned } else { &mut normal }.push(id);
    }
    Ok((pinned, normal))
}

/// 改固定标记：固定时 `pinned_url` 取当前 url，取消时清空。
fn set_pin_flag(conn: &Connection, id: &str, pinned: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE tabs SET pinned = ?2, pinned_url = CASE WHEN ?2 THEN url ELSE NULL END
         WHERE id = ?1",
        params![id, pinned],
    )?;
    Ok(())
}

/// 按给定顺序把标签的 position 写成 0..n-1。
fn renumber<'a>(
    conn: &Connection,
    ordered: impl Iterator<Item = &'a String>,
) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare("UPDATE tabs SET position = ?2 WHERE id = ?1")?;
    for (pos, id) in ordered.enumerate() {
        stmt.execute(params![id, pos as i64])?;
    }
    Ok(())
}
