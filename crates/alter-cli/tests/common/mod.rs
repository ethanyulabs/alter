//! CLI 测试共用：起一个真实的 alterd（与 `alter` 同一构建目录里的二进制）。
// 每个测试 crate 只用到其中一部分
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// 与 `alter` 同目录的 `alterd`；还没构建时先 `cargo build -p alterd`（同一 profile）。
pub fn alterd_bin() -> PathBuf {
    let alter = PathBuf::from(env!("CARGO_BIN_EXE_alter"));
    let alterd = alter.with_file_name("alterd");
    if !alterd.exists() {
        let release = alter
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|d| d == "release");
        let mut cmd = Command::new(env!("CARGO"));
        cmd.args(["build", "-q", "-p", "alterd"]);
        if release {
            cmd.arg("--release");
        }
        assert!(cmd.status().unwrap().success(), "构建 alterd 失败");
    }
    alterd
}

/// 测试结束时一定杀掉子进程。
pub struct Daemon {
    child: Child,
}

impl Daemon {
    /// 启动 alterd 并等到健康文件出现。
    pub fn start(home: &Path) -> Daemon {
        let child = Command::new(alterd_bin())
            .arg("run")
            .env("ALTER_HOME", home)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("alterd 应当能启动");
        let deadline = Instant::now() + Duration::from_secs(5);
        while !home.join("alterd.json").exists() {
            assert!(Instant::now() < deadline, "5 秒内没有出现 alterd.json");
            std::thread::sleep(Duration::from_millis(20));
        }
        Daemon { child }
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
