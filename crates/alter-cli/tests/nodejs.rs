//! cli 能力：`alter nodejs` 用 Bun 运行脚本（N2 R21）。需要 PATH 里有 bun；只起 alterd，不需要 Electron。

mod common;

use std::process::Output;

use assert_cmd::Command;

/// PATH 里的 bun。
fn bun_path() -> String {
    let out = std::process::Command::new("sh")
        .args(["-c", "command -v bun"])
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

fn nodejs(home: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_alter"));
    cmd.arg("nodejs")
        .env("ALTER_HOME", home)
        .env("ALTER_BUN_PATH", bun_path())
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

/// N2 R21.S1 -e 运行脚本：输出 p1，退出码 0。
#[test]
fn n2_r21_s1_runs_inline_script() {
    let home = tempfile::tempdir().unwrap();
    let _d = common::Daemon::start(home.path());
    let out = nodejs(home.path())
        .args([
            "-e",
            r#"const t = await taskSpace("demo"); console.log(t.page("p1").label)"#,
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", show(&out));
    assert_eq!(text(&out.stdout), "p1\n", "{}", show(&out));
}

/// N2 R21.S2 从 stdin 读脚本：输出 true，退出码 0。
#[test]
fn n2_r21_s2_runs_script_from_stdin() {
    let home = tempfile::tempdir().unwrap();
    let _d = common::Daemon::start(home.path());
    let out = nodejs(home.path())
        .write_stdin("console.log((await listTaskSpaces()).length >= 0)\n")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", show(&out));
    assert_eq!(text(&out.stdout), "true\n", "{}", show(&out));
}

/// N2 R21.S3 脚本抛出异常：退出码 1，stderr 含 boom。
#[test]
fn n2_r21_s3_script_error_exits_1() {
    let home = tempfile::tempdir().unwrap();
    let _d = common::Daemon::start(home.path());
    let out = nodejs(home.path())
        .args(["-e", r#"throw new Error("boom")"#])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{}", show(&out));
    assert!(text(&out.stderr).contains("boom"), "{}", show(&out));
}

/// N2 R21.S4 alterd 未运行：没有 gateway.json 时退出码 2，stderr 提示 alter.app。
#[test]
fn n2_r21_s4_no_gateway_exits_2() {
    let home = tempfile::tempdir().unwrap();
    let out = nodejs(home.path()).args(["-e", "1"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2), "{}", show(&out));
    assert!(text(&out.stderr).contains("alter.app"), "{}", show(&out));
}

/// N2 V7 gateway.json 在、但它的端口没人监听（alterd 已异常退出）：不运行脚本，退出码 2。
#[test]
fn n2_v7_unreachable_gateway_exits_2() {
    let home = tempfile::tempdir().unwrap();
    // 借一个刚释放的端口：不再有人监听
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    std::fs::write(
        home.path().join("gateway.json"),
        format!(
            r#"{{"port":{port},"token":"{}","cdpPort":1,"version":"0.1.0"}}"#,
            "t".repeat(64)
        ),
    )
    .unwrap();
    let out = nodejs(home.path())
        .args(["-e", "console.log('ran')"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{}", show(&out));
    assert!(!text(&out.stdout).contains("ran"), "{}", show(&out));
    assert!(text(&out.stderr).contains("alter.app"), "{}", show(&out));
}
