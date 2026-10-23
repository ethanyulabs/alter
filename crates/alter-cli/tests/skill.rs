//! cli 能力：`alter skill install|uninstall`。HOME 指向临时目录，skill 源用 `ALTER_SKILL_DIR` 指向的 fixture。

use std::path::{Path, PathBuf};
use std::process::Output;

use assert_cmd::Command;

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

/// 一个 fixture skill：`SKILL.md` 与 `references/api.md`。
fn fixture_skill(dir: &Path) -> PathBuf {
    let src = dir.join("alter-browser");
    std::fs::create_dir_all(src.join("references")).unwrap();
    std::fs::write(
        src.join("SKILL.md"),
        "---\nname: alter-browser\n---\nfixture\n",
    )
    .unwrap();
    std::fs::write(src.join("references/api.md"), "### page.click\n").unwrap();
    src
}

fn skill(home: &Path, src: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_alter"))
        .arg("skill")
        .args(args)
        .env("HOME", home)
        .env("ALTER_SKILL_DIR", src)
        .timeout(std::time::Duration::from_secs(30))
        .output()
        .unwrap()
}

/// RC N2 R9.S1 装到已存在的 agent 目录：只有 ~/.claude/skills 存在，装进去并写 .alter-skill，不建 ~/.codex/skills。
#[test]
fn rc_n2_r9_s1_installs_into_existing_agent_dirs() {
    let home = tempfile::tempdir().unwrap();
    let fixtures = tempfile::tempdir().unwrap();
    let src = fixture_skill(fixtures.path());
    std::fs::create_dir_all(home.path().join(".claude/skills")).unwrap();

    let out = skill(home.path(), &src, &["install"]);
    assert_eq!(out.status.code(), Some(0), "{}", show(&out));
    let dest = home.path().join(".claude/skills/alter-browser");
    assert_eq!(
        text(&out.stdout),
        format!("installed alter-browser -> {}\n", dest.display()),
        "{}",
        show(&out)
    );
    assert_eq!(
        std::fs::read(dest.join("SKILL.md")).unwrap(),
        std::fs::read(src.join("SKILL.md")).unwrap()
    );
    assert_eq!(
        std::fs::read(dest.join("references/api.md")).unwrap(),
        std::fs::read(src.join("references/api.md")).unwrap()
    );
    assert_eq!(
        std::fs::read_to_string(dest.join(".alter-skill")).unwrap(),
        env!("CARGO_PKG_VERSION")
    );
    assert!(!home.path().join(".codex/skills").exists());
}

/// RC N2 R9.S2 两个目录都不存在：退出码 1，stderr 提示 --target。
#[test]
fn rc_n2_r9_s2_no_agent_dirs() {
    let home = tempfile::tempdir().unwrap();
    let fixtures = tempfile::tempdir().unwrap();
    let src = fixture_skill(fixtures.path());
    let out = skill(home.path(), &src, &["install"]);
    assert_eq!(out.status.code(), Some(1), "{}", show(&out));
    assert!(text(&out.stderr).contains("--target"), "{}", show(&out));
}

/// RC N2 R9.S3 指定目录并覆盖旧版本：SKILL.md 换成新的，多余的 old.md 被删掉。
#[test]
fn rc_n2_r9_s3_target_replaces_old_install() {
    let home = tempfile::tempdir().unwrap();
    let fixtures = tempfile::tempdir().unwrap();
    let src = fixture_skill(fixtures.path());
    let target = tempfile::tempdir().unwrap();
    let old = target.path().join("alter-browser");
    std::fs::create_dir_all(&old).unwrap();
    std::fs::write(old.join("SKILL.md"), "old content\n").unwrap();
    std::fs::write(old.join("old.md"), "stale\n").unwrap();

    let out = skill(
        home.path(),
        &src,
        &["install", "--target", target.path().to_str().unwrap()],
    );
    assert_eq!(out.status.code(), Some(0), "{}", show(&out));
    assert_eq!(
        std::fs::read(old.join("SKILL.md")).unwrap(),
        std::fs::read(src.join("SKILL.md")).unwrap()
    );
    assert!(
        !old.join("old.md").exists(),
        "旧版本留下的 old.md 应当被删掉"
    );
}

/// RC N2 R10.S1 只删除自己装的：.claude 里 alter 装的被删，.codex 里用户手写的（没有 .alter-skill）保留并提示。
#[test]
fn rc_n2_r10_s1_uninstall_only_marked() {
    let home = tempfile::tempdir().unwrap();
    let fixtures = tempfile::tempdir().unwrap();
    let src = fixture_skill(fixtures.path());
    std::fs::create_dir_all(home.path().join(".claude/skills")).unwrap();
    let out = skill(home.path(), &src, &["install"]);
    assert_eq!(out.status.code(), Some(0), "{}", show(&out));
    let own = home.path().join(".codex/skills/alter-browser");
    std::fs::create_dir_all(&own).unwrap();
    std::fs::write(own.join("SKILL.md"), "user's own\n").unwrap();

    let out = skill(home.path(), &src, &["uninstall"]);
    assert_eq!(out.status.code(), Some(0), "{}", show(&out));
    assert!(
        !home.path().join(".claude/skills/alter-browser").exists(),
        "{}",
        show(&out)
    );
    assert!(own.join("SKILL.md").exists(), "{}", show(&out));
    let err = text(&out.stderr);
    assert!(
        err.lines()
            .any(|l| l.contains(".codex/skills/alter-browser") && l.contains("未删除")),
        "{}",
        show(&out)
    );
}

/// 目录下的条目名，排好序。
fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

/// RC N2 T20 复制失败时旧版本原样保留，不留暂存目录（R9.S3 的失败路径）。
#[test]
fn rc_n2_t20_copy_failure_keeps_old_install() {
    let home = tempfile::tempdir().unwrap();
    let fixtures = tempfile::tempdir().unwrap();
    let src = fixture_skill(fixtures.path());
    let target = tempfile::tempdir().unwrap();
    let old = target.path().join("alter-browser");
    std::fs::create_dir_all(&old).unwrap();
    std::fs::write(old.join("SKILL.md"), "old content\n").unwrap();
    // 源里一个读不了的文件：复制到一半失败
    let unreadable = src.join("references/secret.md");
    std::fs::write(&unreadable, "x").unwrap();
    std::fs::set_permissions(
        &unreadable,
        std::os::unix::fs::PermissionsExt::from_mode(0o000),
    )
    .unwrap();

    let out = skill(
        home.path(),
        &src,
        &["install", "--target", target.path().to_str().unwrap()],
    );
    std::fs::set_permissions(
        &unreadable,
        std::os::unix::fs::PermissionsExt::from_mode(0o644),
    )
    .unwrap();
    assert_eq!(out.status.code(), Some(1), "{}", show(&out));
    assert_eq!(
        std::fs::read_to_string(old.join("SKILL.md")).unwrap(),
        "old content\n"
    );
    assert_eq!(
        names(target.path()),
        vec!["alter-browser".to_owned()],
        "{}",
        show(&out)
    );
}

/// RC N2 T20 源与目标是同一个目录：拒绝，什么都不删。
#[test]
fn rc_n2_t20_rejects_same_source_and_target() {
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let src = fixture_skill(target.path());
    let out = skill(
        home.path(),
        &src,
        &["install", "--target", target.path().to_str().unwrap()],
    );
    assert_eq!(out.status.code(), Some(1), "{}", show(&out));
    assert!(text(&out.stderr).contains("相同"), "{}", show(&out));
    assert!(src.join("SKILL.md").is_file(), "源不应被删掉");
}
