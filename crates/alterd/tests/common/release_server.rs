//! 假 Release 服务器：127.0.0.1 上的最小 HTTP 服务，代替 GitHub 的 Release API 与资产下载。
//! 按路径返回登记好的状态码与内容（没登记的 404），并记下每个请求的路径与请求头。只支持 GET。
//! `hold` 可以扣住某个路径的响应，直到 `release`。

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

type Routes = Arc<Mutex<HashMap<String, (u16, Vec<u8>)>>>;

pub struct ReleaseServer {
    pub port: u16,
    routes: Routes,
    requests: Arc<Mutex<Vec<String>>>,
    headers: Arc<Mutex<Vec<String>>>,
    held: Arc<Mutex<std::collections::HashSet<String>>>,
}

impl ReleaseServer {
    pub fn start() -> ReleaseServer {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let routes: Routes = Arc::default();
        let requests: Arc<Mutex<Vec<String>>> = Arc::default();
        let headers: Arc<Mutex<Vec<String>>> = Arc::default();
        let held: Arc<Mutex<std::collections::HashSet<String>>> = Arc::default();
        let (r, q, h, hold) = (
            routes.clone(),
            requests.clone(),
            headers.clone(),
            held.clone(),
        );
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut line = String::new();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                if reader.read_line(&mut line).is_err() {
                    continue;
                }
                // 其余请求头：记下来
                let mut all = String::new();
                let mut header = String::new();
                while reader.read_line(&mut header).is_ok_and(|n| n > 2) {
                    all.push_str(&header);
                    header.clear();
                }
                let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
                q.lock().unwrap().push(path.clone());
                h.lock().unwrap().push(all);
                while hold.lock().unwrap().contains(&path) {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                let (status, body) = r
                    .lock()
                    .unwrap()
                    .get(&path)
                    .cloned()
                    .unwrap_or((404, b"not found".to_vec()));
                let head = format!(
                    "HTTP/1.1 {status} X\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&body);
            }
        });
        ReleaseServer {
            port,
            routes,
            requests,
            headers,
            held,
        }
    }

    /// `ALTER_RELEASE_API` 的值。
    pub fn api(&self) -> String {
        format!("http://127.0.0.1:{}/api", self.port)
    }

    /// `ALTER_RELEASE_BASE` 的值。
    pub fn base(&self) -> String {
        format!("http://127.0.0.1:{}/download", self.port)
    }

    pub fn route(&self, path: &str, status: u16, body: impl Into<Vec<u8>>) {
        self.routes
            .lock()
            .unwrap()
            .insert(path.to_owned(), (status, body.into()));
    }

    /// latest 指向 `tag`。
    pub fn latest(&self, tag: &str) {
        let body =
            format!(r#"{{"tag_name":"{tag}","html_url":"https://example.invalid/r/{tag}"}}"#);
        self.route("/api/releases/latest", 200, body);
    }

    /// 发布 `v<version>`：DMG 与它的 `.sha256`（`good_sha` 为 false 时故意写错）。
    pub fn publish(&self, version: &str, dmg: &Path, good_sha: bool) {
        let bytes = std::fs::read(dmg).unwrap();
        let name = format!("alter-{version}-arm64.dmg");
        let sha = if good_sha {
            sha256(dmg)
        } else {
            "0".repeat(64)
        };
        self.route(&format!("/download/v{version}/{name}"), 200, bytes);
        self.route(
            &format!("/download/v{version}/{name}.sha256"),
            200,
            format!("{sha}  {name}\n"),
        );
    }

    /// 扣住 `path` 的响应，直到 `release(path)`。
    pub fn hold(&self, path: &str) {
        self.held.lock().unwrap().insert(path.to_owned());
    }

    pub fn release(&self, path: &str) {
        self.held.lock().unwrap().remove(path);
    }

    /// 每个请求的请求头（不含请求行），顺序同 `requests`。
    pub fn headers(&self) -> Vec<String> {
        self.headers.lock().unwrap().clone()
    }

    /// 到目前为止收到的请求路径。
    pub fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

fn sha256(file: &Path) -> String {
    let out = Command::new("shasum")
        .args(["-a", "256"])
        .arg(file)
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap()[..64].to_owned()
}

/// 用 `hdiutil create` 做一个假 app 包的 DMG：里面的 `alter.app/Contents/Resources/bin/alter` 是 bash 脚本，
/// `--version` 时打印 `alter <version>`，其他参数在设置了 `ALTER_FAKE_ARGS_FILE` 时追加写进那个文件。
pub fn fake_app_dmg(version: &str, out_dir: &Path) -> PathBuf {
    let src = out_dir.join(format!("src-{version}"));
    let bin = src.join("alter.app/Contents/Resources/bin");
    std::fs::create_dir_all(&bin).unwrap();
    let script = format!(
        "#!/bin/bash\nif [ \"$1\" = \"--version\" ]; then echo \"alter {version}\"; exit 0; fi\n\
         if [ -n \"$ALTER_FAKE_ARGS_FILE\" ]; then echo \"$*\" >> \"$ALTER_FAKE_ARGS_FILE\"; fi\n"
    );
    std::fs::write(bin.join("alter"), script).unwrap();
    std::fs::set_permissions(
        bin.join("alter"),
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .unwrap();
    let dmg = out_dir.join(format!("alter-{version}-arm64.dmg"));
    let ok = Command::new("hdiutil")
        .args([
            "create",
            "-quiet",
            "-fs",
            "HFS+",
            "-volname",
            "alter",
            "-srcfolder",
        ])
        .arg(&src)
        .arg(&dmg)
        .status()
        .unwrap()
        .success();
    assert!(ok, "hdiutil create 失败");
    dmg
}
