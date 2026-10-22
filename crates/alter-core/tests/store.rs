//! storage 能力的场景测试：从 `Store::open` 驱动。

use alter_core::store::{Store, db_path};
use rusqlite::Connection;

/// 表是否有某一列。
fn has_column(conn: &Connection, table: &str, column: &str) -> bool {
    conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM pragma_table_info(?1) WHERE name = ?2)",
        [table, column],
        |r| r.get(0),
    )
    .unwrap()
}

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

/// N1 R13.S1 空目录首次打开：建出 alter.db、8 张表，版本为 2，带 0002 新增的列。
#[test]
fn n1_r13_s1_fresh_open_creates_schema() {
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
    assert_eq!(version, 2);
    // 「共 8 张」：除 SQLite 自己的内部表（如 sqlite_sequence）外恰好这 8 张
    let user_tables: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(user_tables, 8);
    assert!(has_column(&conn, "spaces", "last_active_at"));
    assert!(has_column(&conn, "tabs", "pinned_url"));
}

/// N1 R13.S2 重复打开不重复迁移：第二次打开成功，schema_version 恰好两行。
#[test]
fn n1_r13_s2_reopen_is_idempotent() {
    let home = tempfile::tempdir().unwrap();
    drop(Store::open(home.path()).expect("首次打开应当成功"));

    let store = Store::open(home.path()).expect("再次打开应当成功");
    drop(store);

    let conn = Connection::open(db_path(home.path())).unwrap();
    let rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM schema_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 2);
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

/// 在 `home` 下建一个只应用了迁移 0001 的 alter.db，不经过 `Store::open`。
fn v1_db(home: &std::path::Path) -> Connection {
    let conn = Connection::open(db_path(home)).unwrap();
    conn.execute_batch(include_str!("../migrations/0001_init.sql"))
        .unwrap();
    conn.execute(
        "INSERT INTO schema_version (version, applied_at) VALUES (1, 0)",
        [],
    )
    .unwrap();
    conn
}

/// 读某一列的单个值。
fn query_one<T: rusqlite::types::FromSql>(conn: &Connection, sql: &str) -> T {
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

/// N1 R13.S4 已有标签按分区重新编号：固定标签排到前面，同 Space 内编号为 0..n-1。
#[test]
fn n1_r13_s4_existing_tabs_are_renumbered() {
    let home = tempfile::tempdir().unwrap();
    let conn = v1_db(home.path());
    conn.execute_batch(
        "INSERT INTO spaces (id, name, position, created_at) VALUES ('s1', 'Personal', 0, 0);
         INSERT INTO tabs (id, space_id, url, pinned, position, last_active_at)
           VALUES ('A', 's1', 'https://example.com/a', 0, 0, 1),
                  ('B', 's1', 'https://example.com/b', 1, 5, 2),
                  ('C', 's1', 'https://example.com/c', 0, 9, 3);
         INSERT INTO spaces (id, name, position, created_at) VALUES ('s2', 'Work', 1, 0);
         INSERT INTO tabs (id, space_id, url, pinned, position, last_active_at)
           VALUES ('D', 's2', 'https://example.com/d', 0, 0, 1),
                  ('E', 's2', 'https://example.com/e', 0, 1, 2),
                  ('F', 's2', 'https://example.com/f', 1, 2, 3);",
    )
    .unwrap();
    drop(conn);

    drop(Store::open(home.path()).expect("升级打开应当成功"));

    let conn = Connection::open(db_path(home.path())).unwrap();
    let pos = |id: &str| -> i64 {
        conn.query_row("SELECT position FROM tabs WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .unwrap()
    };
    assert_eq!((pos("A"), pos("B"), pos("C")), (1, 0, 2));
    // 第二个 Space 各自从 0 编号；两个普通标签都在固定标签之前时，编号不能重复
    assert_eq!((pos("D"), pos("E"), pos("F")), (1, 2, 0));
}

/// N1 R14.S1 从版本 1 升级时生成备份：bak-1 是升级前的库，alter.db 已升到 2，数据都在。
#[test]
fn n1_r14_s1_upgrade_from_v1_writes_backup() {
    let home = tempfile::tempdir().unwrap();
    let conn = v1_db(home.path());
    conn.execute(
        "INSERT INTO spaces (id, name, position, created_at) VALUES ('s1', 'Old', 0, 0)",
        [],
    )
    .unwrap();
    drop(conn);

    drop(Store::open(home.path()).expect("升级打开应当成功"));

    let bak = home.path().join("alter.db.bak-1");
    assert!(bak.exists(), "应当生成 alter.db.bak-1");
    let bak = Connection::open(&bak).unwrap();
    assert_eq!(
        query_one::<i64>(&bak, "SELECT MAX(version) FROM schema_version"),
        1
    );
    assert_eq!(query_one::<String>(&bak, "SELECT name FROM spaces"), "Old");

    let db = Connection::open(db_path(home.path())).unwrap();
    assert_eq!(
        query_one::<i64>(&db, "SELECT MAX(version) FROM schema_version"),
        2
    );
    assert_eq!(query_one::<String>(&db, "SELECT name FROM spaces"), "Old");
}

/// N1 R14.S2 新数据库和已是最新的数据库不生成备份。
#[test]
fn n1_r14_s2_no_backup_for_fresh_or_current_db() {
    let home = tempfile::tempdir().unwrap();
    drop(Store::open(home.path()).expect("首次打开应当成功"));
    drop(Store::open(home.path()).expect("再次打开应当成功"));

    let backups: Vec<_> = std::fs::read_dir(home.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("alter.db.bak-"))
        .collect();
    assert!(backups.is_empty(), "不应有备份文件：{backups:?}");
}

/// N1 R14.S1 备份失败时不迁移：`alter.db.bak-1` 被同名目录占住，打开返回错误，库仍是版本 1。
#[test]
fn n1_r14_s1_backup_failure_aborts_migration() {
    let home = tempfile::tempdir().unwrap();
    drop(v1_db(home.path()));
    std::fs::create_dir(home.path().join("alter.db.bak-1")).unwrap();

    assert!(Store::open(home.path()).is_err(), "备份失败时打开应当失败");

    let db = Connection::open(db_path(home.path())).unwrap();
    assert_eq!(
        query_one::<i64>(&db, "SELECT MAX(version) FROM schema_version"),
        1
    );
}

/// N5 CR2：`finish_import` 报告是否恰好更新了一行——导入行已被级联删除（目标 Space 删了）时为 false。
#[test]
fn n5_cr2_finish_import_reports_missing_row() {
    let home = tempfile::tempdir().unwrap();
    let store = Store::open(home.path()).unwrap();
    store.create_space("s1", "Work", "#f97316", 1).unwrap();
    // 最后一个 Space 不能删：先多建一个
    store.create_space("s2", "Personal", "#6366f1", 1).unwrap();
    let id = store.create_import("s1", "chrome", "/p", 1).unwrap();
    assert!(store.finish_import(id, "done", [3, 2, 0], None, 2).unwrap());

    let gone = store.create_import("s1", "chrome", "/p", 3).unwrap();
    store.delete_space("s1", 4).unwrap();
    assert!(
        !store
            .finish_import(gone, "done", [3, 2, 0], None, 5)
            .unwrap()
    );
}
