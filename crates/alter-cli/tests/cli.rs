//! cli 能力的场景测试：以真实的 `alter` 二进制驱动，数据目录用临时目录隔离。

mod common;

use assert_cmd::Command;

fn alter() -> Command {
    Command::new(env!("CARGO_BIN_EXE_alter"))
}

/// R4.S1 打印版本：stdout 恰好一行 `alter 0.1.0`，退出码 0。
#[test]
fn r4_s1_prints_version() {
    alter()
        .arg("--version")
        .assert()
        .code(0)
        .stdout("alter 0.1.0\n");
}

/// 在数据目录下写一个健康文件，模拟 alterd 写出的内容。
fn write_health(home: &std::path::Path, pid: u32) {
    let health = serde_json::json!({
        "version": "0.1.0",
        "pid": pid,
        "startedAt": 1_760_918_400_000_i64,
        "dbPath": home.join("alter.db"),
    });
    std::fs::write(home.join("alterd.json"), health.to_string()).unwrap();
}

/// 解析 stdout：必须恰好一行 JSON。
fn json_line(stdout: &[u8]) -> serde_json::Value {
    let text = std::str::from_utf8(stdout).unwrap();
    assert_eq!(text.lines().count(), 1, "应当恰好一行：{text:?}");
    serde_json::from_str(text).unwrap()
}

/// R5.S1 守护进程运行中：文本含 running / pid / 0.1.0，退出 0；--json 输出完整状态。
#[test]
fn r5_s1_status_running() {
    let home = tempfile::tempdir().unwrap();
    // 测试进程自己一定存活，用它的 pid 代表正在运行的 alterd
    let pid = std::process::id();
    write_health(home.path(), pid);

    alter()
        .arg("status")
        .env("ALTER_HOME", home.path())
        .assert()
        .code(0)
        .stdout(format!("alterd running (pid {pid}, version 0.1.0)\n"));

    let out = alter()
        .args(["status", "--json"])
        .env("ALTER_HOME", home.path())
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    assert_eq!(
        json_line(&out),
        serde_json::json!({
            "running": true,
            "pid": pid,
            "version": "0.1.0",
            "dbPath": home.path().join("alter.db"),
        })
    );
}

/// R5.S2 没有健康文件：stdout `not running`，退出 1；--json 为 `{"running": false}`。
#[test]
fn r5_s2_status_without_health_file() {
    let home = tempfile::tempdir().unwrap();

    alter()
        .arg("status")
        .env("ALTER_HOME", home.path())
        .assert()
        .code(1)
        .stdout("not running\n");

    let out = alter()
        .args(["status", "--json"])
        .env("ALTER_HOME", home.path())
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    assert_eq!(json_line(&out), serde_json::json!({ "running": false }));
}

/// R5.S3 健康文件残留：pid 999999 不存在，stdout `not running (stale)`，退出 1；
/// --json 为 `{"running": false, "stale": true}`。
#[test]
fn r5_s3_status_with_stale_health_file() {
    let home = tempfile::tempdir().unwrap();
    write_health(home.path(), 999_999);

    alter()
        .arg("status")
        .env("ALTER_HOME", home.path())
        .assert()
        .code(1)
        .stdout("not running (stale)\n");

    let out = alter()
        .args(["status", "--json"])
        .env("ALTER_HOME", home.path())
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    assert_eq!(
        json_line(&out),
        serde_json::json!({ "running": false, "stale": true })
    );
}

/// N2 R22.S1 alter spaces list：文本一行含 Personal 与 0 tabs；--json 的 spaces[0].name 为 Personal。
#[test]
fn n2_r22_s1_spaces_list_text_and_json() {
    let home = tempfile::tempdir().unwrap();
    let _d = common::Daemon::start(home.path());

    let out = alter()
        .args(["spaces", "list"])
        .env("ALTER_HOME", home.path())
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert_eq!(text.lines().count(), 1, "应当恰好一行：{text:?}");
    assert!(
        text.contains("Personal") && text.contains("0 tabs"),
        "输出：{text:?}"
    );

    let out = alter()
        .args(["spaces", "list", "--json"])
        .env("ALTER_HOME", home.path())
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    assert_eq!(json_line(&out)["spaces"][0]["name"], "Personal");
}
