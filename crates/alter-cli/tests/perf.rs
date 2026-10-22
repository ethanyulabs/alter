//! 父级 SC1：`alter --version` 冷启动耗时。`#[ignore]`，release 下手动或在验证时跑：
//! `cargo test -p alter-cli --release --test perf -- --ignored --nocapture`

use std::process::Command;
use std::time::{Duration, Instant};

#[test]
#[ignore]
fn sc1_version_cold_start_median_under_20ms() {
    let alter = env!("CARGO_BIN_EXE_alter");
    let mut times: Vec<Duration> = (0..100)
        .map(|_| {
            // 计时包含 fork / exec / 等待退出，只会高估
            let start = Instant::now();
            let out = Command::new(alter).arg("--version").output().unwrap();
            let elapsed = start.elapsed();
            assert!(out.status.success());
            elapsed
        })
        .collect();
    times.sort();
    let median = times[49];
    println!(
        "SC1: alter --version 100 次 中位数 {:.2} ms，p95 {:.2} ms，最大 {:.2} ms",
        median.as_secs_f64() * 1e3,
        times[94].as_secs_f64() * 1e3,
        times[99].as_secs_f64() * 1e3,
    );
    assert!(median < Duration::from_millis(20), "中位数 {median:?}");
}
