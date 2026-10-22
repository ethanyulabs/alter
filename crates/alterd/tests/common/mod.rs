//! shell 契约测试与基准测试共用的辅助：真实的 `alterd run` 子进程与假 Electron 客户端。
// 每个测试 crate 只用到其中一部分
#![allow(dead_code)]

pub mod fake_cdp;
pub mod fixture_profile;

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
        Daemon::spawn_with_stderr(home, Stdio::piped())
    }

    /// 以指定的 stderr 启动。会产生大量日志的测试要用 `Stdio::null()`：没人读的管道写满 64 KB 后，
    /// alterd 会卡在写日志上。
    pub fn spawn_with_stderr(home: &Path, stderr: Stdio) -> Daemon {
        let child = Command::new(env!("CARGO_BIN_EXE_alterd"))
            .arg("run")
            .env("ALTER_HOME", home)
            .stdout(Stdio::null())
            .stderr(stderr)
            .spawn()
            .expect("alterd 应当能启动");
        Daemon { child }
    }

    /// 带额外环境变量启动（stderr 用管道），等到健康文件出现。
    pub fn start_with_env(home: &Path, envs: &[(&str, &str)]) -> Daemon {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_alterd"));
        cmd.arg("run")
            .env("ALTER_HOME", home)
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        for (k, v) in envs {
            cmd.env(k, v);
        }
        let child = cmd.spawn().expect("alterd 应当能启动");
        Daemon::wait_ready(Daemon { child }, home)
    }

    /// SIGTERM，等它退出，返回它写到 stderr 的全部内容。
    pub fn stop_and_stderr(mut self) -> String {
        // SAFETY：kill 只向指定 pid 发信号，不涉及内存
        unsafe { libc::kill(self.child.id() as libc::pid_t, libc::SIGTERM) };
        self.wait(Duration::from_secs(2));
        self.stderr()
    }

    /// 启动并等到健康文件出现。
    pub fn start(home: &Path) -> Daemon {
        Daemon::wait_ready(Daemon::spawn(home), home)
    }

    /// 丢弃 stderr 启动并等到健康文件出现。
    pub fn start_quiet(home: &Path) -> Daemon {
        Daemon::wait_ready(Daemon::spawn_with_stderr(home, Stdio::null()), home)
    }

    fn wait_ready(mut d: Daemon, home: &Path) -> Daemon {
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

    /// 写端的一个副本，供另开线程发送（测试一边不读一边大量发送时，避免两端互相写满而卡住）。
    pub fn writer(&self) -> UnixStream {
        self.writer.try_clone().unwrap()
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

/// 下一条必须是 `shell.error` 且错误码为 `code`。
pub fn expect_error(c: &mut Client, code: &str) {
    let msg = c.recv();
    assert_eq!(msg["type"], "shell.error", "收到：{msg}");
    assert_eq!(msg["code"], code, "收到：{msg}");
}

/// 1 秒内不应再收到任何消息（尤其是 `shell.stateChanged`）。
pub fn expect_silence(c: &mut Client) {
    if let Some(msg) = c.recv_within(Duration::from_secs(1)) {
        panic!("不应收到消息，却收到：{msg}");
    }
}

/// 下一条必须是快照，返回它。
pub fn expect_snapshot(c: &mut Client) -> Value {
    let snap = c.recv();
    assert_eq!(snap["type"], "shell.stateChanged", "收到：{snap}");
    snap
}

/// 新建 Space 并返回随后的快照。
pub fn create_space(c: &mut Client, name: &str, color: &str) -> Value {
    c.send(json!({"type": "shell.event.spaceCreated", "name": name, "color": color}));
    expect_snapshot(c)
}

/// 快照里名为 `name` 的 Space 的 id。
pub fn space_id(snap: &Value, name: &str) -> String {
    snap["spaces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == name)
        .unwrap_or_else(|| panic!("快照里没有 Space {name}：{snap}"))["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// 快照里 Space 的名称，按顺序。
pub fn space_names(snap: &Value) -> Vec<String> {
    snap["spaces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap().to_owned())
        .collect()
}

/// 在 `home` 下建一个只应用了迁移 0001 的 alter.db（不经过 alterd）。
pub fn v1_db(home: &Path) -> rusqlite::Connection {
    let conn = rusqlite::Connection::open(home.join("alter.db")).unwrap();
    conn.execute_batch(include_str!("../../../alter-core/migrations/0001_init.sql"))
        .unwrap();
    conn.execute(
        "INSERT INTO schema_version (version, applied_at) VALUES (1, 0)",
        [],
    )
    .unwrap();
    conn
}

/// 读 `gateway.json`。
pub fn gateway_file(home: &Path) -> Value {
    let text = std::fs::read_to_string(home.join("gateway.json")).expect("应当有 gateway.json");
    serde_json::from_str(&text).unwrap()
}

pub type Ws = tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>;

/// 连 Gateway：`token` 为 None 时不带 Authorization 头。握手被拒时返回 HTTP 状态码。
pub fn ws_connect(home: &Path, token: Option<&str>) -> Result<Ws, u16> {
    use tungstenite::client::IntoClientRequest;
    let gw = gateway_file(home);
    let url = format!("ws://127.0.0.1:{}/rpc", gw["port"]);
    let mut req = url.into_client_request().unwrap();
    if let Some(token) = token {
        req.headers_mut()
            .insert("Authorization", format!("Bearer {token}").parse().unwrap());
    }
    match tungstenite::connect(req) {
        Ok((ws, _)) => Ok(ws),
        Err(tungstenite::Error::Http(resp)) => Err(resp.status().as_u16()),
        Err(e) => panic!("连接 Gateway 失败：{e}"),
    }
}

/// 用 gateway.json 里的 token 连上 Gateway。
pub fn gateway_client(home: &Path) -> Ws {
    let token = gateway_file(home)["token"].as_str().unwrap().to_owned();
    ws_connect(home, Some(&token)).expect("带正确 token 应当能连上")
}

/// 发一个 JSON-RPC 请求并等它的响应（跳过通知）；5 秒没有响应即失败。
pub fn rpc(ws: &mut Ws, id: u64, method: &str, params: Value) -> Value {
    let req = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
    ws.send(tungstenite::Message::Text(req.to_string().into()))
        .unwrap();
    set_ws_timeout(ws, Duration::from_secs(5));
    loop {
        let msg = ws.read().expect("应当收到响应");
        if let tungstenite::Message::Text(text) = msg {
            let v: Value = serde_json::from_str(&text).unwrap();
            if v["id"] == id {
                return v;
            }
        }
    }
}

/// 在 `timeout` 内读下一条文本消息（响应或通知）；超时返回 None。
pub fn ws_recv_within(ws: &mut Ws, timeout: Duration) -> Option<Value> {
    let deadline = Instant::now() + timeout;
    loop {
        let left = deadline.checked_duration_since(Instant::now())?;
        set_ws_timeout(ws, left.max(Duration::from_millis(1)));
        match ws.read() {
            Ok(tungstenite::Message::Text(text)) => {
                return Some(serde_json::from_str(&text).unwrap());
            }
            Ok(_) => continue,
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                return None;
            }
            Err(e) => panic!("读 Gateway 消息失败：{e}"),
        }
    }
}

fn set_ws_timeout(ws: &mut Ws, timeout: Duration) {
    if let tungstenite::stream::MaybeTlsStream::Plain(s) = ws.get_mut() {
        s.set_read_timeout(Some(timeout)).unwrap();
    }
}
