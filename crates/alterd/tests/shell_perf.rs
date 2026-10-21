//! SC1 基准：500 个标签时，alterd 从收到 openRequested 到发出新快照的耗时（p95 < 10 ms）。
//! 默认不跑：`cargo test -p alterd --release --test shell_perf -- --ignored --nocapture`。

mod common;

use std::time::{Duration, Instant};

use common::connected;
use serde_json::json;

#[test]
#[ignore = "基准测试，只在显式要求时运行"]
fn sc1_open_requested_round_trip_with_500_tabs() {
    let home = tempfile::tempdir().unwrap();
    let (_d, mut c, space) = connected(home.path());
    let open = |c: &mut common::Client, i: usize| {
        c.send(json!({
            "type": "shell.event.openRequested",
            "spaceId": space,
            "input": format!("https://example.com/{i}"),
        }));
        let snap = c.recv();
        assert_eq!(snap["type"], "shell.stateChanged");
        snap
    };
    for i in 0..500 {
        open(&mut c, i);
    }

    let mut samples: Vec<Duration> = (500..600)
        .map(|i| {
            let start = Instant::now();
            let snap = open(&mut c, i);
            let elapsed = start.elapsed();
            assert_eq!(snap["tabs"].as_array().unwrap().len(), i + 1);
            elapsed
        })
        .collect();
    samples.sort();
    let p50 = samples[49];
    let p95 = samples[94];
    println!("SC1: 100 次 openRequested → 快照（500–600 个标签）p50 {p50:?}，p95 {p95:?}");
    assert!(p95 < Duration::from_millis(10), "p95 {p95:?} 超过 10 ms");
}
