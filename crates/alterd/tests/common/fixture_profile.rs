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

/// fixture 的 Safe Storage 口令（钥匙串替身 `ALTER_KEYCHAIN_FAKE` 用同一个值）。
pub const COOKIE_PASSWORD: &str = "peanuts";

/// `Cookies` 库里的一行。`encrypted` 为 None 时按 v10 加密 `value`（库版本 ≥24 时先加主机名前缀）写进
/// `encrypted_value`、`value` 列留空；为 Some 时原样写进 `encrypted_value`，`value` 列写 `value`。
pub struct CookieRow {
    pub host: &'static str,
    pub name: &'static str,
    pub value: &'static str,
    pub path: &'static str,
    /// 过期时间（UTC 毫秒）；None 为会话 Cookie
    pub expires_ms: Option<i64>,
    pub secure: bool,
    pub http_only: bool,
    /// Chromium 的取值：-1 unspecified、0 no_restriction、1 lax、2 strict
    pub same_site: i64,
    pub encrypted: Option<Vec<u8>>,
}

impl CookieRow {
    pub fn new(host: &'static str, name: &'static str, value: &'static str) -> CookieRow {
        CookieRow {
            host,
            name,
            value,
            path: "/",
            expires_ms: None,
            secure: false,
            http_only: false,
            same_site: -1,
            encrypted: None,
        }
    }
}

/// 按 Chromium 在 macOS 上的做法加密：PBKDF2-HMAC-SHA1(口令, "saltysalt", 1003) 取 16 字节，
/// AES-128-CBC，IV 为 16 个空格，PKCS#7 填充，前面加 `v10`。独立于被测代码的实现。
pub fn encrypt_v10(plain: &[u8]) -> Vec<u8> {
    use cbc::cipher::{BlockModeEncrypt, KeyIvInit, block_padding::Pkcs7};
    let mut key = [0u8; 16];
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(COOKIE_PASSWORD.as_bytes(), b"saltysalt", 1003, &mut key);
    let ct = cbc::Encryptor::<aes::Aes128>::new(&key.into(), &[b' '; 16].into())
        .encrypt_padded_vec::<Pkcs7>(plain);
    let mut out = b"v10".to_vec();
    out.extend_from_slice(&ct);
    out
}

/// 库版本 ≥24 的明文：`SHA256(host)` 32 字节 + 值。
pub fn with_host_prefix(host: &str, value: &str) -> Vec<u8> {
    use sha2::Digest;
    let mut out = sha2::Sha256::digest(host.as_bytes()).to_vec();
    out.extend_from_slice(value.as_bytes());
    out
}

/// 写 `Cookies`（Chromium 2026-10 的表结构，`meta.version` 为 `version`）。
pub fn write_cookies(dir: &Path, version: i64, rows: &[CookieRow]) {
    std::fs::create_dir_all(dir).unwrap();
    let conn = rusqlite::Connection::open(dir.join("Cookies")).unwrap();
    conn.execute_batch(
        "CREATE TABLE meta(key LONGVARCHAR NOT NULL UNIQUE PRIMARY KEY, value LONGVARCHAR);
         CREATE TABLE cookies(creation_utc INTEGER NOT NULL, host_key TEXT NOT NULL,
           top_frame_site_key TEXT NOT NULL, name TEXT NOT NULL, value TEXT NOT NULL,
           encrypted_value BLOB NOT NULL, path TEXT NOT NULL, expires_utc INTEGER NOT NULL,
           is_secure INTEGER NOT NULL, is_httponly INTEGER NOT NULL, last_access_utc INTEGER NOT NULL,
           has_expires INTEGER NOT NULL, is_persistent INTEGER NOT NULL, priority INTEGER NOT NULL,
           samesite INTEGER NOT NULL, source_scheme INTEGER NOT NULL, source_port INTEGER NOT NULL,
           last_update_utc INTEGER NOT NULL, source_type INTEGER NOT NULL,
           has_cross_site_ancestor INTEGER NOT NULL);",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO meta (key, value) VALUES ('version', ?1)",
        [version.to_string()],
    )
    .unwrap();
    let now = chrome_time(now_ms());
    for r in rows {
        let (value, encrypted) = match &r.encrypted {
            Some(raw) => (r.value.to_owned(), raw.clone()),
            None => {
                let plain = if version >= 24 {
                    with_host_prefix(r.host, r.value)
                } else {
                    r.value.as_bytes().to_vec()
                };
                (String::new(), encrypt_v10(&plain))
            }
        };
        conn.execute(
            "INSERT INTO cookies VALUES (?1, ?2, '', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?1, ?10, ?10, 1, ?11, 2, 443, ?1, 0, 0)",
            rusqlite::params![
                now,
                r.host,
                r.name,
                value,
                encrypted,
                r.path,
                r.expires_ms.map_or(0, chrome_time),
                r.secure as i64,
                r.http_only as i64,
                r.expires_ms.is_some() as i64,
                r.same_site,
            ],
        )
        .unwrap();
    }
}

/// 标准 Cookie fixture：`meta.version` 24，`sid=abc`（127.0.0.1，30 天后过期，httpOnly）、
/// `theme=dark`（.example.com，会话 Cookie）、`old=1`（127.0.0.1，1 天前已过期）。
pub fn standard_cookies(dir: &Path) {
    write_cookies(dir, 24, &standard_cookie_rows());
}

pub fn standard_cookie_rows() -> Vec<CookieRow> {
    vec![
        CookieRow {
            expires_ms: Some(now_ms() + 30 * DAY_MS),
            http_only: true,
            ..CookieRow::new("127.0.0.1", "sid", "abc")
        },
        CookieRow::new(".example.com", "theme", "dark"),
        CookieRow {
            expires_ms: Some(now_ms() - DAY_MS),
            ..CookieRow::new("127.0.0.1", "old", "1")
        },
    ]
}
