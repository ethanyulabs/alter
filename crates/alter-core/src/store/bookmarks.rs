//! 书签：导入时按层级与顺序写入，重复导入不产生重复。

use rusqlite::{OptionalExtension, params};

use super::Store;

/// 待导入的一个书签条目（按先序排列，上级一定排在下级之前）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedBookmark {
    pub title: String,
    /// 文件夹为 None
    pub url: Option<String>,
    /// 上级在整个列表里的下标；None 为根级
    pub parent: Option<usize>,
}

impl Store {
    /// 在一个事务里写入 `items`（整个列表中从 `offset` 起的一段）到 `space_id`，`source` 记来源。
    /// `ids` 按列表下标记下每个条目的行 id（下级用它找上级），调用方跨批次保留。
    /// 已有的同一条目（同一上级下标题与 URL 都相同；文件夹的 URL 为 NULL）直接沿用，不再写入：
    /// 唯一索引里 NULL 互不相等，根级条目与文件夹靠不住 `INSERT OR IGNORE`，所以一律先查再插。
    pub fn import_bookmarks(
        &self,
        space_id: &str,
        source: &str,
        items: &[ImportedBookmark],
        offset: usize,
        ids: &mut Vec<String>,
        new_id: &mut dyn FnMut() -> String,
    ) -> rusqlite::Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for (i, item) in items.iter().enumerate() {
            debug_assert_eq!(ids.len(), offset + i);
            let parent = item.parent.map(|p| ids[p].clone());
            let existing: Option<String> = tx
                .query_row(
                    "SELECT id FROM bookmarks
                     WHERE space_id = ?1 AND parent_id IS ?2 AND title = ?3 AND url IS ?4 LIMIT 1",
                    params![space_id, parent, item.title, item.url],
                    |r| r.get(0),
                )
                .optional()?;
            let id = match existing {
                Some(id) => id,
                None => {
                    let id = new_id();
                    tx.execute(
                        "INSERT INTO bookmarks (id, space_id, parent_id, title, url, position, source)
                         VALUES (?1, ?2, ?3, ?4, ?5,
                           (SELECT COALESCE(MAX(position) + 1, 0) FROM bookmarks
                            WHERE space_id = ?2 AND parent_id IS ?3),
                           ?6)",
                        params![id, space_id, parent, item.title, item.url, source],
                    )?;
                    id
                }
            };
            ids.push(id);
        }
        tx.commit()
    }

    /// `space_id` 里标题或 URL 包含 `query`（Unicode 小写折叠后字面包含）的书签（不含文件夹），
    /// 按写入顺序，至多 `limit` 条：(url, title)。
    pub fn search_bookmarks(
        &self,
        space_id: &str,
        query: &str,
        limit: usize,
    ) -> rusqlite::Result<Vec<(String, String)>> {
        let needle = query.to_lowercase();
        let mut stmt = self.conn.prepare(
            "SELECT url, title FROM bookmarks
             WHERE space_id = ?1 AND url IS NOT NULL
               AND (instr(alter_fold(title), ?2) > 0 OR instr(alter_fold(url), ?2) > 0)
             ORDER BY rowid LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![space_id, needle, limit as i64], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?;
        rows.collect()
    }
}
