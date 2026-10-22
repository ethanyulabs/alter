//! fixture profile：在临时目录里生成 Chromium 格式的 profile（`Bookmarks` JSON、`History` SQLite）。
//! 时间按 Chrome 的格式写：1601-01-01 起的微秒。

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

/// 1601-01-01 到 1970-01-01 的毫秒数。
const CHROME_EPOCH_OFFSET_MS: i64 = 11_644_473_600_000;

pub const DAY_MS: i64 = 24 * 60 * 60 * 1000;

/// UTC 毫秒 → Chrome 时间（微秒）。
pub fn chrome_time(ms: i64) -> i64 {
    (ms + CHROME_EPOCH_OFFSET_MS) * 1000
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

/// 链接节点。
pub fn link(name: &str, url: &str) -> Value {
    json!({"type": "url", "name": name, "url": url})
}

/// 文件夹节点。
pub fn folder(name: &str, children: Vec<Value>) -> Value {
    json!({"type": "folder", "name": name, "children": children})
}

/// 写 `Bookmarks`：书签栏与其他书签下的节点。
pub fn write_bookmarks(dir: &Path, bar: Vec<Value>, other: Vec<Value>) {
    std::fs::create_dir_all(dir).unwrap();
    let doc = json!({
        "checksum": "",
        "roots": {
            "bookmark_bar": {"type": "folder", "name": "书签栏", "children": bar},
            "other": {"type": "folder", "name": "其他书签", "children": other},
            "synced": {"type": "folder", "name": "移动设备书签", "children": []},
        },
        "version": 1,
    });
    std::fs::write(dir.join("Bookmarks"), doc.to_string()).unwrap();
}

/// 写 `History`：每个 URL 一行 `urls`，每次访问（UTC 毫秒）一行 `visits`。
pub fn write_history(dir: &Path, entries: &[(&str, &str, &[i64])]) {
    std::fs::create_dir_all(dir).unwrap();
    let conn = rusqlite::Connection::open(dir.join("History")).unwrap();
    conn.execute_batch(
        "CREATE TABLE urls (id INTEGER PRIMARY KEY AUTOINCREMENT, url LONGVARCHAR, title LONGVARCHAR,
           visit_count INTEGER DEFAULT 0 NOT NULL, typed_count INTEGER DEFAULT 0 NOT NULL,
           last_visit_time INTEGER NOT NULL, hidden INTEGER DEFAULT 0 NOT NULL);
         CREATE TABLE visits (id INTEGER PRIMARY KEY AUTOINCREMENT, url INTEGER NOT NULL,
           visit_time INTEGER NOT NULL, from_visit INTEGER, transition INTEGER DEFAULT 0 NOT NULL);",
    )
    .unwrap();
    for (url, title, visits) in entries {
        let last = visits.iter().copied().max().unwrap_or(0);
        conn.execute(
            "INSERT INTO urls (url, title, visit_count, last_visit_time) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![url, title, visits.len() as i64, chrome_time(last)],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        for v in *visits {
            conn.execute(
                "INSERT INTO visits (url, visit_time) VALUES (?1, ?2)",
                rusqlite::params![id, chrome_time(*v)],
            )
            .unwrap();
        }
    }
}

/// 标准 fixture profile 里 `https://a.example/` 的两次访问（10 天前，UTC 毫秒）。
pub struct Standard {
    pub a_visits: [i64; 2],
}

/// 标准 fixture profile 的书签与历史：书签栏下 `Rust` 与 `Dev`（其中 `MDN`）；
/// `https://a.example/` 10 天前访问 2 次，`https://b.example/` 100 天前访问 1 次。
pub fn standard(dir: &Path) -> Standard {
    write_bookmarks(
        dir,
        vec![
            link("Rust", "https://rust-lang.org/"),
            folder("Dev", vec![link("MDN", "https://developer.mozilla.org/")]),
        ],
        vec![],
    );
    let ten_days_ago = now_ms() - 10 * DAY_MS;
    let a_visits = [ten_days_ago, ten_days_ago + 60_000];
    write_history(
        dir,
        &[
            ("https://a.example/", "A", &a_visits),
            ("https://b.example/", "B", &[now_ms() - 100 * DAY_MS]),
        ],
    );
    Standard { a_visits }
}
