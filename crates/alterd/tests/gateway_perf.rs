//! 父级 SC2：Gateway 单动作内部开销。`#[ignore]`：
//! `cargo test -p alterd --release --test gateway_perf -- --ignored --nocapture`
//!
//! 用一个假 CDP 端（本地 WebSocket 服务，立即应答）代替 Chromium，测「发出 `page.click` 请求」到
//! 「假 CDP 端收到第一个 `Input.dispatchMouseEvent`」的时间。两个时间点都在本测试进程里取，
//! 计入了 WebSocket 收发与途中几次 CDP 往返，只会高估 alterd 的内部开销。

mod common;

use std::time::{Duration, Instant};

use common::fake_cdp::{FakeCdp, agent_page};
use common::{Client, Daemon, gateway_client, rpc, wait_until};
use serde_json::json;

#[test]
#[ignore]
fn sc2_gateway_click_overhead_p95_under_5ms() {
    let home = tempfile::tempdir().unwrap();
    // 1000 次动作各写一行回执日志：stderr 丢弃（没人读的管道写满后 alterd 会卡在写日志上）
    let stderr = if std::env::var_os("SC2_DEBUG").is_some() {
        std::process::Stdio::inherit()
    } else {
        std::process::Stdio::null()
    };
    let _d = Daemon::spawn_with_stderr(home.path(), stderr);
    assert!(wait_until(Duration::from_secs(2), || home
        .path()
        .join("alterd.json")
        .exists()));
    let mut shell = Client::connect(home.path());
    shell.recv();

    let cdp = FakeCdp::start(home.path(), &mut shell);
    let mut ws = gateway_client(home.path());
    let id = agent_page(&mut shell, &mut ws);

    let click = |ws: &mut common::Ws, n: u64| {
        let start = Instant::now();
        let resp = rpc(
            ws,
            n,
            "page.click",
            json!({"id": id, "label": "p1", "selector": "text=Go"}),
        );
        assert!(resp.get("error").is_none(), "{resp}");
        loop {
            let (at, cmd) = cdp.commands.recv_timeout(Duration::from_secs(5)).unwrap();
            if cmd["method"] == "Input.dispatchMouseEvent" && cmd["params"]["type"] == "mouseMoved"
            {
                break at.duration_since(start);
            }
        }
    };
    // 预热：建立 CDP 会话、设置下载行为等一次性开销不计入
    for n in 0..20 {
        click(&mut ws, 100 + n);
    }
    let mut times: Vec<Duration> = (0..1000).map(|n| click(&mut ws, 1000 + n)).collect();
    times.sort();
    let p95 = times[949];
    println!(
        "SC2: page.click 1000 次 p50 {:.3} ms，p95 {:.3} ms，最大 {:.3} ms",
        times[499].as_secs_f64() * 1e3,
        p95.as_secs_f64() * 1e3,
        times[999].as_secs_f64() * 1e3,
    );
    assert!(p95 < Duration::from_millis(5), "p95 {p95:?}");
}
