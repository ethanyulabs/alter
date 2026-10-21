//! shell-control / spaces / tabs 能力的契约测试：真实的 `alterd run` 子进程 + 假 Electron 客户端
//! （Unix socket 按行读写 JSON），数据目录用临时目录隔离。

mod common;

use std::time::Duration;

use common::{Client, Daemon, connected, open_new, sock_path};
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

/// R10.S1 关闭中间的激活标签：先激活 A 再激活 B，关闭 B 后剩下 A、C，C（下方相邻）激活；B 的 archived_at 不为 NULL。
#[test]
fn r10_s1_close_active_middle_tab_activates_next() {
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
