//! shell-control / spaces / tabs 能力的契约测试：真实的 `alterd run` 子进程 + 假 Electron 客户端
//! （Unix socket 按行读写 JSON），数据目录用临时目录隔离。

mod common;

use std::time::Duration;

use common::{
    Client, Daemon, connected, create_space, expect_error, expect_silence, expect_snapshot,
    open_new, sock_path, space_id, space_names, v1_db,
};
use serde_json::{Value, json};

/// R1.S1 新连接收到快照：只有 Personal 一个 Space，没有标签。
#[test]
fn r1_s1_new_connection_receives_snapshot() {
    let home = tempfile::tempdir().unwrap();
    let _d = Daemon::start(home.path());

    let mut c = Client::connect(home.path());
    let snap = c.recv();
    assert_eq!(snap["type"], "shell.stateChanged", "快照：{snap}");
    let spaces = snap["spaces"].as_array().expect("spaces 应当是数组");
    assert_eq!(spaces.len(), 1, "快照：{snap}");
    assert_eq!(spaces[0]["name"], "Personal");
    assert_eq!(spaces[0]["color"], "#6366f1");
    assert_eq!(snap["activeSpaceId"], spaces[0]["id"]);
    assert_eq!(snap["tabs"], json!([]));
}

/// R1.S2 socket 只允许当前用户访问：权限位 0600。
#[test]
fn r1_s2_socket_mode_is_0600() {
    use std::os::unix::fs::PermissionsExt;
    let home = tempfile::tempdir().unwrap();
    let _d = Daemon::start(home.path());

    let mode = std::fs::metadata(sock_path(home.path()))
        .expect("shell.sock 应当存在")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600, "权限位：{:o}", mode & 0o777);
}

/// R4.S1 重启不重复创建：同一 ALTER_HOME 两次启动，快照里都只有同一个 Personal。
#[test]
fn r4_s1_restart_keeps_single_default_space() {
    let home = tempfile::tempdir().unwrap();

    let d = Daemon::start(home.path());
    let first = Client::connect(home.path()).recv();
    let first_id = first["spaces"][0]["id"].clone();
    assert!(first_id.is_string(), "快照：{first}");
    d.terminate();

    let _d = Daemon::start(home.path());
    let second = Client::connect(home.path()).recv();
    let spaces = second["spaces"].as_array().unwrap();
    assert_eq!(spaces.len(), 1, "快照：{second}");
    assert_eq!(spaces[0]["name"], "Personal");
    assert_eq!(spaces[0]["id"], first_id);
}

/// R5.S1 带协议的 URL 原样打开（去掉首尾空白）。
#[test]
fn r5_s1_url_with_scheme_is_kept() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, space) = connected(home.path());
    assert_eq!(
        open_new(&mut c, &space, "  https://example.com/a?x=1  "),
        "https://example.com/a?x=1"
    );
}

/// R5.S2 裸域名补 https。
#[test]
fn r5_s2_bare_domain_gets_https() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, space) = connected(home.path());
    assert_eq!(
        open_new(&mut c, &space, "example.com/docs"),
        "https://example.com/docs"
    );
}

/// R5.S3 localhost 与 IP 补 http。
#[test]
fn r5_s3_localhost_and_ip_get_http() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, space) = connected(home.path());
    assert_eq!(
        open_new(&mut c, &space, "localhost:8080"),
        "http://localhost:8080"
    );
    assert_eq!(
        open_new(&mut c, &space, "127.0.0.1:3000/x"),
        "http://127.0.0.1:3000/x"
    );
}

/// R5.S4 其余内容走 Google 搜索，按 UTF-8 百分号编码。
#[test]
fn r5_s4_other_input_becomes_google_search() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, space) = connected(home.path());
    assert_eq!(
        open_new(&mut c, &space, "rust 生命周期"),
        "https://www.google.com/search?q=rust%20%E7%94%9F%E5%91%BD%E5%91%A8%E6%9C%9F"
    );
}

/// R5.S5 空输入被拒绝：回 E_INVALID_ARGUMENT，之后 1 秒内没有新快照。
#[test]
fn r5_s5_blank_input_is_rejected() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, space) = connected(home.path());
    c.send(json!({"type": "shell.event.openRequested", "spaceId": space, "input": "   "}));
    let err = c.recv();
    assert_eq!(err["type"], "shell.error", "收到：{err}");
    assert_eq!(err["code"], "E_INVALID_ARGUMENT");
    assert!(
        c.recv_within(Duration::from_secs(1)).is_none(),
        "不应再收到消息"
    );
}

/// R5（需求正文）`about:` 开头的输入原样使用。
#[test]
fn r5_about_prefix_is_kept() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, space) = connected(home.path());
    assert_eq!(open_new(&mut c, &space, "about:blank"), "about:blank");
}

/// R2.S1 两个连接都收到变化。
#[test]
fn r2_s1_both_connections_receive_change() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut a, space) = connected(home.path());
    let mut b = Client::connect(home.path());
    b.recv();

    a.send(json!({"type": "shell.event.openRequested", "spaceId": space, "input": "https://example.com/a"}));
    for c in [&mut a, &mut b] {
        let snap = c.recv();
        assert_eq!(snap["type"], "shell.stateChanged", "收到：{snap}");
        let tabs = snap["tabs"].as_array().unwrap();
        assert_eq!(tabs.len(), 1, "快照：{snap}");
        assert_eq!(tabs[0]["url"], "https://example.com/a");
    }
}

/// R2.S2 无法解析的行被丢弃且连接保持。
#[test]
fn r2_s2_malformed_line_is_dropped() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, space) = connected(home.path());
    c.send_raw("not json");
    assert_eq!(
        open_new(&mut c, &space, "https://example.com/b"),
        "https://example.com/b"
    );
}

/// R2.S3 被拒绝的事件只回错误给发送方。
#[test]
fn r2_s3_rejection_goes_to_sender_only() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut a, _space) = connected(home.path());
    let mut b = Client::connect(home.path());
    b.recv();

    a.send(json!({"type": "shell.event.openRequested", "spaceId": "01JZZZZZZZZZZZZZZZZZZZZZZZ", "input": "example.com"}));
    let err = a.recv();
    assert_eq!(err["type"], "shell.error", "收到：{err}");
    assert_eq!(err["code"], "E_SPACE_NOT_FOUND");
    assert!(
        b.recv_within(Duration::from_secs(1)).is_none(),
        "B 不应收到消息"
    );
}

/// R3.S1 socket 路径过长：2 秒内退出码 1，日志 code=1 且提到 shell.sock，不写健康文件。
#[test]
fn r3_s1_socket_path_too_long() {
    let base = std::env::temp_dir();
    let base_len = base.as_os_str().len();
    // 目录路径总长 120 字节：temp_dir + 前缀 + tempfile 追加的 6 个随机字符
    let prefix = "x".repeat(120 - base_len - 6);
    let home = tempfile::Builder::new()
        .prefix(&prefix)
        .rand_bytes(6)
        .tempdir_in(&base)
        .unwrap();
    assert_eq!(
        home.path().as_os_str().len(),
        120,
        "{}",
        home.path().display()
    );

    let mut d = Daemon::spawn(home.path());
    let status = d.wait(Duration::from_secs(2)).expect("2 秒内应当退出");
    assert_eq!(status.code(), Some(1), "退出状态：{status:?}");
    let stderr = d.stderr();
    let logged = stderr.lines().any(|l| {
        serde_json::from_str::<Value>(l)
            .is_ok_and(|v| v["fields"]["code"] == 1 && v.to_string().contains("shell.sock"))
    });
    assert!(logged, "stderr：{stderr}");
    assert!(!home.path().join("alterd.json").exists());
}

/// R8（错误分支）回写一个不存在的标签：回 E_TAB_NOT_FOUND。
#[test]
fn r8_update_unknown_tab_is_rejected() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, _space) = connected(home.path());
    c.send(json!({"type": "shell.event.tabUpdated", "tabId": "01JZZZZZZZZZZZZZZZZZZZZZZZ", "title": "x"}));
    let err = c.recv();
    assert_eq!(err["type"], "shell.error", "收到：{err}");
    assert_eq!(err["code"], "E_TAB_NOT_FOUND");
}

/// R7.S2 指定标签时下发导航指令：回 shell.navigate，标签数量不变。
#[test]
fn r7_s2_open_in_tab_sends_navigate() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, space) = connected(home.path());
    open_new(&mut c, &space, "https://example.com/a");
    // 读回标签 id：再连一个客户端拿快照
    let snap = Client::connect(home.path()).recv();
    let tab_id = snap["tabs"][0]["id"].as_str().unwrap().to_owned();

    c.send(json!({"type": "shell.event.openRequested", "spaceId": space, "input": "example.com", "tabId": tab_id}));
    let nav = c.recv();
    assert_eq!(
        nav,
        json!({"type": "shell.navigate", "tabId": tab_id, "url": "https://example.com"})
    );
    let after = Client::connect(home.path()).recv();
    assert_eq!(after["tabs"].as_array().unwrap().len(), 1, "快照：{after}");
}

/// 快照里各标签的 (url, active)。
fn tab_states(snap: &Value) -> Vec<(String, bool)> {
    snap["tabs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["url"].as_str().unwrap().to_owned(),
                t["active"].as_bool().unwrap(),
            )
        })
        .collect()
}

/// N1 R9.S1（原 R10.S1）关闭中间的激活标签：先激活 A 再激活 B，关闭 B 后剩下 A、C，C（下方相邻）激活；B 的 archived_at 不为 NULL。
#[test]
fn n1_r9_s1_close_active_middle_tab_activates_next() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, space) = connected(home.path());
    for input in [
        "https://a.example",
        "https://b.example",
        "https://c.example",
    ] {
        open_new(&mut c, &space, input);
    }
    let snap = Client::connect(home.path()).recv();
    let id = |i: usize| snap["tabs"][i]["id"].as_str().unwrap().to_owned();
    let (a_id, b, c_id) = (id(0), id(1), id(2));

    // 先激活 A 再激活 B：关闭 B 后，剩下标签里最近激活的是 A，而下方相邻的是 C
    for tab in [&a_id, &b] {
        c.send(json!({"type": "shell.event.tabActivated", "tabId": tab}));
        c.recv();
    }
    c.send(json!({"type": "shell.event.tabClosed", "tabId": b}));
    let after = c.recv();
    assert_eq!(
        tab_states(&after),
        vec![
            ("https://a.example".to_owned(), false),
            ("https://c.example".to_owned(), true),
        ],
        "快照：{after}"
    );
    assert_eq!(after["tabs"][1]["id"], c_id.as_str());

    let db = rusqlite::Connection::open_with_flags(
        home.path().join("alter.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let archived: Option<i64> = db
        .query_row("SELECT archived_at FROM tabs WHERE id = ?1", [&b], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(archived.is_some(), "B 的 archived_at 应当不为 NULL");
}

/// R2（CR3）连续两个事件：不先读就连发两条 openRequested，两个连接都依次收到标签数为 1、2 的两份快照。
#[test]
fn r2_consecutive_events_each_get_snapshot() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut a, space) = connected(home.path());
    let mut b = Client::connect(home.path());
    b.recv();

    for i in 0..2 {
        a.send(json!({"type": "shell.event.openRequested", "spaceId": space, "input": format!("https://example.com/{i}")}));
    }
    for c in [&mut a, &mut b] {
        let counts: Vec<usize> = (0..2)
            .map(|_| c.recv()["tabs"].as_array().unwrap().len())
            .collect();
        assert_eq!(counts, vec![1, 2]);
    }
}

/// R2.S4 慢连接积压时仍收到最新快照：B 停止读取期间 A 连发 300 条，B 之后读到 300 个标签的快照。
#[test]
fn r2_s4_slow_connection_still_gets_latest() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut a, space) = connected(home.path());
    let mut b = Client::connect(home.path());
    b.recv();

    for i in 0..300 {
        a.send(json!({"type": "shell.event.openRequested", "spaceId": space, "input": format!("https://example.com/{i}")}));
        a.recv();
    }

    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    let mut latest = 0;
    while latest < 300 && std::time::Instant::now() < deadline {
        match b.recv_within(Duration::from_millis(200)) {
            Some(snap) => latest = snap["tabs"].as_array().map_or(0, Vec::len),
            None => break,
        }
    }
    assert_eq!(latest, 300, "B 最后读到的快照只有 {latest} 个标签");
}

/// R5.S6 blob URL 原样打开。
#[test]
fn r5_s6_blob_url_is_kept() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, space) = connected(home.path());
    let blob = "blob:https://example.com/0f6c1e2a-5b7d-4c3e-9a1f-2d8e7b6c5a40";
    assert_eq!(open_new(&mut c, &space, blob), blob);
}

/// R5（验证缺口）不是「主机名[:端口][/路径]」形式的输入走搜索：`example.com?q=x`。
#[test]
fn r5_query_without_path_is_search() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, space) = connected(home.path());
    assert_eq!(
        open_new(&mut c, &space, "example.com?q=x"),
        "https://www.google.com/search?q=example.com%3Fq%3Dx"
    );
}

/// R7（验证缺口）带已存在的 tabId、但 spaceId 不存在：回 E_SPACE_NOT_FOUND。
#[test]
fn r7_open_in_tab_with_unknown_space_is_rejected() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, space) = connected(home.path());
    open_new(&mut c, &space, "https://example.com/a");
    let tab_id = Client::connect(home.path()).recv()["tabs"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    c.send(json!({"type": "shell.event.openRequested", "spaceId": "01JZZZZZZZZZZZZZZZZZZZZZZZ", "input": "example.com", "tabId": tab_id}));
    let err = c.recv();
    assert_eq!(err["type"], "shell.error", "收到：{err}");
    assert_eq!(err["code"], "E_SPACE_NOT_FOUND");
}

/// N1 R1.S1 新建后成为当前 Space：名称去掉首尾空白，排在末尾，颜色照传。
#[test]
fn n1_r1_s1_created_space_becomes_active() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, _) = connected(home.path());

    let snap = create_space(&mut c, "  Work ", "#f97316");
    assert_eq!(space_names(&snap), ["Personal", "Work"]);
    assert_eq!(snap["spaces"][1]["color"], "#f97316");
    assert_eq!(snap["activeSpaceId"], space_id(&snap, "Work").as_str());
}

/// N1 R1.S2 名称为空被拒绝：回 E_INVALID_ARGUMENT，之后 1 秒内没有新快照。
#[test]
fn n1_r1_s2_blank_name_is_rejected() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, _) = connected(home.path());

    c.send(json!({"type": "shell.event.spaceCreated", "name": "   ", "color": "#f97316"}));
    expect_error(&mut c, "E_INVALID_ARGUMENT");
    expect_silence(&mut c);
}

/// N1 R1.S3 名称超过 32 个字符被拒绝；恰好 32 个字符（含多字节字符）可以。
#[test]
fn n1_r1_s3_name_longer_than_32_is_rejected() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, _) = connected(home.path());

    c.send(json!({"type": "shell.event.spaceCreated", "name": "a".repeat(33), "color": "#f97316"}));
    expect_error(&mut c, "E_INVALID_ARGUMENT");
    // 32 个汉字是 96 个字节，按字符计数应当被接受
    let snap = create_space(&mut c, &"工".repeat(32), "#f97316");
    assert_eq!(snap["spaces"].as_array().unwrap().len(), 2);
}

/// N1 R1.S4 非预设颜色被拒绝。
#[test]
fn n1_r1_s4_unknown_color_is_rejected() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, _) = connected(home.path());

    c.send(json!({"type": "shell.event.spaceCreated", "name": "Work", "color": "#123456"}));
    expect_error(&mut c, "E_INVALID_ARGUMENT");
}

/// 不存在的 ULID，用于「Space / 标签不存在」的场景。
const MISSING_ID: &str = "01JZZZZZZZZZZZZZZZZZZZZZZZ";

/// N1 R2.S1 改名：id、顺序、颜色不变，当前 Space 仍是 Personal。
#[test]
fn n1_r2_s1_rename_keeps_id_and_active_space() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, personal) = connected(home.path());
    let work = space_id(&create_space(&mut c, "Work", "#f97316"), "Work");
    c.send(json!({"type": "shell.event.spaceActivated", "spaceId": personal}));
    expect_snapshot(&mut c);

    c.send(json!({"type": "shell.event.spaceUpdated", "spaceId": work, "name": "Office"}));
    let snap = expect_snapshot(&mut c);
    assert_eq!(space_names(&snap), ["Personal", "Office"]);
    assert_eq!(snap["spaces"][1]["id"], work.as_str());
    assert_eq!(snap["spaces"][1]["color"], "#f97316");
    assert_eq!(snap["activeSpaceId"], personal.as_str());
}

/// N1 R2.S2 编辑不存在的 Space：回 E_SPACE_NOT_FOUND。
#[test]
fn n1_r2_s2_update_unknown_space_is_rejected() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, _) = connected(home.path());

    c.send(json!({"type": "shell.event.spaceUpdated", "spaceId": MISSING_ID, "name": "X"}));
    expect_error(&mut c, "E_SPACE_NOT_FOUND");
}

/// N1 R3.S1 切换改变当前 Space。
#[test]
fn n1_r3_s1_activate_changes_active_space() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, personal) = connected(home.path());
    let snap = create_space(&mut c, "Work", "#f97316");
    assert_eq!(snap["activeSpaceId"], space_id(&snap, "Work").as_str());

    c.send(json!({"type": "shell.event.spaceActivated", "spaceId": personal}));
    let snap = expect_snapshot(&mut c);
    assert_eq!(snap["activeSpaceId"], personal.as_str());
}

/// N1 R3.S2 切换到不存在的 Space：回 E_SPACE_NOT_FOUND。
#[test]
fn n1_r3_s2_activate_unknown_space_is_rejected() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, _) = connected(home.path());

    c.send(json!({"type": "shell.event.spaceActivated", "spaceId": MISSING_ID}));
    expect_error(&mut c, "E_SPACE_NOT_FOUND");
}

/// N1 R4.S1 重启保留当前 Space：Work 成为当前 Space 后 SIGTERM 再启动，第一份快照的当前 Space 仍是 Work。
#[test]
fn n1_r4_s1_restart_keeps_active_space() {
    let home = tempfile::tempdir().unwrap();
    let (d, mut c, _) = connected(home.path());
    let work = space_id(&create_space(&mut c, "Work", "#f97316"), "Work");
    drop(c);
    d.terminate();

    let _d = Daemon::start(home.path());
    let snap = Client::connect(home.path()).recv();
    assert_eq!(snap["activeSpaceId"], work.as_str(), "快照：{snap}");
}

/// N1 R4.S2 旧数据库没有记录时取第一个 Space：只应用过 0001 的库里有 Personal（0）与 Work（1）。
#[test]
fn n1_r4_s2_legacy_db_activates_first_space() {
    let home = tempfile::tempdir().unwrap();
    v1_db(home.path())
        .execute_batch(
            "INSERT INTO spaces (id, name, position, created_at) VALUES ('s-personal', 'Personal', 0, 0);
             INSERT INTO spaces (id, name, position, created_at) VALUES ('s-work', 'Work', 1, 0);",
        )
        .unwrap();

    let _d = Daemon::start(home.path());
    let snap = Client::connect(home.path()).recv();
    assert_eq!(snap["activeSpaceId"], "s-personal", "快照：{snap}");
}

/// 发 spaceActivated 并读掉随后的快照。
fn activate_space(c: &mut Client, space: &str) -> Value {
    c.send(json!({"type": "shell.event.spaceActivated", "spaceId": space}));
    expect_snapshot(c)
}

/// 发 spaceDeleted 并返回随后的快照。
fn delete_space(c: &mut Client, space: &str) -> Value {
    c.send(json!({"type": "shell.event.spaceDeleted", "spaceId": space}));
    expect_snapshot(c)
}

/// N1 R5.S1 删除中间的当前 Space：后一个 Space 成为当前，库里 Space 与它的标签都没了。
#[test]
fn n1_r5_s1_delete_active_middle_space_activates_next() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, _) = connected(home.path());
    let work = space_id(&create_space(&mut c, "Work", "#f97316"), "Work");
    let play = space_id(&create_space(&mut c, "Play", "#22c55e"), "Play");
    activate_space(&mut c, &work);
    open_new(&mut c, &work, "https://example.com/w");

    let snap = delete_space(&mut c, &work);
    assert_eq!(space_names(&snap), ["Personal", "Play"]);
    assert_eq!(snap["activeSpaceId"], play.as_str());

    let db = rusqlite::Connection::open(home.path().join("alter.db")).unwrap();
    let count = |sql: &str| -> i64 { db.query_row(sql, [&work], |r| r.get(0)).unwrap() };
    assert_eq!(count("SELECT COUNT(*) FROM spaces WHERE id = ?1"), 0);
    assert_eq!(count("SELECT COUNT(*) FROM tabs WHERE space_id = ?1"), 0);
}

/// N1 R5.S2 删除末尾的当前 Space：没有后一个，前一个成为当前。
#[test]
fn n1_r5_s2_delete_active_last_space_activates_previous() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, personal) = connected(home.path());
    let work = space_id(&create_space(&mut c, "Work", "#f97316"), "Work");

    let snap = delete_space(&mut c, &work);
    assert_eq!(snap["activeSpaceId"], personal.as_str());
}

/// N1 R5.S3 删除非当前 Space：当前 Space 不变。
#[test]
fn n1_r5_s3_delete_inactive_space_keeps_active() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, personal) = connected(home.path());
    let work = space_id(&create_space(&mut c, "Work", "#f97316"), "Work");
    activate_space(&mut c, &personal);

    let snap = delete_space(&mut c, &work);
    assert_eq!(space_names(&snap), ["Personal"]);
    assert_eq!(snap["activeSpaceId"], personal.as_str());
}

/// N1 R5.S4 不能删除最后一个 Space：回 E_INVALID_ARGUMENT，之后 1 秒内没有新快照；
/// 不存在的 Space 回 E_SPACE_NOT_FOUND（接口契约）。
#[test]
fn n1_r5_s4_last_space_cannot_be_deleted() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, personal) = connected(home.path());

    c.send(json!({"type": "shell.event.spaceDeleted", "spaceId": MISSING_ID}));
    expect_error(&mut c, "E_SPACE_NOT_FOUND");
    c.send(json!({"type": "shell.event.spaceDeleted", "spaceId": personal}));
    expect_error(&mut c, "E_INVALID_ARGUMENT");
    expect_silence(&mut c);
}

/// N1 R6.S1 启动时只删除孤儿目录：`alterd.json` 出现时，不属于任何现存 Space 的
/// `space-<id>` 已被删除，现存 Space 的目录（Electron 用小写）与其他目录保留。
#[test]
fn n1_r6_s1_startup_sweeps_orphan_partitions() {
    let home = tempfile::tempdir().unwrap();
    let (d, c, personal) = connected(home.path());
    drop(c);
    d.terminate();

    let partitions = home.path().join("chromium/Partitions");
    let own = partitions.join(format!("space-{}", personal.to_lowercase()));
    let orphan = partitions.join("space-01jzzzzzzzzzzzzzzzzzzzzzzz");
    // 需求写明不区分大小写：大写 id 的孤儿目录也要删
    let orphan_upper = partitions.join("space-01JYYYYYYYYYYYYYYYYYYYYYYY");
    let other = partitions.join("default");
    for dir in [&own, &orphan, &orphan_upper, &other] {
        std::fs::create_dir_all(dir.join("Cookies-dir")).unwrap();
    }

    // Daemon::start 在 alterd.json 出现后立即返回，此时检查目录
    let _d = Daemon::start(home.path());
    assert!(!orphan.exists(), "孤儿目录应当已被删除");
    assert!(!orphan_upper.exists(), "大写 id 的孤儿目录也应当已被删除");
    assert!(own.exists(), "现存 Space 的目录应当保留");
    assert!(other.exists(), "非 space-<id> 目录应当保留");
}

/// 依次新建标签，返回最后一份快照。
fn open_tabs(c: &mut Client, space: &str, urls: &[&str]) -> Value {
    let mut snap = Value::Null;
    for url in urls {
        c.send(json!({"type": "shell.event.openRequested", "spaceId": space, "input": url}));
        snap = expect_snapshot(c);
    }
    snap
}

/// 快照里 url 为 `url` 的标签的 id。
fn tab_id(snap: &Value, url: &str) -> String {
    snap["tabs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["url"] == url)
        .unwrap_or_else(|| panic!("快照里没有标签 {url}：{snap}"))["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// 快照里各标签的 (url, pinned)，按快照顺序。
fn tab_pins(snap: &Value) -> Vec<(String, bool)> {
    snap["tabs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["url"].as_str().unwrap().to_owned(),
                t["pinned"].as_bool().unwrap(),
            )
        })
        .collect()
}

fn pins(xs: &[(&str, bool)]) -> Vec<(String, bool)> {
    xs.iter().map(|(u, p)| ((*u).to_owned(), *p)).collect()
}

fn pin(c: &mut Client, tab: &str, pinned: bool) -> Value {
    c.send(json!({"type": "shell.event.tabPinned", "tabId": tab, "pinned": pinned}));
    expect_snapshot(c)
}

fn move_tab(c: &mut Client, tab: &str, pinned: bool, index: u32) -> Value {
    c.send(json!({"type": "shell.event.tabMoved", "tabId": tab, "pinned": pinned, "index": index}));
    expect_snapshot(c)
}

/// 直接读库里某个标签的一列。
fn tab_column<T: rusqlite::types::FromSql>(home: &std::path::Path, id: &str, column: &str) -> T {
    let db = rusqlite::Connection::open(home.join("alter.db")).unwrap();
    db.query_row(
        &format!("SELECT {column} FROM tabs WHERE id = ?1"),
        [id],
        |r| r.get(0),
    )
    .unwrap()
}

const A: &str = "https://example.com/a";
const B: &str = "https://example.com/b";
const C: &str = "https://example.com/c";

/// N1 R8.S1 固定后移到固定分区：C 排到最前，库里记下 pinned_url。
#[test]
fn n1_r8_s1_pin_moves_tab_to_pinned_section() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, space) = connected(home.path());
    let snap = open_tabs(&mut c, &space, &[A, B, C]);
    let c_id = tab_id(&snap, C);

    let snap = pin(&mut c, &c_id, true);
    assert_eq!(tab_pins(&snap), pins(&[(C, true), (A, false), (B, false)]));
    assert_eq!(tab_column::<String>(home.path(), &c_id, "pinned_url"), C);
}

/// N1 R8.S2 取消固定后移到普通分区开头。
#[test]
fn n1_r8_s2_unpin_moves_tab_to_head_of_normal_section() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, space) = connected(home.path());
    let (p, q) = ("https://example.com/p", "https://example.com/q");
    let snap = open_tabs(&mut c, &space, &[p, q, A]);
    pin(&mut c, &tab_id(&snap, p), true);
    let snap = pin(&mut c, &tab_id(&snap, q), true);
    assert_eq!(tab_pins(&snap), pins(&[(p, true), (q, true), (A, false)]));

    let snap = pin(&mut c, &tab_id(&snap, p), false);
    assert_eq!(tab_pins(&snap), pins(&[(q, true), (p, false), (A, false)]));
}

/// N1 R8.S3 固定不存在的标签：回 E_TAB_NOT_FOUND。
#[test]
fn n1_r8_s3_pin_unknown_tab_is_rejected() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, _) = connected(home.path());

    c.send(json!({"type": "shell.event.tabPinned", "tabId": MISSING_ID, "pinned": true}));
    expect_error(&mut c, "E_TAB_NOT_FOUND");
}

/// N1 R11.S1 在普通分区内移动：C 移到第 0 位。
#[test]
fn n1_r11_s1_move_within_normal_section() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, space) = connected(home.path());
    let snap = open_tabs(&mut c, &space, &[A, B, C]);

    let snap = move_tab(&mut c, &tab_id(&snap, C), false, 0);
    assert_eq!(tab_pins(&snap), pins(&[(C, false), (A, false), (B, false)]));
}

/// N1 R11.S2 拖入固定分区即固定：B 放到固定分区第 1 位，库里记下 pinned_url。
#[test]
fn n1_r11_s2_move_into_pinned_section_pins() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, space) = connected(home.path());
    let p = "https://example.com/p";
    let snap = open_tabs(&mut c, &space, &[p, A, B]);
    pin(&mut c, &tab_id(&snap, p), true);
    let b = tab_id(&snap, B);

    let snap = move_tab(&mut c, &b, true, 1);
    assert_eq!(tab_pins(&snap), pins(&[(p, true), (B, true), (A, false)]));
    assert_eq!(tab_column::<String>(home.path(), &b, "pinned_url"), B);
}

/// N1 R11.S3 index 超出范围时放到末尾。
#[test]
fn n1_r11_s3_index_out_of_range_moves_to_end() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, space) = connected(home.path());
    let snap = open_tabs(&mut c, &space, &[A, B, C]);

    let snap = move_tab(&mut c, &tab_id(&snap, A), false, 99);
    assert_eq!(tab_pins(&snap), pins(&[(B, false), (C, false), (A, false)]));
}

/// N1 R11.S4 移动不存在的标签：回 E_TAB_NOT_FOUND。
#[test]
fn n1_r11_s4_move_unknown_tab_is_rejected() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, _) = connected(home.path());

    c.send(
        json!({"type": "shell.event.tabMoved", "tabId": MISSING_ID, "pinned": false, "index": 0}),
    );
    expect_error(&mut c, "E_TAB_NOT_FOUND");
}

/// N1 R10.S1 关闭固定标签：回 shell.discard；行保留，url 回到固定时的地址，激活转给 A；不归档。
#[test]
fn n1_r10_s1_closing_pinned_tab_discards_it() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, space) = connected(home.path());
    let (p_url, q_url) = ("https://example.com/p", "https://example.com/q");
    let snap = open_tabs(&mut c, &space, &[p_url, A]);
    let p = tab_id(&snap, p_url);
    pin(&mut c, &p, true);
    c.send(json!({"type": "shell.event.tabUpdated", "tabId": p, "url": q_url}));
    expect_snapshot(&mut c);
    c.send(json!({"type": "shell.event.tabActivated", "tabId": p}));
    expect_snapshot(&mut c);

    c.send(json!({"type": "shell.event.tabClosed", "tabId": p}));
    // 私有回复先于同一事件的快照（CR9）
    let discard = c.recv();
    assert_eq!(discard["type"], "shell.discard", "收到：{discard}");
    assert_eq!(discard["tabId"], p.as_str());
    let snap = &expect_snapshot(&mut c);
    assert_eq!(tab_pins(snap), pins(&[(p_url, true), (A, false)]));
    assert_eq!(
        tab_states(snap),
        vec![(p_url.to_owned(), false), (A.to_owned(), true)]
    );
    assert_eq!(
        tab_column::<Option<i64>>(home.path(), &p, "archived_at"),
        None
    );
}

/// N1 CR3 关闭普通标签后剩下的标签连续编号：关闭 A、B、C 中的 B 后 A、C 为 0、1，再新建 D 为 2。
#[test]
fn n1_cr3_close_keeps_positions_contiguous() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, space) = connected(home.path());
    let snap = open_tabs(&mut c, &space, &[A, B, C]);
    c.send(json!({"type": "shell.event.tabClosed", "tabId": tab_id(&snap, B)}));
    expect_snapshot(&mut c);
    let d_url = "https://example.com/d";
    let snap = open_tabs(&mut c, &space, &[d_url]);

    let pos = |url: &str| tab_column::<i64>(home.path(), &tab_id(&snap, url), "position");
    assert_eq!((pos(A), pos(C), pos(d_url)), (0, 1, 2));
}

/// N1 CR9 私有回复先于同一事件的快照：重复 30 次「固定 → 激活 → 关闭」，
/// 每次关闭后的第一条消息都是 shell.discard，第二条才是快照。
#[test]
fn n1_cr9_discard_precedes_snapshot() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, space) = connected(home.path());
    let snap = open_tabs(&mut c, &space, &[A, B]);
    let a = tab_id(&snap, A);
    pin(&mut c, &a, true);

    for round in 0..30 {
        c.send(json!({"type": "shell.event.tabActivated", "tabId": a}));
        expect_snapshot(&mut c);
        c.send(json!({"type": "shell.event.tabClosed", "tabId": a}));
        let first = c.recv();
        assert_eq!(
            first["type"], "shell.discard",
            "第 {round} 次关闭后先收到：{first}"
        );
        expect_snapshot(&mut c);
    }
}

/// N1 CR12 消息按 alterd 处理事件的顺序写出：不读消息连发 tabUpdated（/qN）与关闭固定标签，
/// 依次收到「url 为 /qN 的快照 → shell.discard → url 回到固定地址的快照」，重复 30 次。
#[test]
fn n1_cr12_replies_keep_order_with_earlier_snapshots() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, space) = connected(home.path());
    let p_url = "https://example.com/p";
    let snap = open_tabs(&mut c, &space, &[p_url]);
    let p = tab_id(&snap, p_url);
    pin(&mut c, &p, true);

    for round in 0..30 {
        // 上一轮关闭后 P 已卸载，它的回写会被忽略（CR17）：先激活它
        c.send(json!({"type": "shell.event.tabActivated", "tabId": p}));
        expect_snapshot(&mut c);
        let q = format!("https://example.com/q{round}");
        c.send(json!({"type": "shell.event.tabUpdated", "tabId": p, "url": q}));
        c.send(json!({"type": "shell.event.tabClosed", "tabId": p}));
        let msgs = [c.recv(), c.recv(), c.recv()];
        let kinds: Vec<_> = msgs.iter().map(|m| m["type"].as_str().unwrap()).collect();
        assert_eq!(
            kinds,
            ["shell.stateChanged", "shell.discard", "shell.stateChanged"],
            "第 {round} 轮"
        );
        assert_eq!(msgs[0]["tabs"][0]["url"], q.as_str(), "第 {round} 轮");
        assert_eq!(msgs[2]["tabs"][0]["url"], p_url, "第 {round} 轮");
    }
}

/// N1 CR15 每个连接按处理顺序写出：不读消息连发 400 对「tabUpdated（/qN）+ 关闭不存在的标签」，
/// 读到的 800 条必须严格交替为「url 为 /qN 的快照、shell.error」，中间不丢、不越序。
#[test]
fn n1_cr15_messages_follow_processing_order_under_backlog() {
    use std::io::Write;

    let home = tempfile::tempdir().unwrap();
    // 400 次被拒绝各写一行 WARN：stderr 用管道会写满卡住，这里丢弃
    let _d = Daemon::start_quiet(home.path());
    let mut c = Client::connect(home.path());
    let space = c.recv()["spaces"][0]["id"].as_str().unwrap().to_owned();
    let snap = open_tabs(&mut c, &space, &[A]);
    let a = tab_id(&snap, A);

    let mut w = c.writer();
    let sender = std::thread::spawn(move || {
        for n in 0..400 {
            let update = json!({"type": "shell.event.tabUpdated", "tabId": a, "url": format!("https://example.com/q{n}")});
            let close = json!({"type": "shell.event.tabClosed", "tabId": MISSING_ID});
            writeln!(w, "{update}\n{close}").unwrap();
        }
    });
    // 先不读，让连接积压
    std::thread::sleep(Duration::from_millis(500));
    for n in 0..400 {
        let s = c
            .recv_within(Duration::from_secs(1))
            .unwrap_or_else(|| panic!("第 {n} 对：1 秒内没有收到快照"));
        assert_eq!(s["type"], "shell.stateChanged", "第 {n} 对的第一条：{s}");
        assert_eq!(
            s["tabs"][0]["url"],
            format!("https://example.com/q{n}"),
            "第 {n} 对"
        );
        let e = c
            .recv_within(Duration::from_secs(1))
            .unwrap_or_else(|| panic!("第 {n} 对：1 秒内没有收到错误"));
        assert_eq!(e["type"], "shell.error", "第 {n} 对的第二条：{e}");
    }
    sender.join().unwrap();
}

/// N1 CR15 慢连接不丢快照：B 停止读取期间 A 连开 300 个标签，B 之后按顺序收到标签数为 1…300 的
/// 全部 300 份快照（每个连接一条先进先出队列，不再因落后而跳过）。
#[test]
fn n1_cr15_slow_connection_receives_every_snapshot_in_order() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut a, space) = connected(home.path());
    let mut b = Client::connect(home.path());
    b.recv();

    for i in 0..300 {
        a.send(json!({"type": "shell.event.openRequested", "spaceId": space, "input": format!("https://example.com/{i}")}));
        a.recv();
    }

    for n in 1..=300 {
        let snap = b
            .recv_within(Duration::from_secs(2))
            .unwrap_or_else(|| panic!("B 在第 {n} 份快照处 2 秒内没有收到消息"));
        let len = snap["tabs"].as_array().map_or(0, Vec::len);
        assert_eq!(len, n, "B 的第 {n} 份快照");
    }
}

/// N1 CR17 已卸载的固定标签由 alterd 记录：关闭后快照里 discarded 为 true；此时迟到的 tabUpdated
/// 被忽略（不广播、库里 url 不变）；再次激活后 discarded 为 false，url 为固定时的地址。
#[test]
fn n1_cr17_discarded_flag_round_trip() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, space) = connected(home.path());
    let p_url = "https://example.com/p";
    let snap = open_tabs(&mut c, &space, &[p_url, A]);
    let p = tab_id(&snap, p_url);
    pin(&mut c, &p, true);
    let snap = expect_discarded_after_close(&mut c, &p);
    assert_eq!(snap["tabs"][0]["discarded"], true, "快照：{snap}");
    assert_eq!(snap["tabs"][1]["discarded"], false, "快照：{snap}");

    c.send(json!({"type": "shell.event.tabUpdated", "tabId": p, "url": "https://example.com/x"}));
    expect_silence(&mut c);
    assert_eq!(tab_column::<String>(home.path(), &p, "url"), p_url);

    c.send(json!({"type": "shell.event.tabActivated", "tabId": p}));
    let snap = expect_snapshot(&mut c);
    assert_eq!(snap["tabs"][0]["discarded"], false, "快照：{snap}");
    assert_eq!(snap["tabs"][0]["url"], p_url);
}

/// 关闭固定标签：先收到 shell.discard，再返回随后的快照。
fn expect_discarded_after_close(c: &mut Client, tab: &str) -> Value {
    c.send(json!({"type": "shell.event.tabClosed", "tabId": tab}));
    let discard = c.recv();
    assert_eq!(discard["type"], "shell.discard", "收到：{discard}");
    expect_snapshot(c)
}

/// N1 CR24 在已卸载的标签里打开地址：不回 shell.navigate（没有页面可导航），而是改 url、清除卸载状态并广播。
#[test]
fn n1_cr24_open_in_discarded_tab_reloads_it() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, space) = connected(home.path());
    let p_url = "https://example.com/p";
    let snap = open_tabs(&mut c, &space, &[p_url]);
    let p = tab_id(&snap, p_url);
    pin(&mut c, &p, true);
    expect_discarded_after_close(&mut c, &p);

    c.send(json!({"type": "shell.event.openRequested", "spaceId": space, "input": "example.com/z", "tabId": p}));
    let snap = expect_snapshot(&mut c);
    assert_eq!(snap["tabs"][0]["id"], p.as_str());
    assert_eq!(snap["tabs"][0]["discarded"], false, "快照：{snap}");
    assert_eq!(snap["tabs"][0]["url"], "https://example.com/z");
}

/// N1 CR27 已卸载的标签不算激活：Space 只有固定标签 P，关闭后快照里 P 的 active 为 false；
/// 再激活后 active 为 true、discarded 为 false。
#[test]
fn n1_cr27_sole_discarded_tab_is_not_active() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, space) = connected(home.path());
    let p_url = "https://example.com/p";
    let snap = open_tabs(&mut c, &space, &[p_url]);
    let p = tab_id(&snap, p_url);
    pin(&mut c, &p, true);

    let snap = expect_discarded_after_close(&mut c, &p);
    assert_eq!(snap["tabs"][0]["active"], false, "快照：{snap}");

    c.send(json!({"type": "shell.event.tabActivated", "tabId": p}));
    let snap = expect_snapshot(&mut c);
    assert_eq!(snap["tabs"][0]["active"], true, "快照：{snap}");
    assert_eq!(snap["tabs"][0]["discarded"], false, "快照：{snap}");
}

/// 某个 Space 的历史，按访问顺序：(url, title, source)。
fn history_of(home: &std::path::Path, space: &str) -> Vec<(String, String, String)> {
    let db = rusqlite::Connection::open_with_flags(
        home.join("alter.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let mut stmt = db
        .prepare(
            "SELECT url, title, source FROM history WHERE space_id = ?1 ORDER BY visited_at, id",
        )
        .unwrap();
    stmt.query_map([space], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

/// N4 R1.S1 导航与标题写入历史：每次带 url 的回写追加一条，之后的标题回写更新最新那条。
#[test]
fn n4_r1_s1_navigation_and_title_write_history() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, personal) = connected(home.path());
    open_new(&mut c, &personal, "https://example.com/start");
    let tab: String = rusqlite::Connection::open(home.path().join("alter.db"))
        .unwrap()
        .query_row("SELECT id FROM tabs LIMIT 1", [], |r| r.get(0))
        .unwrap();
    for ev in [
        json!({"type": "shell.event.tabUpdated", "tabId": tab, "url": "https://example.com/a"}),
        json!({"type": "shell.event.tabUpdated", "tabId": tab, "title": "A"}),
        json!({"type": "shell.event.tabUpdated", "tabId": tab, "url": "https://example.com/b"}),
    ] {
        c.send(ev);
        expect_snapshot(&mut c);
    }
    let alter = "alter".to_owned();
    assert_eq!(
        history_of(home.path(), &personal),
        vec![
            (
                "https://example.com/a".to_owned(),
                "A".to_owned(),
                alter.clone()
            ),
            ("https://example.com/b".to_owned(), "A".to_owned(), alter),
        ]
    );
}

/// N4 标题回写只更新同 URL 最新的那一条历史，不动之前的记录。
#[test]
fn n4_title_update_touches_only_latest_entry() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, personal) = connected(home.path());
    open_new(&mut c, &personal, "https://example.com/start");
    let tab: String = rusqlite::Connection::open(home.path().join("alter.db"))
        .unwrap()
        .query_row("SELECT id FROM tabs LIMIT 1", [], |r| r.get(0))
        .unwrap();
    for ev in [
        json!({"type": "shell.event.tabUpdated", "tabId": tab, "url": "https://example.com/a"}),
        json!({"type": "shell.event.tabUpdated", "tabId": tab, "title": "A"}),
        json!({"type": "shell.event.tabUpdated", "tabId": tab, "url": "https://example.com/b"}),
        json!({"type": "shell.event.tabUpdated", "tabId": tab, "title": "B"}),
        json!({"type": "shell.event.tabUpdated", "tabId": tab, "url": "https://example.com/a"}),
        json!({"type": "shell.event.tabUpdated", "tabId": tab, "title": "A2"}),
    ] {
        c.send(ev);
        expect_snapshot(&mut c);
    }
    let titles: Vec<_> = history_of(home.path(), &personal)
        .into_iter()
        .map(|(url, title, _)| (url, title))
        .collect();
    assert_eq!(
        titles,
        vec![
            ("https://example.com/a".to_owned(), "A".to_owned()),
            ("https://example.com/b".to_owned(), "B".to_owned()),
            ("https://example.com/a".to_owned(), "A2".to_owned()),
        ]
    );
}

/// 新开一个标签并回写它的地址与标题，返回标签 id。
fn tab_with(c: &mut Client, home: &std::path::Path, space: &str, url: &str, title: &str) -> String {
    open_new(c, space, url);
    let tab: String = rusqlite::Connection::open(home.join("alter.db"))
        .unwrap()
        .query_row(
            "SELECT id FROM tabs WHERE archived_at IS NULL ORDER BY rowid DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    c.send(json!({"type": "shell.event.tabUpdated", "tabId": tab, "title": title}));
    expect_snapshot(c);
    tab
}

/// 发一次搜索，返回 `shell.searchResults` 的 items。
fn search(c: &mut Client, request_id: u32, query: &str) -> Vec<Value> {
    c.send(json!({"type": "shell.event.searchRequested", "requestId": request_id, "query": query}));
    let r = c.recv();
    assert_eq!(r["type"], "shell.searchResults", "收到：{r}");
    assert_eq!(r["requestId"], request_id);
    r["items"].as_array().unwrap().clone()
}

/// (kind, title) 序列。
fn kinds_titles(items: &[Value]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|i| {
            (
                i["kind"].as_str().unwrap().to_owned(),
                i["title"].as_str().unwrap_or("").to_owned(),
            )
        })
        .collect()
}

/// N5 R11.S1（承接 N4 R3.S1）结果的来源与顺序：标签、Space、历史，最后「打开或搜索」。
#[test]
fn n5_r11_s1_sources_and_order() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, personal) = connected(home.path());
    tab_with(
        &mut c,
        home.path(),
        &personal,
        "https://docs.example.com/",
        "Docs Home",
    );
    // 历史里的 Docs Archive：开过、回写过，然后关掉
    let old = tab_with(&mut c, home.path(), &personal, "https://example.com/x", "x");
    c.send(json!({"type": "shell.event.tabUpdated", "tabId": old, "url": "https://docs.example.com/old"}));
    expect_snapshot(&mut c);
    c.send(json!({"type": "shell.event.tabUpdated", "tabId": old, "title": "Docs Archive"}));
    expect_snapshot(&mut c);
    c.send(json!({"type": "shell.event.tabClosed", "tabId": old}));
    expect_snapshot(&mut c);
    create_space(&mut c, "Docs", "#3b82f6");

    let items = search(&mut c, 1, "docs");
    assert_eq!(
        kinds_titles(&items),
        vec![
            ("tab".to_owned(), "Docs Home".to_owned()),
            ("space".to_owned(), "Docs".to_owned()),
            ("history".to_owned(), "Docs Archive".to_owned()),
            (
                "open".to_owned(),
                items[3]["title"].as_str().unwrap_or("").to_owned()
            ),
        ],
        "{items:?}"
    );
    assert_eq!(items[3]["input"], "docs");
    assert_eq!(items[0]["url"], "https://docs.example.com/");
    assert_eq!(items[2]["url"], "https://docs.example.com/old");
}

/// N5 R11.S2（承接 N4 R3.S2）已打开的 URL 不重复出现在历史结果里。
#[test]
fn n5_r11_s2_open_url_not_repeated_in_history() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, personal) = connected(home.path());
    let tab = tab_with(&mut c, home.path(), &personal, "https://example.com/x", "x");
    c.send(
        json!({"type": "shell.event.tabUpdated", "tabId": tab, "url": "https://docs.example.com/"}),
    );
    expect_snapshot(&mut c);
    c.send(json!({"type": "shell.event.tabUpdated", "tabId": tab, "title": "Docs Home"}));
    expect_snapshot(&mut c);
    let items = search(&mut c, 2, "docs home");
    assert!(
        !items
            .iter()
            .any(|i| i["kind"] == "history" && i["url"] == "https://docs.example.com/"),
        "{items:?}"
    );
    assert_eq!(items[0]["kind"], "tab", "{items:?}");
}

/// N5 R11.S3（承接 N4 R3.S3）最多 8 项：12 个匹配的标签只给前 8 个，再加「打开或搜索」。
#[test]
fn n5_r11_s3_at_most_eight() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, personal) = connected(home.path());
    for n in 0..12 {
        tab_with(
            &mut c,
            home.path(),
            &personal,
            &format!("https://example.com/{n}"),
            &format!("Page {n}"),
        );
    }
    let items = search(&mut c, 3, "page");
    assert_eq!(items.len(), 9, "{items:?}");
    assert!(items[..8].iter().all(|i| i["kind"] == "tab"), "{items:?}");
    assert_eq!(items[8]["kind"], "open");
}

/// N5 R11.S4（承接 N4 R3.S4）空输入（只有空白）：items 为空数组。
#[test]
fn n5_r11_s4_blank_query() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, _personal) = connected(home.path());
    assert_eq!(search(&mut c, 4, "  "), Vec::<Value>::new());
}

/// N5 R11.S5 书签排在历史之前：当前 Space 的书签 `Rust`、历史 `Rust Blog`，搜 rust 得书签、历史、「打开」。
/// 书签文件夹与其他 Space 的书签不出现。
#[test]
fn n5_r11_s5_bookmarks_before_history() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, personal) = connected(home.path());
    let work = space_id(&create_space(&mut c, "Work", "#f97316"), "Work");
    // 创建 Work 后它成为当前 Space；切回 Personal
    c.send(json!({"type": "shell.event.spaceActivated", "spaceId": personal}));
    expect_snapshot(&mut c);
    {
        let db = rusqlite::Connection::open(home.path().join("alter.db")).unwrap();
        db.execute_batch(&format!(
            "INSERT INTO bookmarks (id, space_id, parent_id, title, url, position, source) VALUES
               ('f1', '{personal}', NULL, 'Rustaceans', NULL, 0, 'chrome'),
               ('b1', '{personal}', NULL, 'Rust', 'https://rust-lang.org/', 1, 'chrome'),
               ('b2', '{work}', NULL, 'Rust at Work', 'https://work.example/rust', 0, 'chrome');
             INSERT INTO history (space_id, url, title, visited_at, source) VALUES
               ('{personal}', 'https://blog.rust-lang.org/', 'Rust Blog', 1000, 'chrome');"
        ))
        .unwrap();
    }

    let items = search(&mut c, 5, "rust");
    let view: Vec<_> = items
        .iter()
        .map(|i| {
            (
                i["kind"].as_str().unwrap().to_owned(),
                i["title"].as_str().unwrap_or("").to_owned(),
            )
        })
        .collect();
    assert_eq!(
        view,
        vec![
            ("bookmark".to_owned(), "Rust".to_owned()),
            ("history".to_owned(), "Rust Blog".to_owned()),
            ("open".to_owned(), "rust".to_owned()),
        ],
        "{items:?}"
    );
    assert_eq!(items[0]["url"], "https://rust-lang.org/");
}

/// 10 万行历史时的搜索耗时（design.md「风险」要求实测）。`#[ignore]`：
/// `cargo test -p alterd --release --test shell n4_search_perf -- --ignored --nocapture`
#[test]
#[ignore]
fn n4_search_perf_100k_history() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, personal) = connected(home.path());
    {
        let mut db = rusqlite::Connection::open(home.path().join("alter.db")).unwrap();
        let tx = db.transaction().unwrap();
        {
            let mut ins = tx
                .prepare("INSERT INTO history (space_id, url, title, visited_at, source) VALUES (?1, ?2, ?3, ?4, 'alter')")
                .unwrap();
            for n in 0..100_000i64 {
                ins.execute(rusqlite::params![
                    personal,
                    format!("https://site{}.example.com/page/{n}", n % 500),
                    format!("Page {n} of site {}", n % 500),
                    1_000_000 + n
                ])
                .unwrap();
            }
        }
        tx.commit().unwrap();
    }
    let mut times = Vec::new();
    for (i, q) in ["site42", "page 99999", "nothing-matches-this", "of site 1"]
        .iter()
        .enumerate()
    {
        let start = std::time::Instant::now();
        let items = search(&mut c, 100 + i as u32, q);
        times.push((q.to_string(), start.elapsed(), items.len()));
    }
    for (q, t, n) in &times {
        println!("搜索 {q:?}：{:.1} ms，{n} 项", t.as_secs_f64() * 1e3);
    }
}

/// 快照里每个标签的 (id, splitGroup)，按快照顺序。
fn split_groups(snap: &Value) -> Vec<(String, Value)> {
    snap["tabs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["id"].as_str().unwrap().to_owned(),
                t["splitGroup"].clone(),
            )
        })
        .collect()
}

/// 在 `space` 里依次开两个标签，返回 (A, B) 的 id；B 是后开的（激活）。
fn two_tabs(c: &mut Client, home: &std::path::Path, space: &str) -> (String, String) {
    let a = tab_with(c, home, space, "https://example.com/a", "A");
    let b = tab_with(c, home, space, "https://example.com/b", "B");
    (a, b)
}

/// N4 R6.S1 组成分屏：两者 splitGroup 相同且不为 null，顺序不变。
#[test]
fn n4_r6_s1_split_two_tabs() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, personal) = connected(home.path());
    let (a, b) = two_tabs(&mut c, home.path(), &personal);
    c.send(json!({"type": "shell.event.tabActivated", "tabId": a}));
    expect_snapshot(&mut c);
    c.send(json!({"type": "shell.event.splitRequested", "tabIds": [a, b]}));
    let snap = expect_snapshot(&mut c);
    assert_eq!(
        split_groups(&snap),
        vec![(a.clone(), json!(a.clone())), (b, json!(a))],
        "{snap}"
    );
}

/// N4 R6.S2 跨 Space 被拒绝。
#[test]
fn n4_r6_s2_cross_space_rejected() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, personal) = connected(home.path());
    let a = tab_with(&mut c, home.path(), &personal, "https://example.com/a", "A");
    let work = space_id(&create_space(&mut c, "Work", "#3b82f6"), "Work");
    let b = tab_with(&mut c, home.path(), &work, "https://example.com/b", "B");
    c.send(json!({"type": "shell.event.splitRequested", "tabIds": [a, b]}));
    expect_error(&mut c, "E_INVALID_ARGUMENT");
}

/// N4 R6.S3 不是恰好两个标签被拒绝。
#[test]
fn n4_r6_s3_needs_exactly_two() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, personal) = connected(home.path());
    let a = tab_with(&mut c, home.path(), &personal, "https://example.com/a", "A");
    c.send(json!({"type": "shell.event.splitRequested", "tabIds": [a]}));
    expect_error(&mut c, "E_INVALID_ARGUMENT");
}

/// N4 R7.S1 关闭分屏中的一个标签：另一个的 splitGroup 变为 null。
#[test]
fn n4_r7_s1_close_one_dissolves() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, personal) = connected(home.path());
    let (a, b) = two_tabs(&mut c, home.path(), &personal);
    c.send(json!({"type": "shell.event.splitRequested", "tabIds": [a, b]}));
    expect_snapshot(&mut c);
    c.send(json!({"type": "shell.event.tabClosed", "tabId": b}));
    let snap = expect_snapshot(&mut c);
    assert_eq!(split_groups(&snap), vec![(a, Value::Null)], "{snap}");
}

/// N4 R7.S2 显式解散：两者的 splitGroup 都为 null。
#[test]
fn n4_r7_s2_dissolve() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, personal) = connected(home.path());
    let (a, b) = two_tabs(&mut c, home.path(), &personal);
    c.send(json!({"type": "shell.event.splitRequested", "tabIds": [a, b]}));
    expect_snapshot(&mut c);
    c.send(json!({"type": "shell.event.splitDissolved", "tabId": a}));
    let snap = expect_snapshot(&mut c);
    assert_eq!(
        split_groups(&snap),
        vec![(a, Value::Null), (b, Value::Null)],
        "{snap}"
    );
}

/// N4 R8.S1 自动归档：超过阈值的普通标签被归档，固定、TaskSpace 托管、激活的标签保留。
#[test]
fn n4_r8_s1_archive_only_eligible_tabs() {
    let home = tempfile::tempdir().unwrap();
    let _d = Daemon::start_with_env(home.path(), &[("ALTER_ARCHIVE_AFTER_MS", "300")]);
    let mut c = Client::connect(home.path());
    let first = c.recv();
    let personal = first["spaces"][0]["id"].as_str().unwrap().to_owned();
    let a = tab_with(&mut c, home.path(), &personal, "https://example.com/a", "A");
    let p = tab_with(&mut c, home.path(), &personal, "https://example.com/p", "P");
    c.send(json!({"type": "shell.event.tabPinned", "tabId": p, "pinned": true}));
    expect_snapshot(&mut c);
    // 被 TaskSpace 托管的 agent 标签 G
    let mut ws = common::gateway_client(home.path());
    let created = common::rpc(&mut ws, 1, "taskSpace.create", json!({"name": "demo"}));
    assert!(created.get("error").is_none(), "{created}");
    let b = tab_with(&mut c, home.path(), &personal, "https://example.com/b", "B");

    // 在 1.5 秒的等待期间持续接收：记下每份快照实际到达的时刻，截止时必须已经收到一份没有 A 的快照
    let deadline = std::time::Instant::now() + Duration::from_millis(1500);
    let mut snap = json!(null);
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            break;
        }
        let Some(m) = c.recv_within(left) else { break };
        let arrived = std::time::Instant::now();
        if arrived <= deadline
            && m["type"] == "shell.stateChanged"
            && !m["tabs"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t["id"] == a.as_str())
        {
            snap = m;
        }
    }
    assert!(!snap.is_null(), "1.5 秒内没有收到不含 A 的快照");
    let archived: Option<i64> = rusqlite::Connection::open(home.path().join("alter.db"))
        .unwrap()
        .query_row("SELECT archived_at FROM tabs WHERE id = ?1", [&a], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(archived.is_some(), "A 没有归档");
    let ids: Vec<_> = snap["tabs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap().to_owned())
        .collect();
    assert!(ids.contains(&p), "固定标签应保留：{snap}");
    assert!(ids.contains(&b), "激活标签应保留：{snap}");
    assert!(
        snap["tabs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["label"] == "p1"),
        "被托管的标签应保留：{snap}"
    );
}

/// N4 R9.S1 关闭的标签出现在已归档列表。
#[test]
fn n4_r9_s1_closed_tab_in_archived_list() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, personal) = connected(home.path());
    let (a, _b) = two_tabs(&mut c, home.path(), &personal);
    c.send(json!({"type": "shell.event.tabClosed", "tabId": a}));
    let snap = expect_snapshot(&mut c);
    let item = snap["archived"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == a.as_str())
        .unwrap_or_else(|| panic!("archived 里没有 A：{snap}"))
        .clone();
    assert_eq!(item["spaceId"], personal.as_str());
    assert_eq!(item["url"], "https://example.com/a");
}

/// N4 R9.S2 恢复：放回普通分区末尾并激活，已归档列表里不再有它。
#[test]
fn n4_r9_s2_restore() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, personal) = connected(home.path());
    let (a, b) = two_tabs(&mut c, home.path(), &personal);
    c.send(json!({"type": "shell.event.tabClosed", "tabId": a}));
    expect_snapshot(&mut c);
    c.send(json!({"type": "shell.event.tabRestored", "tabId": a}));
    let snap = expect_snapshot(&mut c);
    let tabs: Vec<_> = snap["tabs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["id"].as_str().unwrap().to_owned(),
                t["active"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(tabs, vec![(b, false), (a.clone(), true)], "{snap}");
    assert!(
        !snap["archived"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == a.as_str()),
        "{snap}"
    );
}

/// N4 R9.S3 列表上限：一个 Space 关闭 60 个标签，archived 里恰好是最后关闭的 50 个。
#[test]
fn n4_r9_s3_archived_list_limit() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, personal) = connected(home.path());
    let mut ids = Vec::new();
    for n in 0..60 {
        ids.push(tab_with(
            &mut c,
            home.path(),
            &personal,
            &format!("https://example.com/{n}"),
            &format!("T{n}"),
        ));
    }
    let mut snap = json!(null);
    for id in &ids {
        c.send(json!({"type": "shell.event.tabClosed", "tabId": id}));
        snap = expect_snapshot(&mut c);
        // 关闭间隔至少 1 ms，归档时间才有先后
        std::thread::sleep(Duration::from_millis(2));
    }
    // 按归档时间降序：最后关闭的在最前
    let listed: Vec<String> = snap["archived"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["spaceId"] == personal.as_str())
        .map(|t| t["id"].as_str().unwrap().to_owned())
        .collect();
    let expected: Vec<String> = ids[10..].iter().rev().cloned().collect();
    assert_eq!(listed, expected);
}

/// N4 CR4 两个标签先后访问同一 URL：各自的标题回写只更新自己那次导航的历史行。
#[test]
fn n4_cr4_title_update_follows_its_own_navigation() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, personal) = connected(home.path());
    let t1 = tab_with(&mut c, home.path(), &personal, "https://example.com/1", "1");
    let t2 = tab_with(&mut c, home.path(), &personal, "https://example.com/2", "2");
    for ev in [
        json!({"type": "shell.event.tabUpdated", "tabId": t1, "url": "https://example.com/x"}),
        json!({"type": "shell.event.tabUpdated", "tabId": t2, "url": "https://example.com/x"}),
        json!({"type": "shell.event.tabUpdated", "tabId": t1, "title": "one"}),
        json!({"type": "shell.event.tabUpdated", "tabId": t2, "title": "two"}),
    ] {
        c.send(ev);
        expect_snapshot(&mut c);
    }
    let rows: Vec<_> = history_of(home.path(), &personal)
        .into_iter()
        .map(|(url, title, _)| (url, title))
        .collect();
    assert_eq!(
        rows,
        vec![
            ("https://example.com/x".to_owned(), "one".to_owned()),
            ("https://example.com/x".to_owned(), "two".to_owned()),
        ]
    );
}

/// N4 CR5 历史搜索与标签一样不区分大小写（含非 ASCII 字母），且按字面包含匹配。
#[test]
fn n4_cr5_history_search_folds_unicode_case() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, personal) = connected(home.path());
    let t = tab_with(&mut c, home.path(), &personal, "https://example.com/x", "x");
    c.send(
        json!({"type": "shell.event.tabUpdated", "tabId": t, "url": "https://example.com/aerger"}),
    );
    expect_snapshot(&mut c);
    c.send(json!({"type": "shell.event.tabUpdated", "tabId": t, "title": "Ärger im Büro"}));
    expect_snapshot(&mut c);
    c.send(json!({"type": "shell.event.tabClosed", "tabId": t}));
    expect_snapshot(&mut c);
    let items = search(&mut c, 7, "ärger");
    assert!(
        items
            .iter()
            .any(|i| i["kind"] == "history" && i["title"] == "Ärger im Büro"),
        "{items:?}"
    );
    // 字面包含：% 不是通配符
    assert!(
        !search(&mut c, 8, "%")
            .iter()
            .any(|i| i["kind"] == "history"),
        "% 不应匹配任何历史"
    );
}
