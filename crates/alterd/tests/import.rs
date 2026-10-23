//! import 能力的契约测试：真实的 `alterd run` 子进程 + Gateway 客户端，fixture profile 在测试里生成。

mod common;

use std::path::Path;
use std::time::Duration;

use common::fake_cdp::FakeCdp;
use common::fixture_profile::{self, COOKIE_PASSWORD, CookieRow, Standard};
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
    let id = start_import(ws, params);
    let (notes, done) = wait_done(ws, id);
    (id, notes, done)
}

/// 调 `import.run`，返回 importId。
fn start_import(ws: &mut Ws, params: Value) -> i64 {
    let resp = rpc(ws, 1, "import.run", params);
    resp["result"]["importId"]
        .as_i64()
        .unwrap_or_else(|| panic!("import.run 应当返回 importId，响应：{resp}"))
}

/// 收集通知直到 `importId` 的 `import.done`（10 秒为限）：返回 (全部通知, done 的 params)。
fn wait_done(ws: &mut Ws, id: i64) -> (Vec<Value>, Value) {
    let mut notes = Vec::new();
    loop {
        let msg = ws_recv_within(ws, Duration::from_secs(10)).expect("10 秒内应当收到 import.done");
        if msg["method"].is_string() {
            notes.push(msg.clone());
        }
        if msg["method"] == "import.done" && msg["params"]["importId"] == id {
            return (notes, msg["params"].clone());
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

/// Cookie 导入的环境：alterd 带钥匙串替身（`envs` 可再加），假 Electron 端已上报 cdpReady，建好 Space `Work`。
struct CookieEnv {
    daemon: Daemon,
    shell: Client,
    ws: Ws,
    work: String,
    _cdp: FakeCdp,
}

fn cookie_env(home: &Path, envs: &[(&str, &str)]) -> CookieEnv {
    let mut all = vec![("ALTER_KEYCHAIN_FAKE", COOKIE_PASSWORD)];
    all.extend_from_slice(envs);
    let daemon = Daemon::start_with_env(home, &all);
    let mut shell = Client::connect(home);
    shell.recv();
    let snap = create_space(&mut shell, "Work", "#f97316");
    let work = space_id(&snap, "Work");
    let cdp = FakeCdp::start(home, &mut shell);
    CookieEnv {
        daemon,
        shell,
        ws: gateway_client(home),
        work,
        _cdp: cdp,
    }
}

/// 假 Electron 端：跳过快照，等下一条 `shell.importCookies`（5 秒为限）。
fn next_import_cookies(shell: &mut Client) -> Value {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        let msg = shell
            .recv_within(left)
            .expect("5 秒内应当收到 shell.importCookies");
        if msg["type"] == "shell.importCookies" {
            return msg;
        }
    }
}

/// 假 Electron 端回报这一批的写入结果。
fn answer_cookies(shell: &mut Client, cmd: &Value, imported: u32, rejected: u32) {
    shell.send(json!({
        "type": "shell.event.cookiesImported",
        "requestId": cmd["requestId"],
        "imported": imported,
        "rejected": rejected,
    }));
}

/// `cookies` 数组里名为 `name` 的那一项。
fn cookie<'a>(cmd: &'a Value, name: &str) -> Option<&'a Value> {
    cmd["cookies"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
}

/// RC N1 R1.S1 导入未过期的 Cookie：Electron 端收到 sid 与 theme 两项，回报 2 条后 done，cookies=2。
#[test]
fn rc_n1_r1_s1_import_unexpired_cookies() {
    let home = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    fixture_profile::standard_cookies(profile.path());
    let mut env = cookie_env(home.path(), &[]);

    let id = start_import(
        &mut env.ws,
        json!({"source": "chrome", "profilePath": profile.path(), "spaceId": env.work, "items": ["cookies"]}),
    );
    let cmd = next_import_cookies(&mut env.shell);
    assert_eq!(
        cmd["spaceId"],
        env.work.as_str(),
        "shell.importCookies：{cmd}"
    );
    assert_eq!(
        cmd["cookies"].as_array().unwrap().len(),
        2,
        "shell.importCookies：{cmd}"
    );
    let sid = cookie(&cmd, "sid").unwrap_or_else(|| panic!("没有 sid：{cmd}"));
    assert_eq!(sid["value"], "abc");
    assert_eq!(sid["domain"], "127.0.0.1");
    assert_eq!(sid["httpOnly"], true);
    assert!(
        sid["expirationDate"].is_i64(),
        "sid 应当带 expirationDate：{sid}"
    );
    let theme = cookie(&cmd, "theme").unwrap_or_else(|| panic!("没有 theme：{cmd}"));
    assert_eq!(theme["value"], "dark");
    assert_eq!(theme["domain"], ".example.com");
    assert!(
        theme.get("expirationDate").is_none(),
        "会话 Cookie 不带 expirationDate：{theme}"
    );
    assert!(cookie(&cmd, "old").is_none(), "过期的 old 不应导入：{cmd}");

    answer_cookies(&mut env.shell, &cmd, 2, 0);
    let (_, done) = wait_done(&mut env.ws, id);
    assert_eq!(done["status"], "done", "import.done：{done}");
    assert_eq!(done["counts"]["cookies"], 2, "import.done：{done}");
    assert_eq!(done["counts"]["skipped"], 0, "import.done：{done}");
    let n: i64 = db(home.path())
        .query_row(
            "SELECT cookies_count FROM imports WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 2);
    drop(env.daemon);
}

/// RC N1 R1.S2 Electron 拒绝写入的 Cookie 计入跳过：回报 imported 1、rejected 1 → cookies=1、skipped=1。
#[test]
fn rc_n1_r1_s2_rejected_cookies_are_skipped() {
    let home = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    fixture_profile::standard_cookies(profile.path());
    let mut env = cookie_env(home.path(), &[]);

    let id = start_import(
        &mut env.ws,
        json!({"source": "chrome", "profilePath": profile.path(), "spaceId": env.work, "items": ["cookies"]}),
    );
    let cmd = next_import_cookies(&mut env.shell);
    answer_cookies(&mut env.shell, &cmd, 1, 1);
    let (_, done) = wait_done(&mut env.ws, id);
    assert_eq!(done["status"], "done", "import.done：{done}");
    assert_eq!(done["counts"]["cookies"], 1, "import.done：{done}");
    assert_eq!(done["counts"]["skipped"], 1, "import.done：{done}");
}

/// 导入 `profile` 的 Cookie 并如实回报（Electron 端收到几项就回报写入几项）；返回 (shell.importCookies, import.done)。
fn import_cookies_answering(env: &mut CookieEnv, params: Value) -> (Value, Value) {
    let id = start_import(&mut env.ws, params);
    let cmd = next_import_cookies(&mut env.shell);
    let n = cmd["cookies"].as_array().unwrap().len() as u32;
    answer_cookies(&mut env.shell, &cmd, n, 0);
    let (_, done) = wait_done(&mut env.ws, id);
    (cmd, done)
}

/// RC N1 R1.S3 Arc 用 Arc Safe Storage：counts.cookies=2，日志 `取 Safe Storage 口令` 的 service 为 Arc Safe Storage。
#[test]
fn rc_n1_r1_s3_arc_uses_arc_safe_storage() {
    let home = tempfile::tempdir().unwrap();
    let arc = tempfile::tempdir().unwrap();
    fixture_profile::standard_cookies(&arc.path().join("Default"));
    let mut env = cookie_env(
        home.path(),
        &[("ALTER_ARC_DIR", arc.path().to_str().unwrap())],
    );

    let params = json!({"source": "arc", "spaceId": env.work, "items": ["cookies"]});
    let (_, done) = import_cookies_answering(&mut env, params);
    assert_eq!(done["counts"]["cookies"], 2, "import.done：{done}");
    drop(env.ws);
    let stderr = env.daemon.stop_and_stderr();
    let services: Vec<String> = stderr
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["fields"]["message"] == "取 Safe Storage 口令")
        .map(|v| v["fields"]["service"].as_str().unwrap_or("").to_owned())
        .collect();
    assert_eq!(
        services,
        vec!["Arc Safe Storage".to_owned()],
        "stderr：{stderr}"
    );
}

/// RC N1 R1.S4 新版库的主机名前缀：sid 的值恰好为 abc；前缀与 host_key 不符的 bad 被跳过。
#[test]
fn rc_n1_r1_s4_host_prefix_of_new_db() {
    let home = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    fixture_profile::write_cookies(
        profile.path(),
        24,
        &[
            CookieRow::new("127.0.0.1", "sid", "abc"),
            CookieRow {
                encrypted: Some(fixture_profile::encrypt_v10(
                    &fixture_profile::with_host_prefix("other.example", "x"),
                )),
                ..CookieRow::new("127.0.0.1", "bad", "")
            },
        ],
    );
    let mut env = cookie_env(home.path(), &[]);

    let params = json!({"source": "chrome", "profilePath": profile.path(), "spaceId": env.work, "items": ["cookies"]});
    let (cmd, done) = import_cookies_answering(&mut env, params);
    assert_eq!(
        cookie(&cmd, "sid").unwrap()["value"],
        "abc",
        "shell.importCookies：{cmd}"
    );
    assert!(
        cookie(&cmd, "bad").is_none(),
        "前缀不符的 bad 不应导入：{cmd}"
    );
    assert!(
        done["counts"]["skipped"].as_i64().unwrap() >= 1,
        "import.done：{done}"
    );
}

/// RC N1 R3.S1 一行密文被截断：其余 2 条照常导入，skipped=1。
#[test]
fn rc_n1_r3_s1_truncated_ciphertext_is_skipped() {
    let home = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let mut rows = fixture_profile::standard_cookie_rows();
    rows.push(CookieRow {
        encrypted: Some(b"v10\x01\x02\x03\x04\x05\x06\x07".to_vec()),
        ..CookieRow::new("127.0.0.1", "broken", "")
    });
    fixture_profile::write_cookies(profile.path(), 24, &rows);
    let mut env = cookie_env(home.path(), &[]);

    let params = json!({"source": "chrome", "profilePath": profile.path(), "spaceId": env.work, "items": ["cookies"]});
    let (_, done) = import_cookies_answering(&mut env, params);
    assert_eq!(done["status"], "done", "import.done：{done}");
    assert_eq!(done["counts"]["cookies"], 2, "import.done：{done}");
    assert_eq!(done["counts"]["skipped"], 1, "import.done：{done}");
}

/// RC N1 R3.S2 明文值不加密：encrypted_value 为空时用 value 列的 p1。
#[test]
fn rc_n1_r3_s2_plaintext_value() {
    let home = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    fixture_profile::write_cookies(
        profile.path(),
        24,
        &[CookieRow {
            encrypted: Some(Vec::new()),
            ..CookieRow::new("127.0.0.1", "plain", "p1")
        }],
    );
    let mut env = cookie_env(home.path(), &[]);

    let params = json!({"source": "chrome", "profilePath": profile.path(), "spaceId": env.work, "items": ["cookies"]});
    let (cmd, done) = import_cookies_answering(&mut env, params);
    assert_eq!(
        cookie(&cmd, "plain").unwrap()["value"],
        "p1",
        "shell.importCookies：{cmd}"
    );
    assert_eq!(done["counts"]["cookies"], 1, "import.done：{done}");
}

/// RC N1 T4 边界：Cookie 全部过期时不发 shell.importCookies，导入 done、cookies=0。
#[test]
fn rc_n1_t4_all_expired_sends_nothing() {
    let home = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    fixture_profile::write_cookies(
        profile.path(),
        24,
        &[CookieRow {
            expires_ms: Some(fixture_profile::now_ms() - fixture_profile::DAY_MS),
            ..CookieRow::new("127.0.0.1", "old", "1")
        }],
    );
    let mut env = cookie_env(home.path(), &[]);

    let id = start_import(
        &mut env.ws,
        json!({"source": "chrome", "profilePath": profile.path(), "spaceId": env.work, "items": ["cookies"]}),
    );
    let (_, done) = wait_done(&mut env.ws, id);
    assert_eq!(done["status"], "done", "import.done：{done}");
    assert_eq!(done["counts"]["cookies"], 0, "import.done：{done}");
    while let Some(msg) = env.shell.recv_within(Duration::from_millis(300)) {
        assert_ne!(msg["type"], "shell.importCookies", "不应下发：{msg}");
    }
}

/// RC N1 R2.S1 拒绝授权：书签与历史照常导入，cookies=0，status=partial、warning=keychain_denied，不下发 Cookie。
#[test]
fn rc_n1_r2_s1_keychain_denied_is_partial() {
    let home = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    fixture_profile::standard(profile.path());
    fixture_profile::standard_cookies(profile.path());
    let mut env = cookie_env(home.path(), &[("ALTER_KEYCHAIN_FAKE", "denied")]);

    let id = start_import(
        &mut env.ws,
        json!({"source": "chrome", "profilePath": profile.path(), "spaceId": env.work,
               "items": ["bookmarks", "history", "cookies"]}),
    );
    let (_, done) = wait_done(&mut env.ws, id);
    assert_eq!(done["status"], "partial", "import.done：{done}");
    assert_eq!(done["warning"], "keychain_denied", "import.done：{done}");
    assert_eq!(
        done["counts"],
        json!({"bookmarks": 3, "history": 2, "cookies": 0, "skipped": 0}),
        "import.done：{done}"
    );
    while let Some(msg) = env.shell.recv_within(Duration::from_millis(300)) {
        assert_ne!(msg["type"], "shell.importCookies", "不应下发：{msg}");
    }
    let st = rpc(&mut env.ws, 2, "import.status", json!({"importId": id}));
    assert_eq!(st["result"]["status"], "partial", "import.status：{st}");
    assert_eq!(
        st["result"]["warning"], "keychain_denied",
        "import.status：{st}"
    );
}

/// RC N1 R4.S1 alterd 独立运行（没有 Shell 连接，或连接没上报 cdpReady）：partial、warning=shell_unavailable。
#[test]
fn rc_n1_r4_s1_no_shell_is_partial() {
    let profile = tempfile::tempdir().unwrap();
    fixture_profile::standard(profile.path());
    fixture_profile::standard_cookies(profile.path());
    for with_shell in [false, true] {
        let home = tempfile::tempdir().unwrap();
        let _d = Daemon::start_with_env(home.path(), &[("ALTER_KEYCHAIN_FAKE", COOKIE_PASSWORD)]);
        // 有连接但不发 cdpReady：同样算没有可写入的浏览器窗口
        let _shell = with_shell.then(|| {
            let mut c = Client::connect(home.path());
            c.recv();
            c
        });
        let mut ws = gateway_client(home.path());
        let spaces = rpc(&mut ws, 1, "space.list", json!({}));
        let personal = spaces["result"]["spaces"][0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let (_, _, done) = run_import(
            &mut ws,
            json!({"source": "chrome", "profilePath": profile.path(), "spaceId": personal,
                   "items": ["bookmarks", "cookies"]}),
        );
        assert_eq!(
            done["status"], "partial",
            "with_shell={with_shell}，import.done：{done}"
        );
        assert_eq!(
            done["warning"], "shell_unavailable",
            "with_shell={with_shell}，import.done：{done}"
        );
        assert_eq!(
            done["counts"]["bookmarks"], 3,
            "with_shell={with_shell}，import.done：{done}"
        );
        assert_eq!(
            done["counts"]["cookies"], 0,
            "with_shell={with_shell}，import.done：{done}"
        );
    }
}

/// RC N1 R4.S2 Electron 收到 shell.importCookies 后不回复：时限 500 ms，2 秒内 partial、shell_unavailable。
#[test]
fn rc_n1_r4_s2_no_answer_is_partial() {
    let home = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    fixture_profile::standard_cookies(profile.path());
    let mut env = cookie_env(home.path(), &[("ALTER_IMPORT_COOKIE_TIMEOUT_MS", "500")]);

    let id = start_import(
        &mut env.ws,
        json!({"source": "chrome", "profilePath": profile.path(), "spaceId": env.work, "items": ["cookies"]}),
    );
    next_import_cookies(&mut env.shell);
    let t0 = std::time::Instant::now();
    let msg = loop {
        let left = Duration::from_secs(2).saturating_sub(t0.elapsed());
        let msg = ws_recv_within(&mut env.ws, left).expect("2 秒内应当收到 import.done");
        if msg["method"] == "import.done" && msg["params"]["importId"] == id {
            break msg;
        }
    };
    let done = &msg["params"];
    assert_eq!(done["status"], "partial", "import.done：{done}");
    assert_eq!(done["warning"], "shell_unavailable", "import.done：{done}");
    assert_eq!(done["counts"]["cookies"], 0, "import.done：{done}");
}

/// `dir` 下（递归）内容里含 `needle` 的文件。
fn files_containing(dir: &Path, needle: &[u8]) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(files_containing(&path, needle));
        } else if let Ok(bytes) = std::fs::read(&path)
            && bytes.windows(needle.len()).any(|w| w == needle)
        {
            found.push(path);
        }
    }
    found
}

/// RC N1 R5.S1 导入后找不到明文：stderr 不含值与口令，ALTER_HOME 下没有文件含值，import-tmp 已删或为空。
#[test]
fn rc_n1_r5_s1_no_plaintext_left() {
    const SECRET: &str = "s3cr3t-value-7f2c";
    let home = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    // `value` 列本来就是明文的行：导入过程中也不能在 ALTER_HOME 下多出一份
    const PLAIN: &str = "pl41n-v4lue-9c1e";
    let mut rows = fixture_profile::standard_cookie_rows();
    rows[0].value = SECRET;
    rows.push(CookieRow {
        encrypted: Some(Vec::new()),
        ..CookieRow::new("127.0.0.1", "plain2", PLAIN)
    });
    fixture_profile::write_cookies(profile.path(), 24, &rows);
    let mut env = cookie_env(home.path(), &[]);

    let id = start_import(
        &mut env.ws,
        json!({"source": "chrome", "profilePath": profile.path(), "spaceId": env.work, "items": ["cookies"]}),
    );
    let cmd = next_import_cookies(&mut env.shell);
    // 解密已完成、Main 还没回报：此刻 ALTER_HOME 下也不能有含明文的文件
    let during = files_containing(home.path(), PLAIN.as_bytes());
    assert!(
        during.is_empty(),
        "导入过程中这些文件含明文 Cookie：{during:?}"
    );
    let n = cmd["cookies"].as_array().unwrap().len() as u32;
    answer_cookies(&mut env.shell, &cmd, n, 0);
    let (_, done) = wait_done(&mut env.ws, id);
    assert_eq!(
        cookie(&cmd, "sid").unwrap()["value"],
        SECRET,
        "前提：值确实被解出来了"
    );
    assert_eq!(
        cookie(&cmd, "plain2").unwrap()["value"],
        PLAIN,
        "前提：明文行确实被导入了"
    );
    assert_eq!(done["status"], "done", "import.done：{done}");
    drop(env.ws);
    drop(env.shell);
    let stderr = env.daemon.stop_and_stderr();
    assert!(
        !stderr.contains(SECRET),
        "stderr 不应含 Cookie 明文：{stderr}"
    );
    assert!(
        !stderr.contains(COOKIE_PASSWORD),
        "stderr 不应含口令：{stderr}"
    );
    let leaked = files_containing(home.path(), SECRET.as_bytes());
    assert!(leaked.is_empty(), "这些文件含 Cookie 明文：{leaked:?}");
    let tmp = home.path().join("import-tmp");
    assert!(
        !tmp.exists() || std::fs::read_dir(&tmp).unwrap().next().is_none(),
        "import-tmp 应当已删除或为空，实际内容：{:?}",
        std::fs::read_dir(&tmp).map(|d| d.flatten().map(|e| e.path()).collect::<Vec<_>>())
    );
}

/// 再连一个假 Electron 端并上报 cdpReady（它成为「最近一次上报 cdpReady」的连接）。
fn second_shell(home: &Path) -> (Client, FakeCdp) {
    let mut c = Client::connect(home);
    c.recv();
    let cdp = FakeCdp::start(home, &mut c);
    (c, cdp)
}

/// `shell` 在 `ms` 毫秒内没有收到 `shell.importCookies`。
fn no_import_cookies(shell: &mut Client, ms: u64) {
    while let Some(msg) = shell.recv_within(Duration::from_millis(ms)) {
        assert_ne!(msg["type"], "shell.importCookies", "不应收到：{msg}");
    }
}

/// RC N1 T14 只发给最近一次上报 cdpReady 的连接；别的连接或计数不对的回报都不算；它断开后不退回旧连接。
#[test]
fn rc_n1_t14_only_latest_cdp_connection_counts() {
    let home = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    fixture_profile::standard_cookies(profile.path());
    let mut env = cookie_env(home.path(), &[("ALTER_IMPORT_COOKIE_TIMEOUT_MS", "1500")]);
    let (mut b, _cdp_b) = second_shell(home.path());
    let params = json!({"source": "chrome", "profilePath": profile.path(), "spaceId": env.work, "items": ["cookies"]});

    let id = start_import(&mut env.ws, params.clone());
    let cmd = next_import_cookies(&mut b);
    no_import_cookies(&mut env.shell, 200);
    // A（不是收到这一批的连接）抢先回报，B 回报的计数之和不等于批大小：都不接受
    answer_cookies(&mut env.shell, &cmd, 1, 1);
    answer_cookies(&mut b, &cmd, 5, 0);
    answer_cookies(&mut b, &cmd, 2, 0);
    let (_, done) = wait_done(&mut env.ws, id);
    assert_eq!(done["status"], "done", "import.done：{done}");
    assert_eq!(done["counts"]["cookies"], 2, "import.done：{done}");
    assert_eq!(done["counts"]["skipped"], 0, "import.done：{done}");

    // B 断开：不退回 A
    drop(b);
    std::thread::sleep(Duration::from_millis(200));
    let id = start_import(&mut env.ws, params);
    let (_, done) = wait_done(&mut env.ws, id);
    assert_eq!(done["status"], "partial", "import.done：{done}");
    assert_eq!(done["warning"], "shell_unavailable", "import.done：{done}");
    no_import_cookies(&mut env.shell, 200);
}

/// RC N1 T14 导入期间目标 Space 被删除：不再下发后面的批次，导入 failed。
#[test]
fn rc_n1_t14_space_deleted_mid_import() {
    let home = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let rows: Vec<CookieRow> = (0..1000)
        .map(|i| {
            CookieRow::new(
                "127.0.0.1",
                Box::leak(format!("c{i}").into_boxed_str()),
                "v",
            )
        })
        .collect();
    fixture_profile::write_cookies(profile.path(), 24, &rows);
    let mut env = cookie_env(home.path(), &[]);

    let id = start_import(
        &mut env.ws,
        json!({"source": "chrome", "profilePath": profile.path(), "spaceId": env.work, "items": ["cookies"]}),
    );
    let first = next_import_cookies(&mut env.shell);
    env.shell
        .send(json!({"type": "shell.event.spaceDeleted", "spaceId": env.work}));
    answer_cookies(&mut env.shell, &first, 500, 0);
    let (_, done) = wait_done(&mut env.ws, id);
    assert_eq!(done["status"], "failed", "import.done：{done}");
    let st = rpc(&mut env.ws, 2, "import.status", json!({"importId": id}));
    assert_eq!(
        st["result"]["error"], "目标 Space 已被删除",
        "import.status：{st}"
    );
    no_import_cookies(&mut env.shell, 300);
}

/// RC N1 T15 库里 samesite 的 -1/0/1/2 依次映射为 unspecified、no_restriction、lax、strict。
#[test]
fn rc_n1_t15_same_site_mapping() {
    let home = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let rows: Vec<CookieRow> = [
        (-1, "s_unspec"),
        (0, "s_none"),
        (1, "s_lax"),
        (2, "s_strict"),
    ]
    .into_iter()
    .map(|(v, name)| CookieRow {
        same_site: v,
        ..CookieRow::new("127.0.0.1", name, "x")
    })
    .collect();
    fixture_profile::write_cookies(profile.path(), 24, &rows);
    let mut env = cookie_env(home.path(), &[]);

    let params = json!({"source": "chrome", "profilePath": profile.path(), "spaceId": env.work, "items": ["cookies"]});
    let (cmd, _) = import_cookies_answering(&mut env, params);
    let got = |name: &str| cookie(&cmd, name).unwrap()["sameSite"].clone();
    assert_eq!(got("s_unspec"), "unspecified", "{cmd}");
    assert_eq!(got("s_none"), "no_restriction", "{cmd}");
    assert_eq!(got("s_lax"), "lax", "{cmd}");
    assert_eq!(got("s_strict"), "strict", "{cmd}");
}
