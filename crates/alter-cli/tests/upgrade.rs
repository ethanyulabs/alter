//! cli 能力：`alter upgrade`。Release API 与下载都走假 Release 服务器。

mod common;

use std::path::Path;
use std::process::Output;

use assert_cmd::Command;
use common::release_server::ReleaseServer;

const CURRENT: &str = env!("CARGO_PKG_VERSION");

/// 比自身新一个次版本号的版本（`0.1.0` → `0.2.0`）。
fn newer() -> String {
    let mut parts: Vec<u64> = CURRENT.split('.').map(|p| p.parse().unwrap()).collect();
    parts[1] += 1;
    parts[2] = 0;
    format!("{}.{}.{}", parts[0], parts[1], parts[2])
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

fn upgrade(exe: &Path, server: &ReleaseServer, args: &[&str]) -> Output {
    Command::new(exe)
        .arg("upgrade")
        .args(args)
        .env("ALTER_RELEASE_API", server.api())
        .env("ALTER_RELEASE_BASE", server.base())
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .unwrap()
}

fn alter() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_alter"))
}

/// 没有请求过任何 install.sh。
fn no_install_sh(server: &ReleaseServer) {
    let asked: Vec<String> = server
        .requests()
        .into_iter()
        .filter(|p| p.ends_with("/install.sh"))
        .collect();
    assert!(asked.is_empty(), "不应请求 install.sh：{asked:?}");
}

/// RC N2 R11.S1 检查到新版本：一行提示；--json 输出 current / latest / updateAvailable。
#[test]
fn rc_n2_r11_s1_check_finds_newer() {
    let server = ReleaseServer::start();
    let latest = newer();
    server.latest(&format!("v{latest}"));
    let out = upgrade(alter(), &server, &["--check"]);
    assert_eq!(out.status.code(), Some(0), "{}", show(&out));
    assert_eq!(
        text(&out.stdout),
        format!("alter {latest} is available (current {CURRENT})\n"),
        "{}",
        show(&out)
    );
    let out = upgrade(alter(), &server, &["--check", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", show(&out));
    assert_eq!(
        text(&out.stdout),
        format!("{{\"current\":\"{CURRENT}\",\"latest\":\"{latest}\",\"updateAvailable\":true}}\n"),
        "{}",
        show(&out)
    );
    no_install_sh(&server);
}

/// RC N2 R11.S2 已是最新：一行 up to date，退出码 0，不请求 install.sh。
#[test]
fn rc_n2_r11_s2_up_to_date() {
    let server = ReleaseServer::start();
    server.latest(&format!("v{CURRENT}"));
    let out = upgrade(alter(), &server, &[]);
    assert_eq!(out.status.code(), Some(0), "{}", show(&out));
    assert_eq!(
        text(&out.stdout),
        format!("alter {CURRENT} is up to date\n"),
        "{}",
        show(&out)
    );
    no_install_sh(&server);
}

/// RC N2 R11.S4 取不到版本信息：Release API 指向没有监听的端口，退出码 1。
#[test]
fn rc_n2_r11_s4_unreachable_api() {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let out = Command::new(alter())
        .args(["upgrade", "--check"])
        .env("ALTER_RELEASE_API", format!("http://127.0.0.1:{port}/api"))
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{}", show(&out));
    assert!(
        text(&out.stderr).contains("无法获取最新版本"),
        "{}",
        show(&out)
    );
}

/// RC N2 R11.S5 不是从安装包运行：有新版本时退出码 1，提示 install.sh，不请求 install.sh。
#[test]
fn rc_n2_r11_s5_not_in_bundle() {
    let server = ReleaseServer::start();
    server.latest(&format!("v{}", newer()));
    let out = upgrade(alter(), &server, &[]);
    assert_eq!(out.status.code(), Some(1), "{}", show(&out));
    assert!(text(&out.stderr).contains("install.sh"), "{}", show(&out));
    no_install_sh(&server);
}

/// 把构建出的 alter 复制进一个假的 `<A>/alter.app/Contents/Resources/bin/alter`，返回 (A, 包内 alter)。
fn fake_install(dir: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let a = std::fs::canonicalize(dir).unwrap();
    let bin = a.join("alter.app/Contents/Resources/bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::copy(alter(), bin.join("alter")).unwrap();
    (a, bin.join("alter"))
}

/// RC N2 R11.S3 升级时调用 install.sh：带上 --version、当前 app 所在目录、链接所在目录；之后刷新装过的 skill。
#[test]
fn rc_n2_r11_s3_runs_install_sh_and_refreshes_skill() {
    let server = ReleaseServer::start();
    let latest = newer();
    server.latest(&format!("v{latest}"));
    let (dir_a, b, c, h) = (
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
    );
    let (a, inner) = fake_install(dir_a.path());
    let link = b.path().join("alter");
    std::os::unix::fs::symlink(&inner, &link).unwrap();
    let skill = h.path().join(".claude/skills/alter-browser");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(skill.join(".alter-skill"), CURRENT).unwrap();
    // 假 install.sh：记下参数；先删再写，用一个记录参数的脚本替换包内的 alter
    let install_sh = format!(
        "#!/bin/bash\necho \"$*\" > '{c}/install-args'\nrm -f '{inner}'\n\
         printf '#!/bin/bash\\necho \"$*\" > %s\\n' '{c}/skill-args' > '{inner}'\nchmod +x '{inner}'\n",
        c = c.path().display(),
        inner = inner.display()
    );
    server.route(&format!("/download/v{latest}/install.sh"), 200, install_sh);

    let out = Command::new(&link)
        .arg("upgrade")
        .env("ALTER_RELEASE_API", server.api())
        .env("ALTER_RELEASE_BASE", server.base())
        .env("HOME", h.path())
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", show(&out));
    assert_eq!(
        std::fs::read_to_string(c.path().join("install-args")).unwrap_or_default(),
        format!(
            "--version v{latest} --prefix {} --bin-dir {}\n",
            a.display(),
            b.path().display()
        ),
        "{}",
        show(&out)
    );
    assert_eq!(
        std::fs::read_to_string(c.path().join("skill-args")).unwrap_or_default(),
        format!(
            "skill install --target {}\n",
            h.path().join(".claude/skills").display()
        ),
        "{}",
        show(&out)
    );
}

/// RC N2 T12 install.sh 失败：退出码 1，stderr 含「升级失败」，不刷新 skill。
#[test]
fn rc_n2_t12_install_sh_failure() {
    let server = ReleaseServer::start();
    let latest = newer();
    server.latest(&format!("v{latest}"));
    let (dir_a, b, h) = (
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
    );
    let (_, inner) = fake_install(dir_a.path());
    let link = b.path().join("alter");
    std::os::unix::fs::symlink(&inner, &link).unwrap();
    let skill = h.path().join(".claude/skills/alter-browser");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(skill.join(".alter-skill"), CURRENT).unwrap();
    server.route(
        &format!("/download/v{latest}/install.sh"),
        200,
        "#!/bin/bash\nexit 3\n",
    );

    let out = Command::new(&link)
        .arg("upgrade")
        .env("ALTER_RELEASE_API", server.api())
        .env("ALTER_RELEASE_BASE", server.base())
        .env("HOME", h.path())
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{}", show(&out));
    assert!(text(&out.stderr).contains("升级失败"), "{}", show(&out));
    assert_eq!(
        std::fs::read_to_string(skill.join(".alter-skill")).unwrap(),
        CURRENT,
        "skill 不应被改动"
    );
}

/// 比自身旧的版本（`0.1.0` → `0.0.9`，`1.2.3` → `1.2.2`）。
fn older() -> String {
    let p: Vec<u64> = CURRENT.split('.').map(|x| x.parse().unwrap()).collect();
    if p[2] > 0 {
        format!("{}.{}.{}", p[0], p[1], p[2] - 1)
    } else if p[1] > 0 {
        format!("{}.{}.9", p[0], p[1] - 1)
    } else {
        format!("{}.9.9", p[0] - 1)
    }
}

/// RC N2 T19 latest 比自身旧（Release 被撤回）：报 up to date，不装旧版本（R11.S2）。
#[test]
fn rc_n2_t19_older_latest_is_up_to_date() {
    let server = ReleaseServer::start();
    server.latest(&format!("v{}", older()));
    let out = upgrade(alter(), &server, &[]);
    assert_eq!(out.status.code(), Some(0), "{}", show(&out));
    assert_eq!(
        text(&out.stdout),
        format!("alter {CURRENT} is up to date\n"),
        "{}",
        show(&out)
    );
    no_install_sh(&server);
}

/// RC N2 T19 `--json` 必须与 `--check` 一起用：单独用是用法错误，不访问网络（R11.S1）。
#[test]
fn rc_n2_t19_json_requires_check() {
    let server = ReleaseServer::start();
    server.latest(&format!("v{}", newer()));
    let out = upgrade(alter(), &server, &["--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", show(&out));
    assert!(
        server.requests().is_empty(),
        "不应访问网络：{:?}",
        server.requests()
    );
}

/// RC N2 T19 `--version` 不是 v<x.y.z>：退出码 1，不访问网络。
#[test]
fn rc_n2_t19_rejects_bad_version() {
    let server = ReleaseServer::start();
    for bad in ["foo", "0.2.0", "v0.2", "vv0.2.0", ""] {
        let out = upgrade(alter(), &server, &["--version", bad]);
        assert_eq!(out.status.code(), Some(1), "{bad}：{}", show(&out));
        assert!(
            text(&out.stderr).contains("v<x.y.z>"),
            "{bad}：{}",
            show(&out)
        );
    }
    assert!(
        server.requests().is_empty(),
        "不应访问网络：{:?}",
        server.requests()
    );
}

/// RC N2 T19 路径只是形似安装包（`x.app/Other/Resources/bin/alter`）：当作不在包内（R11.S5）。
#[test]
fn rc_n2_t19_strict_bundle_layout() {
    let server = ReleaseServer::start();
    server.latest(&format!("v{}", newer()));
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("x.app/Other/Resources/bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::copy(alter(), bin.join("alter")).unwrap();
    let out = upgrade(&bin.join("alter"), &server, &[]);
    assert_eq!(out.status.code(), Some(1), "{}", show(&out));
    assert!(text(&out.stderr).contains("install.sh"), "{}", show(&out));
    no_install_sh(&server);
}
