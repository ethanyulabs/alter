//! gateway / task-space 能力的契约测试：真实的 `alterd run` 子进程 + WebSocket 客户端。

mod common;

use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use common::{
    Client, Daemon, connected, create_space, expect_snapshot, gateway_client, gateway_file, rpc,
    ws_connect,
};
use serde_json::json;

/// N2 R1.S1 写出 gateway.json：alterd.json 出现时它已存在，0600，token 64 位十六进制，两个端口都能连。
#[test]
fn n2_r1_s1_writes_gateway_file() {
    let home = tempfile::tempdir().unwrap();
    let _d = Daemon::start(home.path());

    let path = home.path().join("gateway.json");
    assert!(path.exists(), "alterd.json 出现时 gateway.json 应当已在");
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    let gw = gateway_file(home.path());
    let token = gw["token"].as_str().unwrap();
    assert_eq!(token.len(), 64);
    assert!(
        token.bytes().all(|b| b.is_ascii_hexdigit()),
        "token：{token}"
    );
    assert_eq!(gw["version"], "0.1.0");
    for key in ["port", "cdpPort"] {
        let port = gw[key].as_u64().unwrap();
        TcpStream::connect(("127.0.0.1", port as u16))
            .unwrap_or_else(|e| panic!("{key} {port} 连不上：{e}"));
    }
}

/// N2 R1.S2 退出时删除 gateway.json。
#[test]
fn n2_r1_s2_removes_gateway_file_on_exit() {
    let home = tempfile::tempdir().unwrap();
    let d = Daemon::start(home.path());
    assert!(home.path().join("gateway.json").exists());
    d.terminate();
    assert!(!home.path().join("gateway.json").exists());
}

/// N2 R2.S1 缺少 token：握手 401。
#[test]
fn n2_r2_s1_missing_token_is_401() {
    let home = tempfile::tempdir().unwrap();
    let _d = Daemon::start(home.path());
    assert_eq!(ws_connect(home.path(), None).err(), Some(401));
}

/// N2 R2.S2 token 错误：握手 401。
#[test]
fn n2_r2_s2_wrong_token_is_401() {
    let home = tempfile::tempdir().unwrap();
    let _d = Daemon::start(home.path());
    assert_eq!(ws_connect(home.path(), Some("0000")).err(), Some(401));
}

/// N2 R2.S3 token 一致：hello 返回协议版本 1、预算 8、恰好一个 Personal。
#[test]
fn n2_r2_s3_hello_with_valid_token() {
    let home = tempfile::tempdir().unwrap();
    let _d = Daemon::start(home.path());
    let mut ws = gateway_client(home.path());

    let resp = rpc(
        &mut ws,
        1,
        "hello",
        json!({"clientVersion": "0.1.0", "protocolVersion": 1}),
    );
    assert_eq!(resp["result"]["protocolVersion"], 1, "响应：{resp}");
    assert_eq!(resp["result"]["pageBudget"], 8);
    let spaces = resp["result"]["spaces"].as_array().unwrap();
    assert_eq!(spaces.len(), 1);
    assert_eq!(spaces[0]["name"], "Personal");
}

/// N2 R3.S1 协议版本不匹配：E_PROTOCOL_VERSION。
#[test]
fn n2_r3_s1_protocol_version_mismatch() {
    let home = tempfile::tempdir().unwrap();
    let _d = Daemon::start(home.path());
    let mut ws = gateway_client(home.path());

    let resp = rpc(
        &mut ws,
        1,
        "hello",
        json!({"clientVersion": "0.1.0", "protocolVersion": 2}),
    );
    assert_eq!(
        resp["error"]["data"]["code"], "E_PROTOCOL_VERSION",
        "响应：{resp}"
    );
}

/// N2 R3.S2 未知方法：-32601，E_INVALID_ARGUMENT。
#[test]
fn n2_r3_s2_unknown_method() {
    let home = tempfile::tempdir().unwrap();
    let _d = Daemon::start(home.path());
    let mut ws = gateway_client(home.path());

    let resp = rpc(&mut ws, 1, "page.teleport", json!({}));
    assert_eq!(resp["error"]["code"], -32601, "响应：{resp}");
    assert_eq!(resp["error"]["data"]["code"], "E_INVALID_ARGUMENT");
}

/// N2 R3.S3 参数缺失：E_INVALID_ARGUMENT，message 提到 name。
#[test]
fn n2_r3_s3_missing_param() {
    let home = tempfile::tempdir().unwrap();
    let _d = Daemon::start(home.path());
    let mut ws = gateway_client(home.path());

    let resp = rpc(&mut ws, 1, "taskSpace.create", json!({}));
    assert_eq!(
        resp["error"]["data"]["code"], "E_INVALID_ARGUMENT",
        "响应：{resp}"
    );
    assert!(
        resp["error"]["data"]["message"]
            .as_str()
            .unwrap()
            .contains("name"),
        "响应：{resp}"
    );
}

/// N2 R4.S1 列出两个 Space：Personal（2 个标签）、Work（0 个），顺序同侧边栏。
#[test]
fn n2_r4_s1_space_list_with_tab_counts() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, personal) = connected(home.path());
    for url in ["https://example.com/a", "https://example.com/b"] {
        c.send(json!({"type": "shell.event.openRequested", "spaceId": personal, "input": url}));
        expect_snapshot(&mut c);
    }
    create_space(&mut c, "Work", "#f97316");

    let mut ws = gateway_client(home.path());
    let resp = rpc(&mut ws, 1, "space.list", json!({}));
    let spaces = resp["result"]["spaces"]
        .as_array()
        .unwrap_or_else(|| panic!("响应：{resp}"));
    let got: Vec<(String, u64)> = spaces
        .iter()
        .map(|s| {
            (
                s["name"].as_str().unwrap().to_owned(),
                s["tabCount"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(got, [("Personal".to_owned(), 2), ("Work".to_owned(), 0)]);
    assert!(
        spaces
            .iter()
            .all(|s| s["id"].is_string() && s["color"].is_string())
    );
}

/// 不存在的 ULID。
const MISSING_ID: &str = "01JZZZZZZZZZZZZZZZZZZZZZZZ";

/// 一次调用的 `result`；出错时带响应失败。
fn ok(ws: &mut common::Ws, method: &str, params: serde_json::Value) -> serde_json::Value {
    let resp = rpc(ws, 1, method, params);
    assert!(resp.get("error").is_none(), "{method} 失败：{resp}");
    resp["result"].clone()
}

/// 一次调用的 `error.data`；成功时失败。
fn err(ws: &mut common::Ws, method: &str, params: serde_json::Value) -> serde_json::Value {
    let resp = rpc(ws, 1, method, params);
    assert!(resp.get("error").is_some(), "{method} 应当失败：{resp}");
    resp["error"]["data"].clone()
}

/// 读到下一份满足条件的快照（跳过中间的），1 秒内必须出现。
fn snapshot_where(c: &mut Client, cond: impl Fn(&serde_json::Value) -> bool) -> serde_json::Value {
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        let snap = c
            .recv_within(left.max(Duration::from_millis(1)))
            .expect("1 秒内没有出现符合条件的快照");
        if snap["type"] == "shell.stateChanged" && cond(&snap) {
            return snap;
        }
    }
}

/// 起 alterd，连上 Shell 与 Gateway，在 Personal 里开一个用户标签 A；返回 (daemon, shell, ws, Personal id, A 的 id)。
fn setup_with_tab(home: &std::path::Path) -> (Daemon, Client, common::Ws, String, String) {
    let (d, mut c, personal) = connected(home);
    c.send(json!({"type": "shell.event.openRequested", "spaceId": personal, "input": "https://example.com/a"}));
    let snap = expect_snapshot(&mut c);
    let a = snap["tabs"][0]["id"].as_str().unwrap().to_owned();
    let ws = gateway_client(home);
    (d, c, ws, personal, a)
}

/// N2 R5.S1 在当前 Space 创建：响应带 p1；快照末尾多一个 about:blank 的 agent 标签，A 仍激活。
#[test]
fn n2_r5_s1_create_in_active_space() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, mut ws, personal, a) = setup_with_tab(home.path());

    let r = ok(&mut ws, "taskSpace.create", json!({"name": "demo"}));
    assert!(r["id"].is_i64(), "{r}");
    assert_eq!(r["spaceId"], personal.as_str());
    assert_eq!(r["ownership"], "agent");
    assert_eq!(r["pages"], json!(["p1"]));

    let snap = snapshot_where(&mut c, |s| s["tabs"].as_array().unwrap().len() == 2);
    let tabs = snap["tabs"].as_array().unwrap();
    assert_eq!(tabs[0]["id"], a.as_str());
    assert_eq!(tabs[0]["active"], true, "A 应当仍是激活标签：{snap}");
    assert_eq!(tabs[1]["url"], "about:blank");
    assert_eq!(tabs[1]["taskSpaceId"], r["id"]);
    assert_eq!(tabs[1]["label"], "p1");
    assert_eq!(snap["taskSpaces"][0]["id"], r["id"]);
}

/// N2 R5.S2 Space 不存在：E_SPACE_NOT_FOUND。
#[test]
fn n2_r5_s2_create_in_unknown_space() {
    let home = tempfile::tempdir().unwrap();
    let _d = Daemon::start(home.path());
    let mut ws = gateway_client(home.path());
    let e = err(
        &mut ws,
        "taskSpace.create",
        json!({"name": "demo", "spaceId": MISSING_ID}),
    );
    assert_eq!(e["code"], "E_SPACE_NOT_FOUND");
}

/// N2 R6.S1 跨连接恢复。
#[test]
fn n2_r6_s1_resume_from_another_connection() {
    let home = tempfile::tempdir().unwrap();
    let _d = Daemon::start(home.path());
    let mut a = gateway_client(home.path());
    let created = ok(&mut a, "taskSpace.create", json!({"name": "demo"}));
    drop(a);

    let mut b = gateway_client(home.path());
    let resumed = ok(&mut b, "taskSpace.resume", json!({"id": created["id"]}));
    assert_eq!(resumed, created);
}

/// N2 R6.S2 TaskSpace 不存在：E_TASK_SPACE_NOT_FOUND。
#[test]
fn n2_r6_s2_resume_unknown() {
    let home = tempfile::tempdir().unwrap();
    let _d = Daemon::start(home.path());
    let mut ws = gateway_client(home.path());
    let e = err(&mut ws, "taskSpace.resume", json!({"id": 999}));
    assert_eq!(e["code"], "E_TASK_SPACE_NOT_FOUND");
}

/// N2 R6.S3 列出：a、b 两项，status 都是 active。
#[test]
fn n2_r6_s3_list() {
    let home = tempfile::tempdir().unwrap();
    let _d = Daemon::start(home.path());
    let mut ws = gateway_client(home.path());
    ok(&mut ws, "taskSpace.create", json!({"name": "a"}));
    ok(&mut ws, "taskSpace.create", json!({"name": "b"}));
    let list = ok(&mut ws, "taskSpace.list", json!({}));
    let got: Vec<(String, String)> = list["taskSpaces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["name"].as_str().unwrap().to_owned(),
                t["status"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        got,
        [("a".into(), "active".into()), ("b".into(), "active".into())]
    );
}

/// N2 R7.S1 编号递增：p1 之后是 p2。
#[test]
fn n2_r7_s1_new_page_numbering() {
    let home = tempfile::tempdir().unwrap();
    let _d = Daemon::start(home.path());
    let mut ws = gateway_client(home.path());
    let id = ok(&mut ws, "taskSpace.create", json!({"name": "demo"}))["id"].clone();
    assert_eq!(
        ok(&mut ws, "taskSpace.newPage", json!({"id": id})),
        json!({"label": "p2"})
    );
}

/// N2 R7.S2 超出预算：E_PAGE_BUDGET，details {budget:8, open:8}。
#[test]
fn n2_r7_s2_page_budget() {
    let home = tempfile::tempdir().unwrap();
    let _d = Daemon::start(home.path());
    let mut ws = gateway_client(home.path());
    let id = ok(&mut ws, "taskSpace.create", json!({"name": "demo"}))["id"].clone();
    for _ in 2..=8 {
        ok(&mut ws, "taskSpace.newPage", json!({"id": id}));
    }
    let e = err(&mut ws, "taskSpace.newPage", json!({"id": id}));
    assert_eq!(e["code"], "E_PAGE_BUDGET");
    assert_eq!(e["details"], json!({"budget": 8, "open": 8}));
}

/// N2 R7.S3 关闭后标签不复用：关掉 p2 再开是 p3。
#[test]
fn n2_r7_s3_labels_are_not_reused() {
    let home = tempfile::tempdir().unwrap();
    let _d = Daemon::start(home.path());
    let mut ws = gateway_client(home.path());
    let id = ok(&mut ws, "taskSpace.create", json!({"name": "demo"}))["id"].clone();
    ok(&mut ws, "taskSpace.newPage", json!({"id": id}));
    ok(&mut ws, "page.close", json!({"id": id, "label": "p2"}));
    assert_eq!(
        ok(&mut ws, "taskSpace.newPage", json!({"id": id})),
        json!({"label": "p3"})
    );
}

/// N2 R8.S1 托管与列出：adopt 得到 p2；tabs 里 A 为 p2 / user，另有 p1 / agent。
#[test]
fn n2_r8_s1_adopt_and_tabs() {
    let home = tempfile::tempdir().unwrap();
    let (_d, _c, mut ws, _personal, a) = setup_with_tab(home.path());
    let id = ok(&mut ws, "taskSpace.create", json!({"name": "demo"}))["id"].clone();

    assert_eq!(
        ok(&mut ws, "taskSpace.adopt", json!({"id": id, "tabId": a})),
        json!({"label": "p2"})
    );
    let tabs = ok(&mut ws, "taskSpace.tabs", json!({"id": id}));
    let tabs = tabs["tabs"].as_array().unwrap();
    let a_row = tabs.iter().find(|t| t["tabId"] == a.as_str()).unwrap();
    assert_eq!(
        (&a_row["label"], &a_row["openedBy"]),
        (&json!("p2"), &json!("user"))
    );
    let p1 = tabs.iter().find(|t| t["label"] == "p1").unwrap();
    assert_eq!(p1["openedBy"], "agent");
}

/// N2 R8.S2 已被另一个 TaskSpace 托管：E_INVALID_ARGUMENT。
#[test]
fn n2_r8_s2_adopt_already_adopted() {
    let home = tempfile::tempdir().unwrap();
    let (_d, _c, mut ws, _personal, a) = setup_with_tab(home.path());
    let x = ok(&mut ws, "taskSpace.create", json!({"name": "x"}))["id"].clone();
    let y = ok(&mut ws, "taskSpace.create", json!({"name": "y"}))["id"].clone();
    ok(&mut ws, "taskSpace.adopt", json!({"id": x, "tabId": a}));
    let e = err(&mut ws, "taskSpace.adopt", json!({"id": y, "tabId": a}));
    assert_eq!(e["code"], "E_INVALID_ARGUMENT");
}

/// N2 R8.S3 跨 Space 托管被拒：E_TAB_NOT_FOUND。
#[test]
fn n2_r8_s3_adopt_across_spaces() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, mut ws, personal, _a) = setup_with_tab(home.path());
    let work = common::space_id(&create_space(&mut c, "Work", "#f97316"), "Work");
    c.send(json!({"type": "shell.event.openRequested", "spaceId": work, "input": "https://example.com/b"}));
    let snap = expect_snapshot(&mut c);
    let b = snap["tabs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["spaceId"] == work.as_str())
        .unwrap()["id"]
        .clone();
    let id = ok(
        &mut ws,
        "taskSpace.create",
        json!({"name": "demo", "spaceId": personal}),
    )["id"]
        .clone();
    let e = err(&mut ws, "taskSpace.adopt", json!({"id": id, "tabId": b}));
    assert_eq!(e["code"], "E_TAB_NOT_FOUND");
}

/// N2 R8.S4 归还：tabs 里 A 没有 label；快照里 A 仍在且没有 taskSpaceId。
#[test]
fn n2_r8_s4_release() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, mut ws, _personal, a) = setup_with_tab(home.path());
    let id = ok(&mut ws, "taskSpace.create", json!({"name": "demo"}))["id"].clone();
    ok(&mut ws, "taskSpace.adopt", json!({"id": id, "tabId": a}));
    ok(
        &mut ws,
        "taskSpace.release",
        json!({"id": id, "label": "p2"}),
    );

    let tabs = ok(&mut ws, "taskSpace.tabs", json!({"id": id}));
    let a_row = tabs["tabs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["tabId"] == a.as_str())
        .unwrap()
        .clone();
    assert!(a_row.get("label").is_none(), "{a_row}");
    let snap = snapshot_where(&mut c, |s| {
        s["tabs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == a.as_str() && t.get("taskSpaceId").is_none())
            && s["tabs"].as_array().unwrap().len() == 2
    });
    assert!(
        snap["tabs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == a.as_str())
    );
}

/// N2 R9.S1 结束并保留指定页面：closed [p1]，kept [p2, p3]；p1 的标签不在，p2、p3 的标签在且不再托管；status finished。
#[test]
fn n2_r9_s1_finish_keeps_listed_pages() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, mut ws, _personal, a) = setup_with_tab(home.path());
    let id = ok(&mut ws, "taskSpace.create", json!({"name": "demo"}))["id"].clone();
    ok(&mut ws, "taskSpace.newPage", json!({"id": id}));
    assert_eq!(
        ok(&mut ws, "taskSpace.adopt", json!({"id": id, "tabId": a})),
        json!({"label": "p3"})
    );
    // 等到 adopt 之后的那份快照（newPage 之后也有一份 3 个标签的）
    let snap = snapshot_where(&mut c, |s| {
        s["tabs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["label"] == "p3")
    });
    let tab_of = |label: &str| {
        snap["tabs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["label"] == label)
            .unwrap()["id"]
            .clone()
    };
    let (p1, p2, p3) = (tab_of("p1"), tab_of("p2"), tab_of("p3"));

    let r = ok(
        &mut ws,
        "taskSpace.finish",
        json!({"id": id, "keep": ["p2"]}),
    );
    assert_eq!(r, json!({"closed": ["p1"], "kept": ["p2", "p3"]}));
    let snap = snapshot_where(&mut c, |s| s["tabs"].as_array().unwrap().len() == 2);
    let tabs = snap["tabs"].as_array().unwrap();
    assert!(!tabs.iter().any(|t| t["id"] == p1), "{snap}");
    for t in [&p2, &p3] {
        let row = tabs.iter().find(|x| x["id"] == *t).unwrap();
        assert!(row.get("taskSpaceId").is_none(), "{row}");
    }
    let list = ok(&mut ws, "taskSpace.list", json!({}));
    assert_eq!(list["taskSpaces"][0]["status"], "finished");
}

/// N2 R10.S1 用户关掉 agent 页面后 page.info 回 E_PAGE_NOT_FOUND。
#[test]
fn n2_r10_s1_page_closed_by_user() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, mut ws, _personal, _a) = setup_with_tab(home.path());
    let id = ok(&mut ws, "taskSpace.create", json!({"name": "demo"}))["id"].clone();
    let snap = snapshot_where(&mut c, |s| s["tabs"].as_array().unwrap().len() == 2);
    let t = snap["tabs"][1]["id"].clone();
    c.send(json!({"type": "shell.event.tabClosed", "tabId": t}));
    snapshot_where(&mut c, |s| s["tabs"].as_array().unwrap().len() == 1);

    let e = err(&mut ws, "page.info", json!({"id": id, "label": "p1"}));
    assert_eq!(e["code"], "E_PAGE_NOT_FOUND");
    // 账目也关闭了：p1 不再算未关闭的页面（不占预算）
    let resumed = ok(&mut ws, "taskSpace.resume", json!({"id": id}));
    assert_eq!(resumed["pages"], json!([]), "{resumed}");
}

/// 起 alterd、假 CDP 端与一个 agent 页面 `p1`（targetId T1）；返回 (daemon, shell, ws, 假 CDP 端, TaskSpace id)。
fn fake_cdp_setup(
    home: &std::path::Path,
    responder: common::fake_cdp::Responder,
) -> (Daemon, Client, common::Ws, common::fake_cdp::FakeCdp, i64) {
    let (d, mut c, _personal) = connected(home);
    let cdp = common::fake_cdp::FakeCdp::start_with(home, &mut c, responder);
    let mut ws = gateway_client(home);
    let id = common::fake_cdp::agent_page(&mut c, &mut ws);
    (d, c, ws, cdp, id)
}

/// 收到的命令里方法为 `method` 的那些。
fn sent(cdp: &common::fake_cdp::FakeCdp, method: &str) -> Vec<serde_json::Value> {
    cdp.drain()
        .into_iter()
        .filter(|c| c["method"] == method)
        .collect()
}

/// N2 CR1 taskSpace.cdp 以浏览器会话（不带 sessionId）透传，原样返回结果。
#[test]
fn n2_cr1_task_space_cdp_is_browser_level() {
    let home = tempfile::tempdir().unwrap();
    let (_d, _c, mut ws, cdp, id) = fake_cdp_setup(
        home.path(),
        Box::new(|m, _| (m == "Browser.getVersion").then(|| json!({"product": "Fake/1"}))),
    );
    let r = ok(
        &mut ws,
        "taskSpace.cdp",
        json!({"id": id, "method": "Browser.getVersion", "params": {}}),
    );
    assert_eq!(r["result"]["product"], "Fake/1", "{r}");
    let cmds = sent(&cdp, "Browser.getVersion");
    assert_eq!(cmds.len(), 1);
    assert!(cmds[0].get("sessionId").is_none(), "{}", cmds[0]);
}

/// N2 CR1 page.mouse click：在给定坐标按下、抬起，回执 ok。
#[test]
fn n2_cr1_page_mouse_click() {
    let home = tempfile::tempdir().unwrap();
    let (_d, _c, mut ws, cdp, id) = fake_cdp_setup(home.path(), Box::new(|_, _| None));
    let r = ok(
        &mut ws,
        "page.mouse",
        json!({"id": id, "label": "p1", "op": "click", "args": {"x": 5, "y": 6}}),
    );
    assert_eq!(r["ok"], true, "{r}");
    let kinds: Vec<_> = sent(&cdp, "Input.dispatchMouseEvent")
        .into_iter()
        .map(|c| {
            assert_eq!(c["sessionId"], "S1");
            assert_eq!(
                (c["params"]["x"].as_f64(), c["params"]["y"].as_f64()),
                (Some(5.0), Some(6.0))
            );
            c["params"]["type"].as_str().unwrap().to_owned()
        })
        .collect();
    assert_eq!(kinds, ["mouseMoved", "mousePressed", "mouseReleased"]);
}

/// N2 CR1 page.keyboard：type 发 insertText，press 发 keyDown / keyUp。
#[test]
fn n2_cr1_page_keyboard() {
    let home = tempfile::tempdir().unwrap();
    let (_d, _c, mut ws, cdp, id) = fake_cdp_setup(home.path(), Box::new(|_, _| None));
    ok(
        &mut ws,
        "page.keyboard",
        json!({"id": id, "label": "p1", "op": "type", "args": {"text": "hi"}}),
    );
    let typed = sent(&cdp, "Input.insertText");
    assert_eq!(typed.len(), 1);
    assert_eq!(typed[0]["params"]["text"], "hi");
    ok(
        &mut ws,
        "page.keyboard",
        json!({"id": id, "label": "p1", "op": "press", "args": {"key": "Enter"}}),
    );
    let keys: Vec<_> = sent(&cdp, "Input.dispatchKeyEvent")
        .into_iter()
        .map(|c| (c["params"]["type"].clone(), c["params"]["key"].clone()))
        .collect();
    assert_eq!(
        keys,
        [
            (json!("keyDown"), json!("Enter")),
            (json!("keyUp"), json!("Enter"))
        ]
    );
}

/// N2 CR1 page.fetch：在页面里 fetch（等 Promise），返回状态与正文；`saveAs` 写到绝对路径，相对路径被拒绝。
#[test]
fn n2_cr1_page_fetch() {
    let home = tempfile::tempdir().unwrap();
    let (_d, _c, mut ws, cdp, id) = fake_cdp_setup(
        home.path(),
        Box::new(|m, req| {
            let expr = req["params"]["expression"].as_str().unwrap_or("");
            (m == "Runtime.evaluate" && expr.contains("fetch(")).then(|| {
                let binary = expr.trim_end().ends_with("true)");
                let value = if binary {
                    json!({"ok": true, "status": 200, "headers": {"x": "1"}, "base64": "aGk="})
                } else {
                    json!({"ok": true, "status": 200, "headers": {"x": "1"}, "body": "hello"})
                };
                json!({"result": {"type": "object", "value": value}})
            })
        }),
    );
    let r = ok(
        &mut ws,
        "page.fetch",
        json!({"id": id, "label": "p1", "url": "/data", "options": {"method": "GET"}}),
    );
    assert_eq!(
        r,
        json!({"ok": true, "status": 200, "headers": {"x": "1"}, "body": "hello"})
    );
    let evals = sent(&cdp, "Runtime.evaluate");
    let fetch = evals
        .iter()
        .find(|c| {
            c["params"]["expression"]
                .as_str()
                .unwrap()
                .contains("fetch(")
        })
        .unwrap();
    assert_eq!(fetch["params"]["awaitPromise"], true);
    assert_eq!(fetch["params"]["returnByValue"], true);

    let out = home.path().join("saved.bin");
    let r = ok(
        &mut ws,
        "page.fetch",
        json!({"id": id, "label": "p1", "url": "/data", "saveAs": out.to_str().unwrap()}),
    );
    assert_eq!(r["path"], out.to_str().unwrap());
    assert!(r.get("body").is_none(), "{r}");
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "hi");

    let e = err(
        &mut ws,
        "page.fetch",
        json!({"id": id, "label": "p1", "url": "/data", "saveAs": "rel.bin"}),
    );
    assert_eq!(e["code"], "E_INVALID_ARGUMENT");
}

/// N2 CR2 生命周期命令与页面命令同一队列：`page.goto` 等加载时，另一连接的 `taskSpace.finish`
/// 要等它结束才执行；goto 以自己的超时结束，而不是被关掉的视图打断。
#[test]
fn n2_cr2_finish_waits_for_running_goto() {
    let home = tempfile::tempdir().unwrap();
    // 假 CDP 端对 Page.navigate 返回 loaderId，但不会再有生命周期事件：goto 一直等到超时
    let (_d, _c, mut a, cdp, id) = fake_cdp_setup(home.path(), Box::new(|_, _| None));
    let mut b = gateway_client(home.path());

    let req = json!({"jsonrpc": "2.0", "id": 9, "method": "page.goto",
        "params": {"id": id, "label": "p1", "url": "https://example.com/", "timeoutMs": 500}});
    a.send(tungstenite::Message::Text(req.to_string().into()))
        .unwrap();
    // 假 CDP 端收到 Page.navigate 时 goto 一定已拿到队列，此后它还要等到自己的 500 ms 超时
    let navigated_at = loop {
        let (at, cmd) = cdp.commands.recv_timeout(Duration::from_secs(5)).unwrap();
        if cmd["method"] == "Page.navigate" {
            break at;
        }
    };
    let finished = rpc(&mut b, 1, "taskSpace.finish", json!({"id": id}));
    let waited = navigated_at.elapsed();
    assert!(finished.get("error").is_none(), "{finished}");
    assert!(
        waited >= Duration::from_millis(300),
        "finish 在 Page.navigate 之后只等了 {waited:?}，没有排在 goto 之后"
    );
    let goto = rpc_response(&mut a, 9);
    assert_eq!(goto["error"]["data"]["code"], "E_TIMEOUT", "{goto}");
}

/// 读到 id 为 `id` 的响应（请求已经发出）。
fn rpc_response(ws: &mut common::Ws, id: u64) -> serde_json::Value {
    loop {
        if let tungstenite::Message::Text(text) = ws.read().expect("应当收到响应") {
            let v: serde_json::Value = serde_json::from_str(&text).unwrap();
            if v["id"] == id {
                return v;
            }
        }
    }
}

/// N2 V6 create / resume 的结果带 TaskSpace 名称（SDK 的 `TaskSpace.name`）。
#[test]
fn n2_v6_task_space_info_has_name() {
    let home = tempfile::tempdir().unwrap();
    let (_d, _c, mut ws, _personal, _a) = setup_with_tab(home.path());
    let created = ok(&mut ws, "taskSpace.create", json!({"name": "demo"}));
    assert_eq!(created["name"], "demo", "{created}");
    let resumed = ok(&mut ws, "taskSpace.resume", json!({"id": created["id"]}));
    assert_eq!(resumed["name"], "demo", "{resumed}");
}

/// 读到下一条方法为 `method` 的 JSON-RPC 通知（跳过其他消息），`timeout` 内没有即失败。
fn notification(ws: &mut common::Ws, method: &str, timeout: Duration) -> serde_json::Value {
    if let tungstenite::stream::MaybeTlsStream::Plain(s) = ws.get_mut() {
        s.set_read_timeout(Some(timeout)).unwrap();
    }
    loop {
        let msg = ws
            .read()
            .unwrap_or_else(|e| panic!("{timeout:?} 内没有收到 {method}：{e}"));
        if let tungstenite::Message::Text(text) = msg {
            let v: serde_json::Value = serde_json::from_str(&text).unwrap();
            if v.get("id").is_none() && v["method"] == method {
                return v["params"].clone();
            }
        }
    }
}

/// 读到下一条 `reason` 相符的 `taskSpace.ownershipChanged` 通知（同一连接自己调用 handOff 时，
/// 那条通知可能在响应之前到达、已被 `rpc` 跳过，所以按 reason 找）。
fn ownership_changed(ws: &mut common::Ws, reason: &str) -> serde_json::Value {
    loop {
        let p = notification(ws, "taskSpace.ownershipChanged", Duration::from_secs(2));
        if p["reason"] == reason {
            return p;
        }
    }
}

/// 快照里某个 TaskSpace 的视图。
fn snapshot_task_space(snap: &serde_json::Value, id: i64) -> Option<serde_json::Value> {
    snap["taskSpaces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == id)
        .cloned()
}

/// N3 R1.S1 handOff：响应 {}，另一个 resume 过的连接收到 ownershipChanged，快照里 ownership 为 user。
#[test]
fn n3_r1_s1_hand_off_notifies_and_snapshots() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, mut a, _personal, _tab) = setup_with_tab(home.path());
    let id = ok(&mut a, "taskSpace.create", json!({"name": "login"}))["id"]
        .as_i64()
        .unwrap();
    let mut b = gateway_client(home.path());
    ok(&mut b, "taskSpace.resume", json!({"id": id}));

    assert_eq!(
        ok(&mut a, "taskSpace.handOff", json!({"id": id})),
        json!({})
    );
    let params = notification(&mut b, "taskSpace.ownershipChanged", Duration::from_secs(2));
    assert_eq!(
        params,
        json!({"id": id, "ownership": "user", "reason": "handOff"})
    );
    snapshot_where(&mut c, |s| {
        snapshot_task_space(s, id).is_some_and(|t| t["ownership"] == "user")
    });
}

/// N3 R2.S1 handOff 之后 page.click 回 E_USER_CONTROL，不发 CDP 命令；回执日志记 ok=false、E_USER_CONTROL。
#[test]
fn n3_r2_s1_click_rejected_while_user_controls() {
    let home = tempfile::tempdir().unwrap();
    let (d, _c, mut ws, cdp, id) = fake_cdp_setup(home.path(), Box::new(|_, _| None));
    ok(&mut ws, "taskSpace.handOff", json!({"id": id}));
    // 转为用户控制时 alterd 会隐藏页面上的叠层（这是 alterd 自己的动作，不是 agent 的命令）：等它发完
    cdp.expect("Runtime.evaluate", Duration::from_secs(2));
    std::thread::sleep(Duration::from_millis(100));
    cdp.drain();
    let e = err(
        &mut ws,
        "page.click",
        json!({"id": id, "label": "p1", "selector": "body"}),
    );
    assert_eq!(e["code"], "E_USER_CONTROL", "{e}");
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(
        cdp.drain(),
        Vec::<serde_json::Value>::new(),
        "用户控制期间不应发 CDP 命令"
    );

    let stderr = d.stop_and_stderr();
    let receipt = stderr
        .lines()
        .filter(|l| l.contains("\"receipt\""))
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap()["fields"].clone())
        .find(|f| f["method"] == "page.click")
        .unwrap_or_else(|| panic!("没有 page.click 的回执行：{stderr}"));
    assert_eq!(receipt["ok"], false);
    assert_eq!(receipt["error_code"], "E_USER_CONTROL");
}

/// N3 R2.S2 handOff 之后不能新开页面。
#[test]
fn n3_r2_s2_new_page_rejected_while_user_controls() {
    let home = tempfile::tempdir().unwrap();
    let (_d, _c, mut ws, _personal, _tab) = setup_with_tab(home.path());
    let id = ok(&mut ws, "taskSpace.create", json!({"name": "login"}))["id"].clone();
    ok(&mut ws, "taskSpace.handOff", json!({"id": id}));
    let e = err(&mut ws, "taskSpace.newPage", json!({"id": id}));
    assert_eq!(e["code"], "E_USER_CONTROL", "{e}");
}

/// N3 R3.S1 ownershipReturned：订阅者收到 reason=returned，快照里 ownership 回到 agent。
#[test]
fn n3_r3_s1_ownership_returned() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, mut ws, _personal, _tab) = setup_with_tab(home.path());
    let id = ok(&mut ws, "taskSpace.create", json!({"name": "login"}))["id"]
        .as_i64()
        .unwrap();
    ok(&mut ws, "taskSpace.handOff", json!({"id": id}));

    c.send(json!({"type": "shell.event.ownershipReturned", "taskSpaceId": id}));
    let params = ownership_changed(&mut ws, "returned");
    assert_eq!(
        params,
        json!({"id": id, "ownership": "agent", "reason": "returned"})
    );
    snapshot_where(&mut c, |s| {
        snapshot_task_space(s, id).is_some_and(|t| t["ownership"] == "agent")
    });
}

/// N3 R4.S1 已交还后 takeOver：托管所在 Space 的激活标签 A 为 p2，返回 {ownership: agent, userPage: p2}。
#[test]
fn n3_r4_s1_take_over_after_return() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, mut ws, _personal, a) = setup_with_tab(home.path());
    let id = ok(&mut ws, "taskSpace.create", json!({"name": "login"}))["id"]
        .as_i64()
        .unwrap();
    ok(&mut ws, "taskSpace.handOff", json!({"id": id}));
    c.send(json!({"type": "shell.event.ownershipReturned", "taskSpaceId": id}));
    ownership_changed(&mut ws, "returned");

    let r = ok(&mut ws, "taskSpace.takeOver", json!({"id": id}));
    assert_eq!(r, json!({"ownership": "agent", "userPage": "p2"}));
    let tabs = ok(&mut ws, "taskSpace.tabs", json!({"id": id}));
    let row = tabs["tabs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["tabId"] == a.as_str())
        .unwrap()
        .clone();
    assert_eq!(row["label"], "p2", "{tabs}");
}

/// N3 R4.S2 用户尚未交还时 takeOver 回 E_USER_CONTROL。
#[test]
fn n3_r4_s2_take_over_while_user_controls() {
    let home = tempfile::tempdir().unwrap();
    let (_d, _c, mut ws, _personal, _tab) = setup_with_tab(home.path());
    let id = ok(&mut ws, "taskSpace.create", json!({"name": "login"}))["id"].clone();
    ok(&mut ws, "taskSpace.handOff", json!({"id": id}));
    let e = err(&mut ws, "taskSpace.takeOver", json!({"id": id}));
    assert_eq!(e["code"], "E_USER_CONTROL", "{e}");
}

/// N3 R5.S1 userTookControl 中断正在等待的 waitFor：1 秒内以 E_USER_CONTROL 返回，订阅者收到 reason=userTookControl。
#[test]
fn n3_r5_s1_user_took_control_cancels_running_command() {
    let home = tempfile::tempdir().unwrap();
    // 假 CDP 端的求值结果没有 value：选择器永远查不到
    let (_d, mut c, mut a, _cdp, id) = fake_cdp_setup(home.path(), Box::new(|_, _| None));
    let mut b = gateway_client(home.path());
    ok(&mut b, "taskSpace.resume", json!({"id": id}));

    let req = json!({"jsonrpc": "2.0", "id": 9, "method": "page.waitFor",
        "params": {"id": id, "label": "p1", "kind": "selector", "arg": "#never", "timeoutMs": 10000}});
    a.send(tungstenite::Message::Text(req.to_string().into()))
        .unwrap();
    std::thread::sleep(Duration::from_secs(1));
    let sent = std::time::Instant::now();
    c.send(json!({"type": "shell.event.userTookControl", "taskSpaceId": id}));
    let resp = rpc_response(&mut a, 9);
    assert!(
        sent.elapsed() < Duration::from_secs(1),
        "waitFor 在 {:?} 后才返回",
        sent.elapsed()
    );
    assert_eq!(resp["error"]["data"]["code"], "E_USER_CONTROL", "{resp}");
    let params = notification(&mut b, "taskSpace.ownershipChanged", Duration::from_secs(2));
    assert_eq!(
        params,
        json!({"id": id, "ownership": "user", "reason": "userTookControl"})
    );
}

/// N3 R6.S1 / R6.S2 handOff 超过阈值标记 inactive（ownership 仍为 user）；交还后回到 active / agent。
#[test]
fn n3_r6_inactive_after_timeout_and_back() {
    let home = tempfile::tempdir().unwrap();
    let _d = Daemon::start_with_env(home.path(), &[("ALTER_HANDOFF_TIMEOUT_MS", "300")]);
    let mut c = Client::connect(home.path());
    c.recv();
    let mut ws = gateway_client(home.path());
    let id = ok(&mut ws, "taskSpace.create", json!({"name": "login"}))["id"]
        .as_i64()
        .unwrap();
    ok(&mut ws, "taskSpace.handOff", json!({"id": id}));
    std::thread::sleep(Duration::from_secs(1));
    let item = |ws: &mut common::Ws| {
        ok(ws, "taskSpace.list", json!({}))["taskSpaces"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["id"] == id)
            .unwrap()
            .clone()
    };
    let t = item(&mut ws);
    assert_eq!(
        (t["status"].clone(), t["ownership"].clone()),
        (json!("inactive"), json!("user")),
        "R6.S1：{t}"
    );

    c.send(json!({"type": "shell.event.ownershipReturned", "taskSpaceId": id}));
    ownership_changed(&mut ws, "returned");
    let t = item(&mut ws);
    assert_eq!(
        (t["status"].clone(), t["ownership"].clone()),
        (json!("active"), json!("agent")),
        "R6.S2：{t}"
    );
}

/// 让 handOff 引起的「隐藏叠层」CDP 命令发完，再清空记录。
fn settle_after_hand_off(cdp: &common::fake_cdp::FakeCdp) {
    cdp.expect("Runtime.evaluate", Duration::from_secs(2));
    std::thread::sleep(Duration::from_millis(100));
    cdp.drain();
}

/// N3 CR1 用户控制期间 taskSpace.cdp 回 E_USER_CONTROL，不发出 CDP 命令。
#[test]
fn n3_cr1_task_space_cdp_rejected_while_user_controls() {
    let home = tempfile::tempdir().unwrap();
    let (_d, _c, mut ws, cdp, id) = fake_cdp_setup(home.path(), Box::new(|_, _| None));
    ok(&mut ws, "taskSpace.handOff", json!({"id": id}));
    settle_after_hand_off(&cdp);
    let e = err(
        &mut ws,
        "taskSpace.cdp",
        json!({"id": id, "method": "Target.closeTarget", "params": {"targetId": "T1"}}),
    );
    assert_eq!(e["code"], "E_USER_CONTROL", "{e}");
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(cdp.drain(), Vec::<serde_json::Value>::new());
}

/// 经 CDP 转发端点发一个带 token 的 HTTP 请求，返回连接（请求已发出）。
fn forward_request(home: &std::path::Path) -> TcpStream {
    use std::io::Write;
    let gw = gateway_file(home);
    let mut s = TcpStream::connect(("127.0.0.1", gw["cdpPort"].as_u64().unwrap() as u16)).unwrap();
    write!(
        s,
        "GET /json/version HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {}\r\n\r\n",
        gw["token"].as_str().unwrap()
    )
    .unwrap();
    s
}

/// 在 `timeout` 内读到的内容：None 表示超时仍没有数据也没有关闭（连接还在转发中）。
fn read_within(s: &mut TcpStream, timeout: Duration) -> Option<String> {
    use std::io::Read;
    s.set_read_timeout(Some(timeout)).unwrap();
    let mut buf = [0u8; 256];
    match s.read(&mut buf) {
        Ok(n) => Some(String::from_utf8_lossy(&buf[..n]).into_owned()),
        Err(e)
            if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut =>
        {
            None
        }
        Err(_) => Some(String::new()),
    }
}

/// N3 CR2 用户控制期间 CDP 转发端点拒绝新连接（423），转为用户控制时断开已有连接；交还后恢复转发。
#[test]
fn n3_cr2_forward_respects_user_control() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, mut ws, _cdp, id) = fake_cdp_setup(home.path(), Box::new(|_, _| None));
    // 假 CDP 端只应答 alterd 自己的连接：转发过去的请求得不到回应，连接保持
    let mut before = forward_request(home.path());
    assert_eq!(
        read_within(&mut before, Duration::from_millis(300)),
        None,
        "接管前应当在转发中"
    );

    ok(&mut ws, "taskSpace.handOff", json!({"id": id}));
    assert_eq!(
        read_within(&mut before, Duration::from_secs(1)).as_deref(),
        Some(""),
        "转为用户控制后已有的转发连接应当被断开"
    );
    let mut during = forward_request(home.path());
    let resp = read_within(&mut during, Duration::from_secs(1)).unwrap_or_default();
    assert!(resp.starts_with("HTTP/1.1 423"), "用户控制期间：{resp:?}");

    c.send(json!({"type": "shell.event.ownershipReturned", "taskSpaceId": id}));
    ownership_changed(&mut ws, "returned");
    let mut after = forward_request(home.path());
    assert_eq!(
        read_within(&mut after, Duration::from_millis(300)),
        None,
        "交还后应当恢复转发"
    );
}

/// N3 CR3 用户控制期间 release / finish 回 E_USER_CONTROL，页面与状态不变。
#[test]
fn n3_cr3_release_and_finish_rejected_while_user_controls() {
    let home = tempfile::tempdir().unwrap();
    let (_d, _c, mut ws, _personal, _tab) = setup_with_tab(home.path());
    let id = ok(&mut ws, "taskSpace.create", json!({"name": "login"}))["id"].clone();
    ok(&mut ws, "taskSpace.handOff", json!({"id": id}));
    let e = err(
        &mut ws,
        "taskSpace.release",
        json!({"id": id, "label": "p1"}),
    );
    assert_eq!(e["code"], "E_USER_CONTROL", "{e}");
    let e = err(&mut ws, "taskSpace.finish", json!({"id": id, "keep": []}));
    assert_eq!(e["code"], "E_USER_CONTROL", "{e}");
    assert_eq!(
        ok(&mut ws, "taskSpace.resume", json!({"id": id}))["pages"],
        json!(["p1"])
    );
    let list = ok(&mut ws, "taskSpace.list", json!({}));
    let t = list["taskSpaces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == id)
        .unwrap()
        .clone();
    assert_eq!(t["status"], "active", "{t}");
}

/// N3 CR4 激活标签被另一个 TaskSpace 托管时，takeOver 不抢它：userPage 为 null。
#[test]
fn n3_cr4_take_over_when_active_tab_owned_by_other() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, mut ws, _personal, a) = setup_with_tab(home.path());
    let other = ok(&mut ws, "taskSpace.create", json!({"name": "other"}))["id"].clone();
    assert_eq!(
        ok(&mut ws, "taskSpace.adopt", json!({"id": other, "tabId": a}))["label"],
        "p2"
    );
    let id = ok(&mut ws, "taskSpace.create", json!({"name": "login"}))["id"]
        .as_i64()
        .unwrap();
    ok(&mut ws, "taskSpace.handOff", json!({"id": id}));
    c.send(json!({"type": "shell.event.ownershipReturned", "taskSpaceId": id}));
    ownership_changed(&mut ws, "returned");
    let r = ok(&mut ws, "taskSpace.takeOver", json!({"id": id}));
    assert_eq!(r, json!({"ownership": "agent", "userPage": null}));
}

/// N3 CR5 有任何 TaskSpace 处于用户控制时，原始 CDP 透传（taskSpace.cdp、page.cdp）都被拒绝：
/// 不能借另一个仍归 agent 的 TaskSpace 发浏览器级命令。全部交还后恢复。
#[test]
fn n3_cr5_raw_cdp_rejected_while_any_user_controlled() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, mut ws, cdp, a) = fake_cdp_setup(home.path(), Box::new(|_, _| None));
    let b = ok(&mut ws, "taskSpace.create", json!({"name": "login"}))["id"]
        .as_i64()
        .unwrap();
    ok(&mut ws, "taskSpace.handOff", json!({"id": b}));
    std::thread::sleep(Duration::from_millis(200));
    cdp.drain();

    let e = err(
        &mut ws,
        "taskSpace.cdp",
        json!({"id": a, "method": "Target.closeTarget", "params": {"targetId": "T2"}}),
    );
    assert_eq!(e["code"], "E_USER_CONTROL", "taskSpace.cdp：{e}");
    let e = err(
        &mut ws,
        "page.cdp",
        json!({"id": a, "label": "p1", "method": "Target.closeTarget", "params": {"targetId": "T2"}}),
    );
    assert_eq!(e["code"], "E_USER_CONTROL", "page.cdp：{e}");
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(cdp.drain(), Vec::<serde_json::Value>::new());

    c.send(json!({"type": "shell.event.ownershipReturned", "taskSpaceId": b}));
    ownership_changed(&mut ws, "returned");
    ok(
        &mut ws,
        "taskSpace.cdp",
        json!({"id": a, "method": "Browser.getVersion", "params": {}}),
    );
}

/// N3 CR7 原始 CDP 透传在全局检查通过后、发送之前发生了接管（另一个 TaskSpace），也回 E_USER_CONTROL：
/// A 的 p2 还没上报 targetId，`page.cdp` 在等页面会话；此时 B handOff，再替 p2 上报 targetId。
#[test]
fn n3_cr7_raw_cdp_cancelled_by_takeover_while_waiting() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, mut ws, cdp, a) = fake_cdp_setup(home.path(), Box::new(|_, _| None));
    assert_eq!(
        ok(&mut ws, "taskSpace.newPage", json!({"id": a}))["label"],
        "p2"
    );
    let snap = snapshot_where(&mut c, |s| {
        s["tabs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["label"] == "p2")
    });
    let p2_tab = snap["tabs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["label"] == "p2")
        .unwrap()["id"]
        .clone();

    let mut other = gateway_client(home.path());
    let req = json!({"jsonrpc": "2.0", "id": 9, "method": "page.cdp",
        "params": {"id": a, "label": "p2", "method": "Target.closeTarget", "params": {"targetId": "T9"}}});
    ws.send(tungstenite::Message::Text(req.to_string().into()))
        .unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let b = ok(&mut other, "taskSpace.create", json!({"name": "login"}))["id"].clone();
    ok(&mut other, "taskSpace.handOff", json!({"id": b}));
    c.send(json!({"type": "shell.event.tabUpdated", "tabId": p2_tab, "targetId": "T2"}));

    let resp = rpc_response(&mut ws, 9);
    assert_eq!(resp["error"]["data"]["code"], "E_USER_CONTROL", "{resp}");
    std::thread::sleep(Duration::from_millis(200));
    let closes: Vec<_> = cdp
        .drain()
        .into_iter()
        .filter(|c| c["method"] == "Target.closeTarget")
        .collect();
    assert!(closes.is_empty(), "接管后不应发出透传的命令：{closes:?}");
}

/// N4 CR3 agent 经 `page.close` 关闭分屏里的标签，另一个标签的 splitGroup 也被清空。
#[test]
fn n4_cr3_page_close_dissolves_split() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, mut ws, _personal, a) = setup_with_tab(home.path());
    let id = ok(&mut ws, "taskSpace.create", json!({"name": "demo"}))["id"].clone();
    let snap = snapshot_where(&mut c, |s| {
        s["tabs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["label"] == "p1")
    });
    let g = snap["tabs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["label"] == "p1")
        .unwrap()["id"]
        .clone();
    c.send(json!({"type": "shell.event.splitRequested", "tabIds": [a, g]}));
    snapshot_where(&mut c, |s| {
        s["tabs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == a.as_str() && !t["splitGroup"].is_null())
    });
    ok(&mut ws, "page.close", json!({"id": id, "label": "p1"}));
    snapshot_where(&mut c, |s| {
        let tabs = s["tabs"].as_array().unwrap();
        !tabs.iter().any(|t| t["id"] == g)
            && tabs
                .iter()
                .any(|t| t["id"] == a.as_str() && t["splitGroup"].is_null())
    });
}
