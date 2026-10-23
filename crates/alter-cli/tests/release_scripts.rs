//! release 能力：发布用的脚本（`scripts/version-check.sh --tag`）。在仓库根目录里直接跑脚本。

use std::path::PathBuf;
use std::process::Output;

use assert_cmd::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
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

fn version_check(args: &[&str]) -> Output {
    Command::new("bash")
        .current_dir(repo_root())
        .arg("scripts/version-check.sh")
        .args(args)
        .output()
        .unwrap()
}

/// RC N2 R3.S1 tag 与版本号不一致：退出码 1，stderr 同时含 tag 与版本号。
#[test]
fn rc_n2_r3_s1_tag_mismatch_fails() {
    let v = env!("CARGO_PKG_VERSION");
    let out = version_check(&["--tag", "v99.0.0"]);
    assert_eq!(out.status.code(), Some(1), "{}", show(&out));
    let err = text(&out.stderr);
    assert!(err.contains("v99.0.0") && err.contains(v), "{}", show(&out));
}

/// RC N2 R3.S2 tag 与版本号一致：退出码 0。
#[test]
fn rc_n2_r3_s2_tag_matches() {
    let tag = format!("v{}", env!("CARGO_PKG_VERSION"));
    let out = version_check(&["--tag", &tag]);
    assert_eq!(out.status.code(), Some(0), "{}", show(&out));
}
