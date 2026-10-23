//! install 能力：`scripts/install.sh`。下载走假 Release 服务器，安装用 `hdiutil create` 做的假 app 包，
//! `--prefix` / `--bin-dir` 指向临时目录（非默认 prefix 时不会去退出、重开正在运行的 alter）。

mod common;

use std::path::{Path, PathBuf};
use std::process::Output;

use assert_cmd::Command;
use common::release_server::{ReleaseServer, fake_app_dmg};

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

/// 以假服务器为 Release API / 下载根运行 install.sh。
fn install(server: &ReleaseServer, args: &[&str]) -> Command {
    let mut cmd = Command::new("bash");
    cmd.arg(repo_root().join("scripts/install.sh"))
        .args(args)
        .env("ALTER_RELEASE_API", server.api())
        .env("ALTER_RELEASE_BASE", server.base())
        .timeout(std::time::Duration::from_secs(60));
    cmd
}

fn arg(p: &Path) -> &str {
    p.to_str().unwrap()
}

fn last_line(out: &Output) -> String {
    text(&out.stdout).lines().last().unwrap_or("").to_owned()
}

/// 当前挂载着的镜像路径（`hdiutil info` 的 image-path 行）。
fn mounted_images() -> Vec<String> {
    let out = std::process::Command::new("hdiutil")
        .arg("info")
        .output()
        .unwrap();
    text(&out.stdout)
        .lines()
        .filter_map(|l| {
            l.strip_prefix("image-path")
                .map(|r| r.trim_start_matches([' ', ':']).trim().to_owned())
        })
        .collect()
}

/// RC N2 R4.S1 从假 Release 服务器安装指定版本：最后一行 installed，bin-dir 的 alter 是指向包内 alter 的链接，DMG 已卸载。
#[test]
fn rc_n2_r4_s1_installs_given_version() {
    let server = ReleaseServer::start();
    let work = tempfile::tempdir().unwrap();
    server.publish("0.2.0", &fake_app_dmg("0.2.0", work.path()), true);
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    // 脚本的临时目录放在这里：并行的其他用例也会挂载 DMG，只检查这次安装自己的挂载
    let scratch = tempfile::tempdir().unwrap();

    let out = install(
        &server,
        &[
            "--version",
            "v0.2.0",
            "--prefix",
            arg(a.path()),
            "--bin-dir",
            arg(b.path()),
        ],
    )
    .env("TMPDIR", scratch.path())
    .output()
    .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", show(&out));
    let app = a.path().join("alter.app");
    assert_eq!(
        last_line(&out),
        format!("alter 0.2.0 installed to {}", app.display()),
        "{}",
        show(&out)
    );
    let link = b.path().join("alter");
    assert_eq!(
        std::fs::read_link(&link).unwrap(),
        app.join("Contents/Resources/bin/alter"),
        "{}",
        show(&out)
    );
    let v = std::process::Command::new(&link)
        .arg("--version")
        .output()
        .unwrap();
    assert_eq!(text(&v.stdout), "alter 0.2.0\n");
    let mine = std::fs::canonicalize(scratch.path()).unwrap();
    let left: Vec<String> = mounted_images()
        .into_iter()
        .filter(|p| p.starts_with(mine.to_str().unwrap()))
        .collect();
    assert!(left.is_empty(), "DMG 应当已卸载：{left:?}");
}

/// RC N2 R4.S2 省略版本时安装 latest（v0.3.0）。
#[test]
fn rc_n2_r4_s2_installs_latest() {
    let server = ReleaseServer::start();
    let work = tempfile::tempdir().unwrap();
    server.publish("0.3.0", &fake_app_dmg("0.3.0", work.path()), true);
    server.latest("v0.3.0");
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());

    let out = install(
        &server,
        &["--prefix", arg(a.path()), "--bin-dir", arg(b.path())],
    )
    .output()
    .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", show(&out));
    assert_eq!(
        last_line(&out),
        format!(
            "alter 0.3.0 installed to {}",
            a.path().join("alter.app").display()
        ),
        "{}",
        show(&out)
    );
}

/// RC N2 R4.S3 覆盖已安装的旧版本：bin-dir 的 alter 输出 0.3.0，prefix 下只有 alter.app 一个条目。
#[test]
fn rc_n2_r4_s3_replaces_old_version() {
    let server = ReleaseServer::start();
    let work = tempfile::tempdir().unwrap();
    server.publish("0.2.0", &fake_app_dmg("0.2.0", work.path()), true);
    server.publish("0.3.0", &fake_app_dmg("0.3.0", work.path()), true);
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    for v in ["v0.2.0", "v0.3.0"] {
        let out = install(
            &server,
            &[
                "--version",
                v,
                "--prefix",
                arg(a.path()),
                "--bin-dir",
                arg(b.path()),
            ],
        )
        .output()
        .unwrap();
        assert_eq!(out.status.code(), Some(0), "{v}：{}", show(&out));
    }
    let v = std::process::Command::new(b.path().join("alter"))
        .arg("--version")
        .output()
        .unwrap();
    assert_eq!(text(&v.stdout), "alter 0.3.0\n");
    let entries: Vec<_> = std::fs::read_dir(a.path())
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert_eq!(entries, vec![std::ffi::OsString::from("alter.app")]);
}

/// RC N2 R4.S4 bin 目录不在 PATH 时给出提示，退出码仍为 0。
#[test]
fn rc_n2_r4_s4_path_hint() {
    let server = ReleaseServer::start();
    let work = tempfile::tempdir().unwrap();
    server.publish("0.2.0", &fake_app_dmg("0.2.0", work.path()), true);
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());

    let out = install(
        &server,
        &[
            "--version",
            "v0.2.0",
            "--prefix",
            arg(a.path()),
            "--bin-dir",
            arg(b.path()),
        ],
    )
    .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
    .output()
    .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", show(&out));
    let bdir = b.path().display().to_string();
    assert!(
        text(&out.stderr)
            .lines()
            .any(|l| l.contains(&bdir) && l.contains("PATH")),
        "{}",
        show(&out)
    );
}

fn is_empty(dir: &Path) -> bool {
    std::fs::read_dir(dir).unwrap().next().is_none()
}

/// RC N2 R5.S1 校验和不符：退出码非 0，stderr 含 SHA256，prefix 与 bin-dir 仍为空。
#[test]
fn rc_n2_r5_s1_checksum_mismatch() {
    let server = ReleaseServer::start();
    let work = tempfile::tempdir().unwrap();
    server.publish("0.2.0", &fake_app_dmg("0.2.0", work.path()), false);
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let out = install(
        &server,
        &[
            "--version",
            "v0.2.0",
            "--prefix",
            arg(a.path()),
            "--bin-dir",
            arg(b.path()),
        ],
    )
    .output()
    .unwrap();
    assert_ne!(out.status.code(), Some(0), "{}", show(&out));
    assert!(text(&out.stderr).contains("SHA256"), "{}", show(&out));
    assert!(is_empty(a.path()) && is_empty(b.path()), "{}", show(&out));
}

/// RC N2 R5.S2 版本不存在（404）：退出码非 0，stderr 含 v9.9.9，prefix 与 bin-dir 仍为空。
#[test]
fn rc_n2_r5_s2_version_not_found() {
    let server = ReleaseServer::start();
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let out = install(
        &server,
        &[
            "--version",
            "v9.9.9",
            "--prefix",
            arg(a.path()),
            "--bin-dir",
            arg(b.path()),
        ],
    )
    .output()
    .unwrap();
    assert_ne!(out.status.code(), Some(0), "{}", show(&out));
    assert!(text(&out.stderr).contains("v9.9.9"), "{}", show(&out));
    assert!(is_empty(a.path()) && is_empty(b.path()), "{}", show(&out));
}

/// RC N2 R6.S1 预演：按顺序打印 [dry-run] 步骤，不改动文件、不发请求。
#[test]
fn rc_n2_r6_s1_dry_run() {
    let server = ReleaseServer::start();
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let out = install(
        &server,
        &[
            "--dry-run",
            "--version",
            "v0.2.0",
            "--prefix",
            arg(a.path()),
            "--bin-dir",
            arg(b.path()),
            "--with-skill",
        ],
    )
    .output()
    .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", show(&out));
    let stdout = text(&out.stdout);
    let wants = [
        "alter-0.2.0-arm64.dmg".to_owned(),
        "SHA256".to_owned(),
        format!("{}/alter.app", a.path().display()),
        format!("{}/alter", b.path().display()),
        "alter skill install".to_owned(),
    ];
    let lines: Vec<&str> = stdout
        .lines()
        .filter(|l| l.starts_with("[dry-run]"))
        .collect();
    let mut at = 0;
    for want in &wants {
        let pos = lines[at..]
            .iter()
            .position(|l| l.contains(want.as_str()))
            .unwrap_or_else(|| {
                panic!(
                    "缺少（或顺序不对）含「{want}」的 [dry-run] 行：{}",
                    show(&out)
                )
            });
        at += pos + 1;
    }
    assert!(is_empty(a.path()) && is_empty(b.path()), "{}", show(&out));
    assert!(
        server.requests().is_empty(),
        "不应发出请求：{:?}",
        server.requests()
    );
}

/// RC N2 T9 预演且省略版本：只打印「查询 latest 版本」，也不发请求。
#[test]
fn rc_n2_t9_dry_run_without_version() {
    let server = ReleaseServer::start();
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let out = install(
        &server,
        &[
            "--dry-run",
            "--prefix",
            arg(a.path()),
            "--bin-dir",
            arg(b.path()),
        ],
    )
    .output()
    .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", show(&out));
    assert!(
        text(&out.stdout).contains("[dry-run] 查询 latest 版本"),
        "{}",
        show(&out)
    );
    assert!(
        server.requests().is_empty(),
        "不应发出请求：{:?}",
        server.requests()
    );
}

/// RC N2 T9 预演、正在运行的就是 `<prefix>/alter.app`：打印退出与重新打开 alter 的步骤。
/// install.sh 只在这个包真实存在时才认为是它在运行，所以包建在临时目录里，不依赖本机的 /Applications。
#[test]
fn rc_n2_t9_dry_run_running_app() {
    let server = ReleaseServer::start();
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let app = a.path().join("alter.app");
    std::fs::create_dir(&app).unwrap();
    let mut fake = FakeApp::start(&app, false);
    let out = install(
        &server,
        &[
            "--dry-run",
            "--version",
            "v0.2.0",
            "--prefix",
            arg(a.path()),
            "--bin-dir",
            arg(b.path()),
        ],
    )
    .env("PATH", fake.path())
    .output()
    .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", show(&out));
    let stdout = text(&out.stdout);
    assert!(
        stdout.contains("[dry-run] 退出正在运行的 alter"),
        "{}",
        show(&out)
    );
    assert!(
        stdout.contains("[dry-run] 重新打开 alter"),
        "{}",
        show(&out)
    );
    assert!(fake.alive(), "预演不应退出 app");
}

/// RC N2 R6.S2 非 Apple Silicon：退出码 1，stderr 含 Apple Silicon，prefix 为空。
#[test]
fn rc_n2_r6_s2_rejects_non_arm64() {
    let server = ReleaseServer::start();
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let out = install(
        &server,
        &[
            "--version",
            "v0.2.0",
            "--prefix",
            arg(a.path()),
            "--bin-dir",
            arg(b.path()),
        ],
    )
    .env("ALTER_INSTALL_UNAME_M", "x86_64")
    .output()
    .unwrap();
    assert_eq!(out.status.code(), Some(1), "{}", show(&out));
    assert!(
        text(&out.stderr).contains("Apple Silicon"),
        "{}",
        show(&out)
    );
    assert!(is_empty(a.path()), "{}", show(&out));
}

/// RC N2 R7.S1 带 --with-skill 安装：安装完成后运行一次新装的 `alter skill install`。
#[test]
fn rc_n2_r7_s1_with_skill_runs_skill_install() {
    let server = ReleaseServer::start();
    let work = tempfile::tempdir().unwrap();
    server.publish("0.2.0", &fake_app_dmg("0.2.0", work.path()), true);
    let (a, b, c) = (
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
    );
    let args_file = c.path().join("skill-args");
    let out = install(
        &server,
        &[
            "--version",
            "v0.2.0",
            "--prefix",
            arg(a.path()),
            "--bin-dir",
            arg(b.path()),
            "--with-skill",
        ],
    )
    .env("ALTER_FAKE_ARGS_FILE", &args_file)
    .output()
    .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", show(&out));
    assert_eq!(
        std::fs::read_to_string(&args_file).unwrap_or_default(),
        "skill install\n",
        "{}",
        show(&out)
    );
}

/// RC N2 T10 `alter skill install` 失败时安装仍算成功，stderr 给出提示。
#[test]
fn rc_n2_t10_skill_failure_keeps_install() {
    let server = ReleaseServer::start();
    let work = tempfile::tempdir().unwrap();
    server.publish("0.2.0", &fake_app_dmg("0.2.0", work.path()), true);
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    // 假 alter 往一个目录写参数会失败（退出码非 0）
    let out = install(
        &server,
        &[
            "--version",
            "v0.2.0",
            "--prefix",
            arg(a.path()),
            "--bin-dir",
            arg(b.path()),
            "--with-skill",
        ],
    )
    .env("ALTER_FAKE_ARGS_FILE", a.path())
    .output()
    .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", show(&out));
    assert!(
        text(&out.stderr).contains("alter skill install"),
        "{}",
        show(&out)
    );
    assert!(last_line(&out).contains("installed to"), "{}", show(&out));
}

/// 假的「正在运行的 alter」：一个真实的进程，加上 PATH 里的假 `lsappinfo`（报它的 pid 与 `bundle`）
/// 和假 `open`（把参数写进 `open-args`）。`ignore_term` 为 true 时进程忽略 SIGTERM。
/// 进程经中间的 bash 放到后台，不是测试进程的子进程：退出后由 launchd 回收，不会变成僵尸
/// （真实的 app 也是这样，否则 `kill -0` 对已退出的僵尸进程仍然成功）。
struct FakeApp {
    pid: u32,
    dir: tempfile::TempDir,
}

impl FakeApp {
    fn start(bundle: &Path, ignore_term: bool) -> FakeApp {
        let trap = if ignore_term { "trap '' TERM; " } else { "" };
        let out = std::process::Command::new("bash")
            .args([
                "-c",
                &format!(
                    "bash -c \"{trap}while true; do sleep 0.1; done\" > /dev/null 2>&1 & echo $!"
                ),
            ])
            .output()
            .unwrap();
        let pid: u32 = text(&out.stdout).trim().parse().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let lsappinfo = format!(
            "#!/bin/bash\nkill -0 {pid} 2>/dev/null || exit 0\n\
             case \"$*\" in *pid*) echo '\"pid\"={pid}';; esac\n\
             case \"$*\" in *bundlepath*) echo '\"LSBundlePath\"=\"{bundle}\"';; esac\n",
            bundle = bundle.display()
        );
        let open = format!(
            "#!/bin/bash\necho \"$*\" >> '{}/open-args'\n",
            dir.path().display()
        );
        for (name, body) in [("lsappinfo", lsappinfo), ("open", open)] {
            let p = dir.path().join(name);
            std::fs::write(&p, body).unwrap();
            std::fs::set_permissions(&p, std::os::unix::fs::PermissionsExt::from_mode(0o755))
                .unwrap();
        }
        FakeApp { pid, dir }
    }

    /// 假命令目录在前的 PATH。
    fn path(&self) -> String {
        format!(
            "{}:/usr/bin:/bin:/usr/sbin:/sbin",
            self.dir.path().display()
        )
    }

    fn alive(&mut self) -> bool {
        std::process::Command::new("kill")
            .args(["-0", &self.pid.to_string()])
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }

    fn open_args(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("open-args")).unwrap_or_default()
    }
}

impl Drop for FakeApp {
    fn drop(&mut self) {
        let _ = std::process::Command::new("kill")
            .args(["-9", &self.pid.to_string()])
            .stderr(std::process::Stdio::null())
            .status();
    }
}

/// 包内 alter 报的版本。
fn installed_version(prefix: &Path) -> String {
    let out = std::process::Command::new(prefix.join("alter.app/Contents/Resources/bin/alter"))
        .arg("--version")
        .output()
        .unwrap();
    text(&out.stdout)
}

/// RC N2 T18 链接失败时回退：旧的 0.2.0 原样保留，不留 .old / .new（R4.S3 的失败路径）。
#[test]
fn rc_n2_t18_rolls_back_when_link_fails() {
    let server = ReleaseServer::start();
    let work = tempfile::tempdir().unwrap();
    server.publish("0.2.0", &fake_app_dmg("0.2.0", work.path()), true);
    server.publish("0.3.0", &fake_app_dmg("0.3.0", work.path()), true);
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let ok = install(
        &server,
        &[
            "--version",
            "v0.2.0",
            "--prefix",
            arg(a.path()),
            "--bin-dir",
            arg(b.path()),
        ],
    )
    .output()
    .unwrap();
    assert_eq!(ok.status.code(), Some(0), "{}", show(&ok));
    // bin-dir 是一个普通文件：链接这一步一定失败
    let not_dir = work.path().join("not-a-dir");
    std::fs::write(&not_dir, "x").unwrap();
    let out = install(
        &server,
        &[
            "--version",
            "v0.3.0",
            "--prefix",
            arg(a.path()),
            "--bin-dir",
            arg(&not_dir),
        ],
    )
    .output()
    .unwrap();
    assert_ne!(out.status.code(), Some(0), "{}", show(&out));
    assert_eq!(
        installed_version(a.path()),
        "alter 0.2.0\n",
        "{}",
        show(&out)
    );
    let entries: Vec<_> = std::fs::read_dir(a.path())
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert_eq!(
        entries,
        vec![std::ffi::OsString::from("alter.app")],
        "{}",
        show(&out)
    );
}

/// 先把 0.2.0 装进 `a`（bin-dir 为 `b`），并发布 0.3.0；返回 `<a>/alter.app` 的规范路径。
fn preinstall(server: &ReleaseServer, work: &Path, a: &Path, b: &Path) -> PathBuf {
    server.publish("0.2.0", &fake_app_dmg("0.2.0", work), true);
    server.publish("0.3.0", &fake_app_dmg("0.3.0", work), true);
    let first = install(
        server,
        &[
            "--version",
            "v0.2.0",
            "--prefix",
            arg(a),
            "--bin-dir",
            arg(b),
        ],
    )
    .output()
    .unwrap();
    assert_eq!(first.status.code(), Some(0), "{}", show(&first));
    std::fs::canonicalize(a).unwrap().join("alter.app")
}

/// RC N2 T18 正在运行的就是要替换的包：先退出它，装完重新打开。
#[test]
fn rc_n2_t18_quits_and_reopens_running_app() {
    let server = ReleaseServer::start();
    let work = tempfile::tempdir().unwrap();
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let app = preinstall(&server, work.path(), a.path(), b.path());
    let mut fake = FakeApp::start(&app, false);
    let out = install(
        &server,
        &[
            "--version",
            "v0.3.0",
            "--prefix",
            arg(a.path()),
            "--bin-dir",
            arg(b.path()),
        ],
    )
    .env("PATH", fake.path())
    .output()
    .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", show(&out));
    assert!(!fake.alive(), "正在运行的 app 应当被退出：{}", show(&out));
    let opened = std::fs::canonicalize(fake.open_args().trim()).unwrap();
    assert_eq!(opened, app, "{}", show(&out));
    assert_eq!(installed_version(a.path()), "alter 0.3.0\n");
}

/// RC N2 T18 app 不肯退出：10 秒后报错，什么都不改。
#[test]
fn rc_n2_t18_refuses_when_app_does_not_quit() {
    let server = ReleaseServer::start();
    let work = tempfile::tempdir().unwrap();
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let app = preinstall(&server, work.path(), a.path(), b.path());
    let mut fake = FakeApp::start(&app, true);
    let out = install(
        &server,
        &[
            "--version",
            "v0.3.0",
            "--prefix",
            arg(a.path()),
            "--bin-dir",
            arg(b.path()),
        ],
    )
    .env("PATH", fake.path())
    .output()
    .unwrap();
    assert_ne!(out.status.code(), Some(0), "{}", show(&out));
    assert!(text(&out.stderr).contains("没有退出"), "{}", show(&out));
    assert_eq!(
        installed_version(a.path()),
        "alter 0.2.0\n",
        "{}",
        show(&out)
    );
    let entries: Vec<_> = std::fs::read_dir(a.path())
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert_eq!(
        entries,
        vec![std::ffi::OsString::from("alter.app")],
        "{}",
        show(&out)
    );
    assert!(fake.alive());
    assert_eq!(fake.open_args(), "", "没退出过就不该重新打开");
}

/// RC N2 T18 正在运行的是别处的 alter.app：不碰它。
#[test]
fn rc_n2_t18_leaves_other_bundle_alone() {
    let server = ReleaseServer::start();
    let work = tempfile::tempdir().unwrap();
    server.publish("0.2.0", &fake_app_dmg("0.2.0", work.path()), true);
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    // 两个包都要真实存在（prefix 下已装着旧版，别处还有一个在运行）：任一个不存在时
    // install.sh 在比较路径之前就判定它不在运行，测不到「路径不同」这一支
    std::fs::create_dir(a.path().join("alter.app")).unwrap();
    let other = tempfile::tempdir().unwrap();
    let other_app = other.path().join("alter.app");
    std::fs::create_dir(&other_app).unwrap();
    let mut fake = FakeApp::start(&other_app, false);
    let out = install(
        &server,
        &[
            "--version",
            "v0.2.0",
            "--prefix",
            arg(a.path()),
            "--bin-dir",
            arg(b.path()),
        ],
    )
    .env("PATH", fake.path())
    .output()
    .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", show(&out));
    assert!(fake.alive(), "别处的 alter 不应被退出");
    assert_eq!(fake.open_args(), "");
}

/// RC N2 T18 相对路径、含空格的 --prefix 与 --bin-dir：链接仍指向正确的位置。
#[test]
fn rc_n2_t18_relative_paths_with_spaces() {
    let server = ReleaseServer::start();
    let work = tempfile::tempdir().unwrap();
    server.publish("0.2.0", &fake_app_dmg("0.2.0", work.path()), true);
    let cwd = tempfile::tempdir().unwrap();
    let out = install(
        &server,
        &[
            "--version",
            "v0.2.0",
            "--prefix",
            "my apps",
            "--bin-dir",
            "b dir",
        ],
    )
    .current_dir(cwd.path())
    .output()
    .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", show(&out));
    let v = std::process::Command::new(cwd.path().join("b dir/alter"))
        .arg("--version")
        .output()
        .unwrap();
    assert_eq!(text(&v.stdout), "alter 0.2.0\n", "{}", show(&out));
}

/// RC N2 T23 latest 查询失败（API 返回 404）：退出码 1，stderr 含「无法获取 latest 版本」（R4.S2 的失败路径）。
#[test]
fn rc_n2_t23_latest_query_failure_is_reported() {
    let server = ReleaseServer::start();
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let out = install(
        &server,
        &["--prefix", arg(a.path()), "--bin-dir", arg(b.path())],
    )
    .output()
    .unwrap();
    assert_eq!(out.status.code(), Some(1), "{}", show(&out));
    assert!(
        text(&out.stderr).contains("无法获取 latest 版本"),
        "{}",
        show(&out)
    );
}

/// RC N2 T23 `--version` 不是 v<x.y.z>：退出码 1，不发请求。
#[test]
fn rc_n2_t23_rejects_bad_version() {
    let server = ReleaseServer::start();
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    for bad in ["../evil", "0.2.0", "v0.2", "v0.2.0/x"] {
        let out = install(
            &server,
            &[
                "--version",
                bad,
                "--prefix",
                arg(a.path()),
                "--bin-dir",
                arg(b.path()),
            ],
        )
        .output()
        .unwrap();
        assert_eq!(out.status.code(), Some(1), "{bad}：{}", show(&out));
        assert!(
            text(&out.stderr).contains("v<x.y.z>"),
            "{bad}：{}",
            show(&out)
        );
    }
    assert!(
        server.requests().is_empty(),
        "不应发请求：{:?}",
        server.requests()
    );
}
