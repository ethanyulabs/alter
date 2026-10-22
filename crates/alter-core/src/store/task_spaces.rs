//! task_spaces / task_pages 表的读写：agent 的 TaskSpace 与它的 Page（`p1`、`p2`…）。

use rusqlite::{OptionalExtension, params};

use super::Store;

/// 一个 TaskSpace。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSpaceRow {
    pub id: i64,
    pub space_id: String,
    pub name: String,
    /// `agent` | `user`
    pub ownership: String,
    /// `active` | `finished` | `inactive`
    pub status: String,
    pub page_budget: i64,
}

/// TaskSpace 里一个未关闭的 Page。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageRow {
    pub task_space_id: i64,
    pub label: String,
    pub tab_id: String,
    /// `agent` | `user`
    pub opened_by: String,
}

const TASK_SPACE_COLUMNS: &str = "id, space_id, name, ownership, status, page_budget";

fn task_space_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<TaskSpaceRow> {
    Ok(TaskSpaceRow {
        id: r.get(0)?,
        space_id: r.get(1)?,
        name: r.get(2)?,
        ownership: r.get(3)?,
        status: r.get(4)?,
        page_budget: r.get(5)?,
    })
}

fn page_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<PageRow> {
    Ok(PageRow {
        task_space_id: r.get(0)?,
        label: r.get(1)?,
        tab_id: r.get(2)?,
        opened_by: r.get(3)?,
    })
}

/// 按 `p<n>` 的数字排序。
const PAGE_ORDER: &str = "CAST(SUBSTR(label, 2) AS INTEGER)";

impl Store {
    /// 新建 TaskSpace（ownership 为 agent、状态 active），返回 id。
    pub fn create_task_space(
        &self,
        space_id: &str,
        name: &str,
        page_budget: i64,
        now: i64,
    ) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO task_spaces (space_id, name, page_budget, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![space_id, name, page_budget, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn task_space(&self, id: i64) -> rusqlite::Result<Option<TaskSpaceRow>> {
        self.conn
            .query_row(
                &format!("SELECT {TASK_SPACE_COLUMNS} FROM task_spaces WHERE id = ?1"),
                [id],
                task_space_row,
            )
            .optional()
    }

    /// 全部 TaskSpace，按 id 升序。
    pub fn list_task_spaces(&self) -> rusqlite::Result<Vec<TaskSpaceRow>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {TASK_SPACE_COLUMNS} FROM task_spaces ORDER BY id"
        ))?;
        stmt.query_map([], task_space_row)?.collect()
    }

    /// 改 ownership。回到 `agent` 时未结束的 TaskSpace 的 status 一并回到 `active`（`inactive` 只出现在 user 期间）。
    pub fn set_ownership(&self, id: i64, ownership: &str) -> rusqlite::Result<()> {
        if ownership == "agent" {
            self.conn.execute(
                "UPDATE task_spaces SET ownership = 'agent',
                    status = CASE WHEN status = 'finished' THEN status ELSE 'active' END
                 WHERE id = ?1",
                [id],
            )?;
        } else {
            self.conn.execute(
                "UPDATE task_spaces SET ownership = ?2 WHERE id = ?1",
                params![id, ownership],
            )?;
        }
        Ok(())
    }

    /// ownership 为 user 且仍 active 的 TaskSpace 标记为 inactive；返回是否有变化。
    pub fn mark_inactive(&self, id: i64) -> rusqlite::Result<bool> {
        let n = self.conn.execute(
            "UPDATE task_spaces SET status = 'inactive'
             WHERE id = ?1 AND ownership = 'user' AND status = 'active'",
            [id],
        )?;
        Ok(n > 0)
    }

    pub fn finish_task_space(&self, id: i64, now: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE task_spaces SET status = 'finished', finished_at = ?2 WHERE id = ?1",
            params![id, now],
        )?;
        Ok(())
    }

    /// 某个 TaskSpace 未关闭的 Page，按编号升序。
    pub fn open_pages(&self, task_space_id: i64) -> rusqlite::Result<Vec<PageRow>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT task_space_id, label, tab_id, opened_by FROM task_pages
             WHERE task_space_id = ?1 AND closed_at IS NULL AND tab_id IS NOT NULL ORDER BY {PAGE_ORDER}"
        ))?;
        stmt.query_map([task_space_id], page_row)?.collect()
    }

    /// 全部未关闭的 Page（快照用）。
    pub fn all_open_pages(&self) -> rusqlite::Result<Vec<PageRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT task_space_id, label, tab_id, opened_by FROM task_pages
             WHERE closed_at IS NULL AND tab_id IS NOT NULL",
        )?;
        stmt.query_map([], page_row)?.collect()
    }

    /// 未关闭的 Page。
    pub fn open_page(&self, task_space_id: i64, label: &str) -> rusqlite::Result<Option<PageRow>> {
        self.conn
            .query_row(
                "SELECT task_space_id, label, tab_id, opened_by FROM task_pages
                 WHERE task_space_id = ?1 AND label = ?2 AND closed_at IS NULL AND tab_id IS NOT NULL",
                params![task_space_id, label],
                page_row,
            )
            .optional()
    }

    /// 托管着该标签的未关闭 Page。
    pub fn page_of_tab(&self, tab_id: &str) -> rusqlite::Result<Option<PageRow>> {
        self.conn
            .query_row(
                "SELECT task_space_id, label, tab_id, opened_by FROM task_pages
                 WHERE tab_id = ?1 AND closed_at IS NULL",
                [tab_id],
                page_row,
            )
            .optional()
    }

    /// 下一个 Page 标签：已用过的最大编号 + 1（关闭的也算，标签不复用）。
    pub fn next_page_label(&self, task_space_id: i64) -> rusqlite::Result<String> {
        let max: Option<i64> = self.conn.query_row(
            &format!("SELECT MAX({PAGE_ORDER}) FROM task_pages WHERE task_space_id = ?1"),
            [task_space_id],
            |r| r.get(0),
        )?;
        Ok(format!("p{}", max.unwrap_or(0) + 1))
    }

    pub fn add_page(
        &self,
        task_space_id: i64,
        label: &str,
        tab_id: &str,
        opened_by: &str,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO task_pages (task_space_id, label, tab_id, opened_by) VALUES (?1, ?2, ?3, ?4)",
            params![task_space_id, label, tab_id, opened_by],
        )?;
        Ok(())
    }

    /// 关闭 Page（归还或关闭）；返回是否有这一页。
    pub fn close_page(&self, task_space_id: i64, label: &str, now: i64) -> rusqlite::Result<bool> {
        let n = self.conn.execute(
            "UPDATE task_pages SET closed_at = ?3
             WHERE task_space_id = ?1 AND label = ?2 AND closed_at IS NULL",
            params![task_space_id, label, now],
        )?;
        Ok(n > 0)
    }

    /// 标签被关闭（归档）后，托管它的 Page 也关闭。
    pub fn close_pages_of_tab(&self, tab_id: &str, now: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE task_pages SET closed_at = ?2 WHERE tab_id = ?1 AND closed_at IS NULL",
            params![tab_id, now],
        )?;
        Ok(())
    }

    /// 在 Space 末尾新建 agent 标签，但不抢激活：`last_active_at` 取同 Space 未归档标签的最小值 − 1。
    pub fn insert_agent_tab(
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
                     COALESCE((SELECT MIN(last_active_at) - 1 FROM tabs
                               WHERE space_id = ?2 AND archived_at IS NULL), ?4))",
            params![id, space_id, url, now],
        )?;
        Ok(())
    }

    /// 未归档标签所在的 Space。
    pub fn tab_space(&self, tab_id: &str) -> rusqlite::Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT space_id FROM tabs WHERE id = ?1 AND archived_at IS NULL",
                [tab_id],
                |r| r.get(0),
            )
            .optional()
    }
}
