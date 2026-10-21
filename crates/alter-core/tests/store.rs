//! storage 能力的场景测试：从 `Store::open` 驱动。

use alter_core::store::{Store, db_path};
use rusqlite::Connection;

const TABLES: [&str; 8] = [
    "schema_version",
    "spaces",
    "tabs",
    "bookmarks",
    "history",
    "task_spaces",
    "task_pages",
    "imports",
];

/// R1.S1 空目录首次打开：建出 alter.db、8 张表，版本为 1。
#[test]
fn r1_s1_fresh_open_creates_schema() {
    let home = tempfile::tempdir().unwrap();

    let store = Store::open(home.path()).expect("首次打开应当成功");
    drop(store);

    let path = db_path(home.path());
    assert_eq!(path, home.path().join("alter.db"));
    assert!(path.exists(), "alter.db 应当存在");

    // 用独立连接检查落盘结果
    let conn = Connection::open(&path).unwrap();
    for table in TABLES {
        let found: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(found, 1, "缺少表 {table}");
    }
    let version: i64 = conn
        .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 1);
}

/// R1.S2 重复打开不重复迁移：第二次打开成功，schema_version 只有一行。
#[test]
fn r1_s2_reopen_is_idempotent() {
    let home = tempfile::tempdir().unwrap();
    drop(Store::open(home.path()).expect("首次打开应当成功"));

    let store = Store::open(home.path()).expect("再次打开应当成功");
    drop(store);

    let conn = Connection::open(db_path(home.path())).unwrap();
    let rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM schema_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 1);
}

/// R1.S3 WAL 与外键生效：PRAGMA 为 wal 与 1，引用不存在 space 的 tab 被拒绝。
#[test]
fn r1_s3_wal_and_foreign_keys() {
    let home = tempfile::tempdir().unwrap();
    let store = Store::open(home.path()).expect("首次打开应当成功");
    let conn = store.conn();

    let journal: String = conn
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    assert_eq!(journal, "wal");
    let fk: i64 = conn
        .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
        .unwrap();
    assert_eq!(fk, 1);

    let err = conn
        .execute(
            "INSERT INTO tabs (id, space_id, url, position, last_active_at) \
             VALUES ('t1', 'no-such-space', 'https://example.com', 0, 0)",
            [],
        )
        .expect_err("引用不存在的 space 应当被拒绝");
    assert!(
        err.to_string().contains("FOREIGN KEY constraint failed"),
        "实际错误：{err}"
    );
}
