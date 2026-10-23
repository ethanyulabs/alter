//! 父级 SC3：导入 1000 条 Cookie 的耗时。`#[ignore]`：
//! `cargo test -p alterd --test import_perf -- --ignored --nocapture`
//!
//! 用 debug 构建：钥匙串替身 `ALTER_KEYCHAIN_FAKE` 只在 debug 构建里生效，debug 更慢，结果只会高估。
//! 假 Electron 端收到 `shell.importCookies` 立即回报；计时从 `import.run` 返回到收到 `import.done`，
//! 两个时间点都在本测试进程里取。顺带钉住批大小：1000 条恰好分成 500 + 500 两批。

mod common;

use std::time::{Duration, Instant};

use common::fake_cdp::FakeCdp;
use common::fixture_profile::{self, COOKIE_PASSWORD, CookieRow};
use common::{Client, Daemon, create_space, gateway_client, rpc, space_id, ws_recv_within};
use serde_json::{Value, json};

#[test]
#[ignore]
fn sc3_import_1000_cookies_median_under_2s() {
    let home = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let rows: Vec<CookieRow> = (0..1000)
        .map(|i| {
            let name: &'static str = Box::leak(format!("c{i}").into_boxed_str());
            CookieRow {
                expires_ms: Some(fixture_profile::now_ms() + 30 * fixture_profile::DAY_MS),
                ..CookieRow::new("127.0.0.1", name, "value")
            }
        })
        .collect();
    fixture_profile::write_cookies(profile.path(), 24, &rows);

    let _d = Daemon::start_with_env(home.path(), &[("ALTER_KEYCHAIN_FAKE", COOKIE_PASSWORD)]);
    let mut shell = Client::connect(home.path());
    shell.recv();
    let work = space_id(&create_space(&mut shell, "Work", "#f97316"), "Work");
    let _cdp = FakeCdp::start(home.path(), &mut shell);
    let mut ws = gateway_client(home.path());

    let mut times = Vec::new();
    for run in 0..5 {
        let resp = rpc(
            &mut ws,
            1,
            "import.run",
            json!({"source": "chrome", "profilePath": profile.path(), "spaceId": work, "items": ["cookies"]}),
        );
        let id = resp["result"]["importId"]
            .as_i64()
            .expect("import.run 应当返回 importId");
        let t0 = Instant::now();
        // 假 Electron 端：每收到一批立即全部回报写入成功
        let mut batches = Vec::new();
        while batches.len() < 2 {
            let msg = shell
                .recv_within(Duration::from_secs(5))
                .expect("5 秒内应当收到 shell.importCookies");
            if msg["type"] != "shell.importCookies" {
                continue;
            }
            let n = msg["cookies"].as_array().unwrap().len();
            shell.send(
                json!({"type": "shell.event.cookiesImported", "requestId": msg["requestId"],
                              "imported": n, "rejected": 0}),
            );
            batches.push(n);
        }
        let done: Value = loop {
            let msg = ws_recv_within(&mut ws, Duration::from_secs(10))
                .expect("10 秒内应当收到 import.done");
            if msg["method"] == "import.done" && msg["params"]["importId"] == id {
                break msg["params"].clone();
            }
        };
        let elapsed = t0.elapsed();
        assert_eq!(batches, vec![500, 500], "第 {run} 次的批大小");
        assert_eq!(done["counts"]["cookies"], 1000, "import.done：{done}");
        // 不应再有第三批
        while let Some(msg) = shell.recv_within(Duration::from_millis(100)) {
            assert_ne!(msg["type"], "shell.importCookies", "多出一批：{msg}");
        }
        times.push(elapsed);
    }
    times.sort();
    let median = times[times.len() / 2];
    println!("SC3 导入 1000 条 Cookie：5 次 {times:?}，中位数 {median:?}");
    assert!(
        median < Duration::from_millis(2000),
        "中位数 {median:?} 不小于 2000 ms"
    );
}
