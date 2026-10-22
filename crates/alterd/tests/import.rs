//! import 能力的契约测试：真实的 `alterd run` 子进程 + Gateway 客户端，fixture profile 在测试里生成。

mod common;

use std::path::Path;
use std::time::Duration;

use common::fixture_profile::{self, Standard};
use common::{
    Client, Daemon, Ws, connected, create_space, gateway_client, rpc, space_id, ws_recv_within,
};
use serde_json::{Value, json};

/// 起 alterd，建 Space `Work`，返回 Gateway 客户端与 `Work` 的 id。
fn start_with_work(home: &Path) -> (Daemon, Ws, String) {
    let (d, mut c, _) = connected(home);
    let snap = create_space(&mut c, "Work", "#f97316");
    let work = space_id(&snap, "Work");
    (d, gateway_client(home), work)
}

/// 调 `import.run`，收集它之后的通知直到 `import.done`（10 秒为限）：返回 (importId, 全部通知, done 的 params)。
fn run_import(ws: &mut Ws, params: Value) -> (i64, Vec<Value>, Value) {
    let resp = rpc(ws, 1, "import.run", params);
    let id = resp["result"]["importId"]
        .as_i64()
        .unwrap_or_else(|| panic!("import.run 应当返回 importId，响应：{resp}"));
    let mut notes = Vec::new();
    loop {
        let msg = ws_recv_within(ws, Duration::from_secs(10)).expect("10 秒内应当收到 import.done");
        if msg["method"].is_string() {
            notes.push(msg.clone());
        }
        if msg["method"] == "import.done" && msg["params"]["importId"] == id {
            return (id, notes, msg["params"].clone());
        }
    }
}

fn db(home: &Path) -> rusqlite::Connection {
    rusqlite::Connection::open(home.join("alter.db")).unwrap()
}

/// `space_id` 的书签：(title, url, 上级标题, source)，按标题排序。
fn bookmarks_of(
    home: &Path,
    space_id: &str,
) -> Vec<(String, Option<String>, Option<String>, String)> {
    let conn = db(home);
    let mut stmt = conn
        .prepare(
            "SELECT b.title, b.url, p.title, b.source FROM bookmarks b
             LEFT JOIN bookmarks p ON p.id = b.parent_id
             WHERE b.space_id = ?1 ORDER BY b.title",
        )
        .unwrap();
    stmt.query_map([space_id], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
    })
    .unwrap()
    .map(Result::unwrap)
    .collect()
}

/// N5 R1.S1 导入书签树：counts.bookmarks 为 3，Dev 文件夹、其下的 MDN、Rust，source 为 chrome。
#[test]
fn n5_r1_s1_import_bookmark_tree() {
    let home = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    fixture_profile::standard(profile.path());
    let (_d, mut ws, work) = start_with_work(home.path());

    let (id, _, done) = run_import(
        &mut ws,
        json!({"source": "chrome", "profilePath": profile.path(), "spaceId": work, "items": ["bookmarks"]}),
    );
    assert_eq!(done["status"], "done", "import.done：{done}");
    assert_eq!(done["counts"]["bookmarks"], 3, "import.done：{done}");

    let chrome = "chrome".to_owned();
    assert_eq!(
        bookmarks_of(home.path(), &work),
        vec![
            ("Dev".to_owned(), None, None, chrome.clone()),
            (
                "MDN".to_owned(),
                Some("https://developer.mozilla.org/".to_owned()),
                Some("Dev".to_owned()),
                chrome.clone()
            ),
            (
                "Rust".to_owned(),
                Some("https://rust-lang.org/".to_owned()),
                None,
                chrome
            ),
        ]
    );

    // 按原有层级与顺序：根级 Rust 在 Dev 之前，MDN 是 Dev 下的第一项
    let conn = db(home.path());
    let position = |title: &str| -> i64 {
        conn.query_row(
            "SELECT position FROM bookmarks WHERE space_id = ?1 AND title = ?2",
            [work.as_str(), title],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert!(position("Rust") < position("Dev"), "Rust 应当排在 Dev 之前");
    assert_eq!(position("MDN"), 0);

    // import.status 与 import.done 一致
    let st = rpc(&mut ws, 2, "import.status", json!({"importId": id}));
    assert_eq!(
        st["result"]["status"], done["status"],
        "import.status：{st}"
    );
    assert_eq!(
        st["result"]["counts"], done["counts"],
        "import.status：{st}"
    );
}

/// N5 R1.S2 对同一个 profile 与 Space 再导入一次书签：Work 仍是 3 行（根级条目与文件夹也不重复）。
#[test]
fn n5_r1_s2_reimport_does_not_duplicate() {
    let home = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    fixture_profile::standard(profile.path());
    let (_d, mut ws, work) = start_with_work(home.path());
    let params = json!({"source": "chrome", "profilePath": profile.path(), "spaceId": work, "items": ["bookmarks"]});

    run_import(&mut ws, params.clone());
    assert_eq!(bookmarks_of(home.path(), &work).len(), 3);
    let (_, _, done) = run_import(&mut ws, params);
    assert_eq!(done["status"], "done", "import.done：{done}");
    assert_eq!(
        bookmarks_of(home.path(), &work).len(),
        3,
        "{:?}",
        bookmarks_of(home.path(), &work)
    );
}

/// N5 R2.S1 只导入 90 天内的访问：counts.history 为 2，两行 a.example 的 visited_at 与 fixture 相同，没有 b.example。
#[test]
fn n5_r2_s1_history_last_90_days() {
    let home = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let Standard { a_visits } = fixture_profile::standard(profile.path());
    let (_d, mut ws, work) = start_with_work(home.path());

    let params = json!({"source": "chrome", "profilePath": profile.path(), "spaceId": work, "items": ["history"]});
    let (_, _, done) = run_import(&mut ws, params.clone());
    assert_eq!(done["counts"]["history"], 2, "import.done：{done}");
    // 同一份历史再导入一次：按（URL、访问时间）去重，不多出行
    let (_, _, again) = run_import(&mut ws, params);
    assert_eq!(again["status"], "done", "第二次 import.done：{again}");

    let conn = db(home.path());
    let mut stmt = conn
        .prepare(
            "SELECT url, visited_at, source FROM history WHERE space_id = ?1 ORDER BY visited_at",
        )
        .unwrap();
    let rows: Vec<(String, i64, String)> = stmt
        .query_map([&work], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let a = "https://a.example/".to_owned();
    let chrome = "chrome".to_owned();
    assert_eq!(
        rows,
        vec![
            (a.clone(), a_visits[0], chrome.clone()),
            (a, a_visits[1], chrome),
        ]
    );
}

/// N5 R5.S1 来源目录不存在：E_IMPORT_SOURCE_NOT_FOUND，imports 没有新行。
#[test]
fn n5_r5_s1_source_not_found() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut ws, work) = start_with_work(home.path());

    let resp = rpc(
        &mut ws,
        1,
        "import.run",
        json!({"source": "chrome", "profilePath": "/nonexistent/profile", "spaceId": work}),
    );
    assert_eq!(
        resp["error"]["data"]["code"], "E_IMPORT_SOURCE_NOT_FOUND",
        "响应：{resp}"
    );
    let n: i64 = db(home.path())
        .query_row("SELECT COUNT(*) FROM imports", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 0);
}

/// N5 R6.S1 省略 profilePath：Arc 用 `ALTER_ARC_DIR/Default`，书签的 source 为 arc。
#[test]
fn n5_r6_s1_arc_default_profile() {
    let home = tempfile::tempdir().unwrap();
    let arc = tempfile::tempdir().unwrap();
    fixture_profile::standard(&arc.path().join("Default"));
    let arc_dir = arc.path().to_str().unwrap();
    let _d = Daemon::start_with_env(home.path(), &[("ALTER_ARC_DIR", arc_dir)]);
    let mut ws = gateway_client(home.path());
    let spaces = rpc(&mut ws, 1, "space.list", json!({}));
    let personal = spaces["result"]["spaces"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let (_, _, done) = run_import(
        &mut ws,
        json!({"source": "arc", "spaceId": personal, "items": ["bookmarks"]}),
    );
    assert_eq!(done["status"], "done", "import.done：{done}");
    assert_eq!(done["counts"]["bookmarks"], 3, "import.done：{done}");
    let rows = bookmarks_of(home.path(), &personal);
    assert_eq!(rows.len(), 3, "{rows:?}");
    assert!(rows.iter().all(|r| r.3 == "arc"), "{rows:?}");
}

/// N5 R6.S2 列出可导入的 profile：`Default` 与 `Profile 1`（含 `Bookmarks`）按名称排序；
/// 不含数据文件的目录与普通文件不算；Arc 的数据根目录不存在时为空数组。
#[test]
fn n5_r6_s2_list_import_sources() {
    let home = tempfile::tempdir().unwrap();
    let chrome = tempfile::tempdir().unwrap();
    for name in ["Profile 1", "Default"] {
        fixture_profile::write_bookmarks(&chrome.path().join(name), vec![], vec![]);
    }
    std::fs::create_dir_all(chrome.path().join("Crashpad")).unwrap();
    std::fs::write(chrome.path().join("Local State"), "{}").unwrap();
    let chrome_dir = chrome.path().to_str().unwrap();
    let missing = home.path().join("no-arc");
    let _d = Daemon::start_with_env(
        home.path(),
        &[
            ("ALTER_CHROME_DIR", chrome_dir),
            ("ALTER_ARC_DIR", missing.to_str().unwrap()),
        ],
    );
    let mut c = Client::connect(home.path());
    c.recv();

    c.send(json!({"type": "shell.event.importSourcesRequested", "requestId": 5}));
    let msg = c.recv();
    assert_eq!(msg["type"], "shell.importSources", "收到：{msg}");
    assert_eq!(msg["requestId"], 5);
    assert_eq!(msg["chrome"], json!(["Default", "Profile 1"]));
    assert_eq!(msg["arc"], json!([]));
}

/// N5 R8.S1 省略 items 时两类数据的进度：依次 progress bookmarks、progress history、done（3、2），
/// 之后 import.status 与 done 一致。
#[test]
fn n5_r8_s1_progress_of_default_items() {
    let home = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    fixture_profile::standard(profile.path());
    let (_d, mut ws, work) = start_with_work(home.path());
    // 旁观的另一个连接：进度只推给发起导入的连接
    let mut bystander = gateway_client(home.path());

    let (id, notes, done) = run_import(
        &mut ws,
        json!({"source": "chrome", "profilePath": profile.path(), "spaceId": work}),
    );
    if let Some(msg) = ws_recv_within(&mut bystander, Duration::from_millis(300)) {
        panic!("旁观的连接不应收到消息：{msg}");
    }
    let seq: Vec<(String, String)> = notes
        .iter()
        .map(|n| {
            (
                n["method"].as_str().unwrap().to_owned(),
                n["params"]["item"].as_str().unwrap_or("").to_owned(),
            )
        })
        .collect();
    assert_eq!(
        seq,
        vec![
            ("import.progress".to_owned(), "bookmarks".to_owned()),
            ("import.progress".to_owned(), "history".to_owned()),
            ("import.done".to_owned(), String::new()),
        ],
        "通知：{notes:?}"
    );
    assert_eq!(done["status"], "done", "import.done：{done}");
    assert_eq!(done["counts"]["bookmarks"], 3, "import.done：{done}");
    assert_eq!(done["counts"]["history"], 2, "import.done：{done}");

    let st = rpc(&mut ws, 2, "import.status", json!({"importId": id}));
    assert_eq!(
        st["result"]["status"], done["status"],
        "import.status：{st}"
    );
    assert_eq!(
        st["result"]["counts"], done["counts"],
        "import.status：{st}"
    );
}

/// N5 CR3：上次 alterd 退出时还在进行的导入，下次启动时记为 failed（`error` 为 `interrupted`），
/// 留在 `import-tmp` 里的副本被删除。
#[test]
fn n5_cr3_startup_recovers_interrupted_import() {
    let home = tempfile::tempdir().unwrap();
    let personal = {
        let d = Daemon::start(home.path());
        let mut ws = gateway_client(home.path());
        let spaces = rpc(&mut ws, 1, "space.list", json!({}));
        let id = spaces["result"]["spaces"][0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        drop(ws);
        d.terminate();
        id
    };
    db(home.path())
        .execute(
            "INSERT INTO imports (id, space_id, source, profile_path, status, started_at, finished_at, error)
             VALUES (7, ?1, 'chrome', '/p', 'running', 1, NULL, NULL),
                    (8, ?1, 'chrome', '/p', 'done', 1, 2, NULL),
                    (9, ?1, 'arc', '/p', 'failed', 1, 3, 'boom')",
            [&personal],
        )
        .unwrap();
    let leftover = home.path().join("import-tmp").join("7");
    std::fs::create_dir_all(&leftover).unwrap();
    std::fs::write(leftover.join("History"), b"copy").unwrap();

    let _d = Daemon::start(home.path());
    let mut ws = gateway_client(home.path());
    let st = rpc(&mut ws, 1, "import.status", json!({"importId": 7}));
    assert_eq!(st["result"]["status"], "failed", "import.status：{st}");
    assert_eq!(st["result"]["error"], "interrupted", "import.status：{st}");
    assert!(
        !home.path().join("import-tmp").exists(),
        "上次留下的临时副本应当已删除"
    );
    // 已经结束的导入不受影响
    let ended: Vec<(i64, String, Option<String>, Option<i64>)> = {
        let conn = db(home.path());
        let mut stmt = conn
            .prepare(
                "SELECT id, status, error, finished_at FROM imports WHERE id IN (8, 9) ORDER BY id",
            )
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(
        ended,
        vec![
            (8, "done".to_owned(), None, Some(2)),
            (9, "failed".to_owned(), Some("boom".to_owned()), Some(3)),
        ]
    );
}

/// N5 可观测性（父需求 NFR）：导入每完成一类数据记一条日志，带类别与计数。
#[test]
fn n5_obs_logs_each_item_with_counts() {
    let home = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    fixture_profile::standard(profile.path());
    // 书签栏里多一条没有 url 的链接：跳过并计数
    fixture_profile::write_bookmarks(
        profile.path(),
        vec![
            fixture_profile::link("Rust", "https://rust-lang.org/"),
            fixture_profile::folder(
                "Dev",
                vec![fixture_profile::link(
                    "MDN",
                    "https://developer.mozilla.org/",
                )],
            ),
            json!({"type": "url", "name": "坏的"}),
        ],
        vec![],
    );
    let (d, mut ws, work) = start_with_work(home.path());
    run_import(
        &mut ws,
        json!({"source": "chrome", "profilePath": profile.path(), "spaceId": work}),
    );
    drop(ws);
    let stderr = d.stop_and_stderr();
    let items: Vec<(String, i64, i64)> = stderr
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["fields"]["message"] == "导入完成一类")
        .map(|v| {
            (
                v["fields"]["item"].as_str().unwrap_or("").to_owned(),
                v["fields"]["count"].as_i64().unwrap_or(-1),
                v["fields"]["skipped"].as_i64().unwrap_or(-1),
            )
        })
        .collect();
    assert_eq!(
        items,
        vec![("bookmarks".to_owned(), 3, 1), ("history".to_owned(), 2, 0)],
        "stderr：{stderr}"
    );
}
