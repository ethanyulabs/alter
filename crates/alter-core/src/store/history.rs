//! 浏览历史：主框架每次完成导航追加一条（`source = 'alter'`），之后的标题回写按该次导航写入的行 id 更新
//! （由 alterd 记住标签 → 行 id）。搜索按 Unicode 小写折叠后字面包含匹配。

use rusqlite::params;

use super::Store;

impl Store {
    /// 在 `space_id` 的历史里追加一条访问。`visited_at` 取 `now` 与该 Space 最新一条 + 1 中较大的：
    /// 同一毫秒内的多次访问也保持先后顺序，且不撞 `(space_id, url, visited_at)` 唯一索引。返回新行的 id。
    pub fn add_history(
        &self,
        space_id: &str,
        url: &str,
        title: &str,
        now: i64,
    ) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO history (space_id, url, title, visited_at, source)
             VALUES (?1, ?2, ?3, MAX(?4, (SELECT COALESCE(MAX(visited_at), 0) + 1 FROM history WHERE space_id = ?1)), 'alter')",
            params![space_id, url, title, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// 标题回写：更新编号为 `id` 的那条历史（该标签最近一次导航写入的那一行）的标题。
    pub fn update_history_title(&self, id: i64, title: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE history SET title = ?2 WHERE id = ?1",
            params![id, title],
        )?;
        Ok(())
    }

    /// 标题或 URL 包含 `query`（Unicode 小写折叠后字面包含）的历史，按 URL 去重取最近一次访问，
    /// 跳过 `exclude` 里的 URL（已打开的标签），按最近访问降序，至多 `limit` 条：(url, title)。
    pub fn search_history(
        &self,
        query: &str,
        exclude: &[String],
        limit: usize,
    ) -> rusqlite::Result<Vec<(String, String)>> {
        // 与标签的匹配一致：Unicode 小写折叠后按字面包含（SQLite 的 LIKE 只对 ASCII 忽略大小写，且有通配符）
        let needle = query.to_lowercase();
        // 一次扫描按 URL 分组：SQLite 聚合查询里与 MAX() 同在的裸列取的是 MAX 那一行，
        // 所以 title 是该 URL 最近一次访问的标题
        let mut stmt = self.conn.prepare(
            "SELECT h.url, h.title, MAX(h.visited_at) AS last FROM history h
             JOIN spaces s ON s.id = h.space_id AND s.archived_at IS NULL
             WHERE instr(alter_fold(h.title), ?1) > 0 OR instr(alter_fold(h.url), ?1) > 0
             GROUP BY h.url
             ORDER BY last DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![needle, (limit + exclude.len()) as i64], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (url, title) = row?;
            if !exclude.contains(&url) {
                out.push((url, title));
            }
            if out.len() == limit {
                break;
            }
        }
        Ok(out)
    }
}

impl Store {
    /// 在一个事务里写入导入的访问 (url, title, visited_at)，`source` 记来源；
    /// 同一 Space 里 (url, visited_at) 相同的已有访问由唯一索引跳过。
    pub fn import_history(
        &self,
        space_id: &str,
        source: &str,
        visits: &[(String, String, i64)],
    ) -> rusqlite::Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR IGNORE INTO history (space_id, url, title, visited_at, source)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for (url, title, visited_at) in visits {
                stmt.execute(params![space_id, url, title, visited_at, source])?;
            }
        }
        tx.commit()
    }
}

/// 在连接上注册 `alter_fold(text)`：Unicode 小写折叠（与 Rust 的 `str::to_lowercase` 一致），供搜索使用。
pub(super) fn register_functions(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    use rusqlite::functions::FunctionFlags;
    conn.create_scalar_function(
        "alter_fold",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| Ok(ctx.get::<Option<String>>(0)?.map(|s| s.to_lowercase())),
    )
}
