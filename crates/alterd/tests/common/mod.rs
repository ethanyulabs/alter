//! shell 契约测试与基准测试共用的辅助：真实的 `alterd run` 子进程与假 Electron 客户端。
// 每个测试 crate 只用到其中一部分
#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use wait_timeout::ChildExt;

/// 测试结束时一定杀掉子进程，避免残留的 alterd 串扰其他测试。
pub struct Daemon {
    pub child: Child,
}

impl Daemon {
    pub fn spawn(home: &Path) -> Daemon {
        let child = Command::new(env!("CARGO_BIN_EXE_alterd"))
            .arg("run")
            .env("ALTER_HOME", home)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("alterd 应当能启动");
        Daemon { child }
    }

    /// 启动并等到健康文件出现。
    pub fn start(home: &Path) -> Daemon {
        let mut d = Daemon::spawn(home);
        if !wait_until(Duration::from_secs(2), || home.join("alterd.json").exists()) {
            let _ = d.child.kill();
            panic!("2 秒内没有出现 alterd.json，stderr：{}", d.stderr());
        }
        d
    }

    pub fn wait(&mut self, timeout: Duration) -> Option<ExitStatus> {
        self.child.wait_timeout(timeout).unwrap()
    }

    pub fn stderr(&mut self) -> String {
        let mut s = String::new();
        if let Some(mut err) = self.child.stderr.take() {
            err.read_to_string(&mut s).unwrap();
        }
        s
    }

    /// SIGTERM 并等待正常退出。
    pub fn terminate(mut self) {
        // SAFETY：kill 只向指定 pid 发信号，不涉及内存
        let rc = unsafe { libc::kill(self.child.id() as libc::pid_t, libc::SIGTERM) };
        assert_eq!(rc, 0, "发送 SIGTERM 失败");
        let status = self.wait(Duration::from_secs(2)).expect("2 秒内应当退出");
        assert_eq!(status.code(), Some(0), "退出状态：{status:?}");
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// 每 20 ms 检查一次，直到条件成立或超时。
pub fn wait_until(timeout: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    cond()
}

pub fn sock_path(home: &Path) -> PathBuf {
    home.join("shell.sock")
}

/// 假 Electron 客户端：按行收发 JSON。
pub struct Client {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
}

impl Client {
    pub fn connect(home: &Path) -> Client {
        let stream = UnixStream::connect(sock_path(home)).expect("应当能连接 shell.sock");
        let writer = stream.try_clone().unwrap();
        Client {
            reader: BufReader::new(stream),
            writer,
        }
    }

    /// 在 `timeout` 内读一行 JSON；超时返回 None。
    pub fn recv_within(&mut self, timeout: Duration) -> Option<Value> {
        self.reader
            .get_ref()
            .set_read_timeout(Some(timeout))
            .unwrap();
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) => panic!("连接被关闭"),
            Ok(_) => Some(
                serde_json::from_str(&line).unwrap_or_else(|e| panic!("不是 JSON：{line}：{e}")),
            ),
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                None
            }
            Err(e) => panic!("读取失败：{e}"),
        }
    }

    /// 1 秒内必须收到一行。
    pub fn recv(&mut self) -> Value {
        self.recv_within(Duration::from_secs(1))
            .expect("1 秒内没有收到消息")
    }

    pub fn send_raw(&mut self, line: &str) {
        self.writer.write_all(line.as_bytes()).unwrap();
        self.writer.write_all(b"\n").unwrap();
    }

    pub fn send(&mut self, msg: Value) {
        self.send_raw(&msg.to_string());
    }
}

/// 启动 alterd 并连上，读掉初始快照；返回客户端与 Personal 的 id。
pub fn connected(home: &Path) -> (Daemon, Client, String) {
    let d = Daemon::start(home);
    let mut c = Client::connect(home);
    let snap = c.recv();
    let space = snap["spaces"][0]["id"].as_str().unwrap().to_owned();
    (d, c, space)
}

/// 发一条 openRequested（新建标签），返回下一条快照里最后一个标签的 url。
pub fn open_new(c: &mut Client, space: &str, input: &str) -> String {
    c.send(json!({"type": "shell.event.openRequested", "spaceId": space, "input": input}));
    let snap = c.recv();
    assert_eq!(snap["type"], "shell.stateChanged", "收到：{snap}");
    let tabs = snap["tabs"].as_array().unwrap();
    tabs.last()
        .unwrap_or_else(|| panic!("快照里没有标签：{snap}"))["url"]
        .as_str()
        .unwrap()
        .to_owned()
}
