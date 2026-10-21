//! 迁移运行器：按编号升序执行编进二进制的 SQL 文件。

use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, params};

use super::StoreError;

/// 全部迁移，(版本号, SQL)，按版本号升序排列。
const MIGRATIONS: &[(i64, &str)] = &[(1, include_str!("../../migrations/0001_init.sql"))];

/// 依次执行版本号大于当前版本的迁移，每个迁移一个事务，执行后写入 `schema_version`。
pub(super) fn apply(conn: &mut Connection) -> Result<(), StoreError> {
    let current = current_version(conn)?;
    for &(version, sql) in MIGRATIONS.iter().filter(|(v, _)| *v > current) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.execute(
            "INSERT INTO schema_version (version, applied_at) VALUES (?1, ?2)",
            params![version, now_millis()],
        )?;
        tx.commit()?;
    }
    Ok(())
}

/// 已应用的最高版本；`schema_version` 表还不存在时为 0。
fn current_version(conn: &Connection) -> Result<i64, StoreError> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_version')",
        [],
        |r| r.get(0),
    )?;
    if !exists {
        return Ok(0);
    }
    let version: Option<i64> =
        conn.query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))?;
    Ok(version.unwrap_or(0))
}

/// 当前 UTC 毫秒时间戳。
fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
