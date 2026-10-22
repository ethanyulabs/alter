//! alterd-lifecycle 能力的场景测试：以真实子进程驱动 `alterd run`，数据目录用临时目录隔离。

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use wait_timeout::ChildExt;

/// 测试结束时一定杀掉子进程，避免残留的 alterd 串扰其他测试。
struct Daemon {
    child: Child,
}

impl Daemon {
    fn spawn(home: &Path) -> Daemon {
        Daemon::spawn_with(home, &[], Stdio::inherit())
    }

    /// 以额外参数与指定的 stdin 启动 `alterd run`。
    fn spawn_with(home: &Path, args: &[&str], stdin: Stdio) -> Daemon {
        let child = Command::new(env!("CARGO_BIN_EXE_alterd"))
            .arg("run")
            .args(args)
            .env("ALTER_HOME", home)
            .stdin(stdin)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("alterd 应当能启动");
        Daemon { child }
    }

    fn pid(&self) -> u32 {
        self.child.id()
    }

    fn is_running(&mut self) -> bool {
        self.child.try_wait().unwrap().is_none()
    }

    fn wait(&mut self, timeout: Duration) -> Option<ExitStatus> {
        self.child.wait_timeout(timeout).unwrap()
    }

    fn stderr(&mut self) -> String {
        let mut s = String::new();
        if let Some(mut err) = self.child.stderr.take() {
            err.read_to_string(&mut s).unwrap();
        }
        s
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn health_path(home: &Path) -> PathBuf {
    home.join("alterd.json")
}

/// 每 20 ms 检查一次，直到条件成立或超时。
fn wait_until(timeout: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    cond()
}

fn read_health(home: &Path) -> serde_json::Value {
    let text = std::fs::read_to_string(health_path(home)).expect("健康文件应当可读");
    serde_json::from_str(&text).expect("健康文件应当是 JSON")
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

/// N1 R15.S1 启动后写出健康文件：1 秒内出现 alterd.json，字段正确，库已迁移到版本 2。
#[test]
fn n1_r15_s1_run_writes_health_file() {
    let home = tempfile::tempdir().unwrap();
    let before = now_millis();

    let mut daemon = Daemon::spawn(home.path());
    assert!(
        wait_until(Duration::from_secs(1), || health_path(home.path()).exists()),
        "1 秒内没有出现 alterd.json，stderr：{}",
        {
            let _ = daemon.child.kill();
            daemon.stderr()
        }
    );

    let health = read_health(home.path());
    let db = home.path().join("alter.db");
    assert_eq!(health["pid"], daemon.pid());
    assert_eq!(health["version"], "0.1.0");
    assert_eq!(health["dbPath"], db.to_str().unwrap());
    let started_at = health["startedAt"].as_i64().expect("startedAt 应当是整数");
    assert!(
        (before..=now_millis()).contains(&started_at),
        "startedAt {started_at} 不在启动前后的毫秒区间内"
    );

    let conn = rusqlite::Connection::open(&db).unwrap();
    let version: i64 = conn
        .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 2);
    assert!(daemon.is_running());
}

fn send_sigterm(pid: u32) {
    // SAFETY：kill 只向指定 pid 发信号，不涉及内存
    let rc = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
    assert_eq!(rc, 0, "发送 SIGTERM 失败");
}

/// R3.S2 SIGTERM 后清理退出：2 秒内退出码 0，alterd.json 被删除。
#[test]
fn r3_s2_sigterm_exits_cleanly() {
    let home = tempfile::tempdir().unwrap();
    let mut daemon = Daemon::spawn(home.path());
    assert!(wait_until(Duration::from_secs(1), || health_path(
        home.path()
    )
    .exists()));

    send_sigterm(daemon.pid());

    let status = daemon
        .wait(Duration::from_secs(2))
        .expect("收到 SIGTERM 后 2 秒内应当退出");
    assert_eq!(status.code(), Some(0), "退出状态：{status:?}");
    assert!(
        !health_path(home.path()).exists(),
        "alterd.json 应当已被删除"
    );
}

/// R3.S3 拒绝第二个实例：第二个进程 2 秒内退出码 3，stderr 含 already running，第一个不受影响。
#[test]
fn r3_s3_second_instance_is_rejected() {
    let home = tempfile::tempdir().unwrap();
    let mut first = Daemon::spawn(home.path());
    assert!(wait_until(Duration::from_secs(1), || health_path(
        home.path()
    )
    .exists()));

    let mut second = Daemon::spawn(home.path());
    let status = second
        .wait(Duration::from_secs(2))
        .expect("第二个实例应当在 2 秒内退出");
    assert_eq!(status.code(), Some(3), "退出状态：{status:?}");
    let stderr = second.stderr();
    assert!(stderr.contains("already running"), "stderr：{stderr}");
    // 可观测性 NFR：stderr 只有 JSON 行，其中一行带退出码 3
    let lines: Vec<serde_json::Value> = stderr
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|_| panic!("不是 JSON 行：{l}")))
        .collect();
    assert!(
        lines.iter().any(|v| v["fields"]["code"] == 3
            && v["fields"]["message"]
                .as_str()
                .is_some_and(|m| m.contains("already running"))),
        "没有 code=3 的 already running 日志：{stderr}"
    );

    assert!(first.is_running(), "第一个实例应当仍在运行");
    assert_eq!(read_health(home.path())["pid"], first.pid());
}

/// R3.S4 接管残留的健康文件：pid 999999 的旧文件 1 秒内被改写为新进程，1 秒时仍在运行。
#[test]
fn r3_s4_stale_health_file_is_taken_over() {
    let home = tempfile::tempdir().unwrap();
    let stale = serde_json::json!({
        "version": "0.1.0",
        "pid": 999_999,
        "startedAt": 1_760_918_400_000_i64,
        "dbPath": home.path().join("alter.db"),
    });
    std::fs::write(health_path(home.path()), stale.to_string()).unwrap();

    let mut daemon = Daemon::spawn(home.path());
    let pid = daemon.pid();
    assert!(
        wait_until(Duration::from_secs(1), || read_health(home.path())["pid"]
            == pid),
        "1 秒内 alterd.json 没有改写为新进程的 pid，当前内容：{}",
        read_health(home.path())
    );

    std::thread::sleep(Duration::from_secs(1));
    assert!(daemon.is_running(), "stderr：{}", {
        let _ = daemon.child.kill();
        daemon.stderr()
    });
}

/// R3.S3 拒绝第二个实例（并发启动）：同时启动 4 个，恰好 1 个留下运行，其余 2 秒内退出码 3。
#[test]
fn r3_s3_concurrent_start_leaves_one_instance() {
    let home = tempfile::tempdir().unwrap();
    let mut daemons: Vec<Daemon> = (0..4).map(|_| Daemon::spawn(home.path())).collect();

    std::thread::sleep(Duration::from_secs(2));
    let mut running = Vec::new();
    for d in daemons.iter_mut() {
        match d.wait(Duration::ZERO) {
            None => running.push(d.pid()),
            Some(status) => {
                assert_eq!(status.code(), Some(3), "退出状态：{status:?}");
                let stderr = d.stderr();
                assert!(stderr.contains("already running"), "stderr：{stderr}");
            }
        }
    }
    assert_eq!(running.len(), 1, "仍在运行的实例：{running:?}");
    assert_eq!(read_health(home.path())["pid"], running[0]);
}

/// R1.S1 stdin 写端关闭后退出：带 `--exit-on-stdin-close` 时，关闭管道写端后 2 秒内退出码 0，alterd.json 被删除。
#[test]
fn exit_on_stdin_r1_s1_closing_pipe_exits_cleanly() {
    let home = tempfile::tempdir().unwrap();
    let mut daemon = Daemon::spawn_with(home.path(), &["--exit-on-stdin-close"], Stdio::piped());
    assert!(
        wait_until(Duration::from_secs(1), || health_path(home.path()).exists()),
        "1 秒内没有出现 alterd.json，stderr：{}",
        {
            let _ = daemon.child.kill();
            daemon.stderr()
        }
    );

    // 丢掉写端即关闭管道
    drop(daemon.child.stdin.take());

    let status = daemon
        .wait(Duration::from_secs(2))
        .expect("stdin 关闭后 2 秒内应当退出");
    assert_eq!(status.code(), Some(0), "退出状态：{status:?}");
    assert!(
        !health_path(home.path()).exists(),
        "alterd.json 应当已被删除"
    );
}

/// R1.S2 不带参数时忽略 stdin：stdin 为 /dev/null（立即 EOF），3 秒后仍在运行，健康文件 pid 为该进程。
#[test]
fn exit_on_stdin_r1_s2_without_flag_ignores_stdin_eof() {
    let home = tempfile::tempdir().unwrap();
    let mut daemon = Daemon::spawn_with(home.path(), &[], Stdio::null());
    assert!(wait_until(Duration::from_secs(1), || health_path(
        home.path()
    )
    .exists()));

    std::thread::sleep(Duration::from_secs(3));
    assert!(daemon.is_running(), "stderr：{}", {
        let _ = daemon.child.kill();
        daemon.stderr()
    });
    assert_eq!(read_health(home.path())["pid"], daemon.pid());
}

/// R1.S3 stdin 读取出错时退出：以只读打开的目录作 stdin（读取返回 EISDIR），2 秒内退出码 0，alterd.json 不存在。
#[test]
fn exit_on_stdin_r1_s3_read_error_exits_cleanly() {
    let home = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let stdin = std::fs::File::open(dir.path()).expect("目录应当能以只读打开");
    let mut daemon =
        Daemon::spawn_with(home.path(), &["--exit-on-stdin-close"], Stdio::from(stdin));

    let status = daemon
        .wait(Duration::from_secs(2))
        .expect("stdin 读取出错后 2 秒内应当退出");
    assert_eq!(
        status.code(),
        Some(0),
        "退出状态：{status:?}，stderr：{}",
        daemon.stderr()
    );
    assert!(!health_path(home.path()).exists(), "alterd.json 应当不存在");
}
