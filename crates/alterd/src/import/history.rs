//! 读 `History`（SQLite）：`visits` join `urls`，只取最近 90 天的访问。

use std::path::Path;

/// 1601-01-01 到 1970-01-01 的毫秒数。
const CHROME_EPOCH_OFFSET_MS: i64 = 11_644_473_600_000;

/// 只导入这么久以内的访问。
pub(super) const WINDOW_MS: i64 = 90 * 24 * 60 * 60 * 1000;

/// Chrome 时间（1601 年起的微秒）→ UTC 毫秒。
pub(super) fn chrome_to_unix_ms(t: i64) -> i64 {
    t / 1000 - CHROME_EPOCH_OFFSET_MS
}

fn unix_ms_to_chrome(ms: i64) -> i64 {
    (ms + CHROME_EPOCH_OFFSET_MS) * 1000
}

/// 一次访问：(url, title, visited_at)。
pub(super) type Visit = (String, String, i64);

/// 读副本 `db` 里访问时间不早于 `since`（UTC 毫秒）的访问：(url, title, visited_at)，
/// 以及 URL 为空而跳过的条数。
pub(super) fn read(db: &Path, since: i64) -> Result<(Vec<Visit>, u32), String> {
    let conn = rusqlite::Connection::open(db).map_err(|e| format!("打开 History 失败：{e}"))?;
    let mut stmt = conn
        .prepare(
            "SELECT u.url, u.title, v.visit_time FROM visits v JOIN urls u ON u.id = v.url
             WHERE v.visit_time >= ?1 ORDER BY v.visit_time",
        )
        .map_err(|e| format!("读取 History 失败：{e}"))?;
    let rows = stmt
        .query_map([unix_ms_to_chrome(since)], |r| {
            Ok((
                r.get::<_, Option<String>>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })
        .map_err(|e| format!("读取 History 失败：{e}"))?;
    let mut out = Vec::new();
    let mut skipped = 0;
    for row in rows {
        match row {
            Ok((Some(url), title, t)) if !url.is_empty() => {
                out.push((url, title.unwrap_or_default(), chrome_to_unix_ms(t)))
            }
            _ => skipped += 1,
        }
    }
    Ok((out, skipped))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_chrome_time_to_unix_ms() {
        assert_eq!(chrome_to_unix_ms(11_644_473_600_000_000), 0);
        assert_eq!(
            chrome_to_unix_ms(unix_ms_to_chrome(1_700_000_000_123)),
            1_700_000_000_123
        );
    }
}
