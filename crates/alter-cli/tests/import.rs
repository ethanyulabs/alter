//! cli 能力：`alter import` 经 Gateway 运行导入（N5 R9）。只起 alterd，不需要 Electron。

mod common;
#[path = "../../alterd/tests/common/fixture_profile.rs"]
#[allow(dead_code)]
mod fixture_profile;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::Output;

use assert_cmd::Command;
use serde_json::{Value, json};

fn alter(home: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_alter"));
    cmd.env("ALTER_HOME", home)
        .timeout(std::time::Duration::from_secs(30));
    cmd
}

fn text(out: &[u8]) -> String {
    String::from_utf8_lossy(out).into_owned()
}

fn show(o: &Output) -> String {
    format!(
        "code={:?} stdout={:?} stderr={:?}",
        o.status.code(),
        text(&o.stdout),
        text(&o.stderr)
    )
}

/// 经 shell.sock 建 Space `Work`，返回它的 id。
fn create_work(home: &Path) -> String {
    let stream = UnixStream::connect(home.join("shell.sock")).unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(2)))
        .unwrap();
    let mut writer = stream.try_clone().unwrap();
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let msg = json!({"type": "shell.event.spaceCreated", "name": "Work", "color": "#f97316"});
    writeln!(writer, "{msg}").unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let snap: Value = serde_json::from_str(&line).unwrap();
    snap["spaces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "Work")
        .unwrap_or_else(|| panic!("快照里没有 Work：{snap}"))["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// N5 R9.S1 导入书签与历史：stdout 一行计数，退出码 0。
#[test]
fn n5_r9_s1_import_prints_counts() {
    let home = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    fixture_profile::standard(profile.path());
    let _d = common::Daemon::start(home.path());
    let work = create_work(home.path());

    let out = alter(home.path())
        .args(["import", "--from", "chrome", "--profile"])
        .arg(profile.path())
        .args(["--space", &work, "--items", "bookmarks,history"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", show(&out));
    assert_eq!(
        text(&out.stdout),
        "imported bookmarks=3 history=2 cookies=0 skipped=0\n",
        "{}",
        show(&out)
    );
}

/// N5 R9.S2 来源不存在：退出码 1，stderr 含 E_IMPORT_SOURCE_NOT_FOUND。
#[test]
fn n5_r9_s2_missing_source_exits_1() {
    let home = tempfile::tempdir().unwrap();
    let _d = common::Daemon::start(home.path());
    let work = create_work(home.path());

    let out = alter(home.path())
        .args([
            "import",
            "--from",
            "chrome",
            "--profile",
            "/nonexistent",
            "--space",
            &work,
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{}", show(&out));
    assert!(
        text(&out.stderr).contains("E_IMPORT_SOURCE_NOT_FOUND"),
        "{}",
        show(&out)
    );
}
