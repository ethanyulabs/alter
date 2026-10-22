//! 假 CDP 端：本地 WebSocket 服务代替 Chromium，经 `DevToolsActivePort` 与 `cdpReady` 交给 alterd。
//! 对每条命令立即应答（可按方法覆盖结果），并把收到的命令连同时间点送进通道，供测试断言。

use std::net::TcpListener;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::{Client, Ws, expect_snapshot, rpc};

/// 按方法给出结果；返回 None 时用默认结果。
pub type Responder = Box<dyn Fn(&str, &Value) -> Option<Value> + Send>;

pub struct FakeCdp {
    /// 收到的每条命令（完整请求）与收到的时间
    pub commands: mpsc::Receiver<(Instant, Value)>,
}

impl FakeCdp {
    /// 起假 CDP 端并让 alterd 连上它。
    pub fn start(home: &Path, shell: &mut Client) -> FakeCdp {
        FakeCdp::start_with(home, shell, Box::new(|_, _| None))
    }

    pub fn start_with(home: &Path, shell: &mut Client, responder: Responder) -> FakeCdp {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (tx, commands) = mpsc::channel();
        std::thread::spawn(move || serve(listener, tx, responder));
        let chromium = home.join("chromium");
        std::fs::create_dir_all(&chromium).unwrap();
        std::fs::write(
            chromium.join("DevToolsActivePort"),
            format!("{port}\n/devtools/browser/fake\n"),
        )
        .unwrap();
        shell.send(json!({"type": "shell.event.cdpReady", "port": port}));
        FakeCdp { commands }
    }

    /// 取出到目前为止收到的命令（不等待）。
    pub fn drain(&self) -> Vec<Value> {
        self.commands.try_iter().map(|(_, c)| c).collect()
    }

    /// 等到收到某个方法的命令（跳过其他的），返回它；`timeout` 内没有即失败。
    pub fn expect(&self, method: &str, timeout: Duration) -> Value {
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let (_, cmd) = self
                .commands
                .recv_timeout(left)
                .unwrap_or_else(|_| panic!("{timeout:?} 内没有收到 {method}"));
            if cmd["method"] == method {
                return cmd;
            }
        }
    }
}

fn serve(listener: TcpListener, tx: mpsc::Sender<(Instant, Value)>, responder: Responder) {
    let (stream, _) = listener.accept().unwrap();
    let mut ws = tungstenite::accept(stream).unwrap();
    loop {
        let Ok(msg) = ws.read() else { return };
        let tungstenite::Message::Text(text) = msg else {
            continue;
        };
        let req: Value = serde_json::from_str(&text).unwrap();
        let _ = tx.send((Instant::now(), req.clone()));
        let method = req["method"].as_str().unwrap_or("");
        let result = responder(method, &req).unwrap_or_else(|| default_result(method));
        let mut resp = json!({"id": req["id"], "result": result});
        if let Some(sid) = req.get("sessionId") {
            resp["sessionId"] = sid.clone();
        }
        if ws
            .send(tungstenite::Message::Text(resp.to_string().into()))
            .is_err()
        {
            return;
        }
    }
}

/// 默认结果：页面里恰好有一个可见元素，导航返回 loaderId 但不会再有生命周期事件。
fn default_result(method: &str) -> Value {
    match method {
        "Target.attachToTarget" => json!({"sessionId": "S1"}),
        "Target.getTargetInfo" => json!({"targetInfo": {"browserContextId": "C1"}}),
        "Page.navigate" => json!({"frameId": "T1", "loaderId": "L1"}),
        // 选择器求值：返回匹配元素的数组对象
        "Runtime.evaluate" => json!({"result": {"type": "object", "objectId": "arr"}}),
        "Runtime.getProperties" => {
            json!({"result": [{"name": "0", "value": {"type": "object", "objectId": "el"}}]})
        }
        "DOM.getContentQuads" => json!({"quads": [[0, 0, 10, 0, 10, 10, 0, 10]]}),
        _ => json!({}),
    }
}

/// 建一个 TaskSpace，并替它的 `p1` 上报 targetId `T1`（真实环境由 Main 上报）；返回 TaskSpace id。
pub fn agent_page(shell: &mut Client, ws: &mut Ws) -> i64 {
    let created = rpc(ws, 1, "taskSpace.create", json!({"name": "fake"}));
    let id = created["result"]["id"].as_i64().unwrap();
    let tab = loop {
        let s = expect_snapshot(shell);
        let found = s["tabs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["label"] == "p1")
            .map(|t| t["id"].clone());
        if let Some(tab) = found {
            break tab;
        }
    };
    shell.send(json!({"type": "shell.event.tabUpdated", "tabId": tab, "targetId": "T1"}));
    id
}
