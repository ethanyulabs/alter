//! CLI 侧的 Gateway 客户端：读 `gateway.json`，用同步 WebSocket 发 JSON-RPC 请求、收通知。
//! 不引入 tokio：CLI 冷启动预算 20 ms。

use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use tungstenite::client::IntoClientRequest;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

/// 连不上 alterd 时的提示（退出码 2）。
pub const NOT_RUNNING: &str = "无法连接 alterd，请先打开 alter.app";

/// 调用失败的原因。
#[derive(Debug)]
pub enum CallError {
    /// 没有 gateway.json，或连不上
    Unreachable(String),
    /// Gateway 返回了 JSON-RPC 错误
    Rpc { code: String, message: String },
}

/// 一条到 Gateway 的连接。
pub struct Session {
    ws: WebSocket<MaybeTlsStream<TcpStream>>,
    next_id: u64,
}

impl Session {
    /// 按 `gateway.json` 连上 Gateway；读超时默认 30 秒。
    pub fn connect(home: &Path) -> Result<Session, CallError> {
        let gw = alter_core::gateway_file::read(home)
            .ok_or_else(|| CallError::Unreachable("没有 gateway.json".to_owned()))?;
        let mut req = format!("ws://127.0.0.1:{}/rpc", gw.port)
            .into_client_request()
            .map_err(|e| CallError::Unreachable(e.to_string()))?;
        req.headers_mut().insert(
            "Authorization",
            format!("Bearer {}", gw.token)
                .parse()
                .map_err(|_| CallError::Unreachable("token 不合法".to_owned()))?,
        );
        let (ws, _) =
            tungstenite::connect(req).map_err(|e| CallError::Unreachable(e.to_string()))?;
        let mut session = Session { ws, next_id: 1 };
        session.set_timeout(Duration::from_secs(30));
        Ok(session)
    }

    /// 之后每次读消息的超时。
    pub fn set_timeout(&mut self, timeout: Duration) {
        if let MaybeTlsStream::Plain(s) = self.ws.get_mut() {
            let _ = s.set_read_timeout(Some(timeout));
        }
    }

    /// 发一个请求并返回 `result`；等待期间收到的通知丢弃。
    pub fn request(&mut self, method: &str, params: Value) -> Result<Value, CallError> {
        let id = self.next_id;
        self.next_id += 1;
        let body = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.ws
            .send(Message::Text(body.to_string().into()))
            .map_err(|e| CallError::Unreachable(e.to_string()))?;
        loop {
            let v = self.read()?;
            if v["id"] != id {
                continue;
            }
            if let Some(err) = v.get("error") {
                return Err(CallError::Rpc {
                    code: err["data"]["code"]
                        .as_str()
                        .unwrap_or("E_INTERNAL")
                        .to_owned(),
                    message: err["data"]["message"].as_str().unwrap_or("").to_owned(),
                });
            }
            return Ok(v["result"].clone());
        }
    }

    /// 下一条通知：(method, params)。
    pub fn notification(&mut self) -> Result<(String, Value), CallError> {
        loop {
            let v = self.read()?;
            if let Some(method) = v["method"].as_str() {
                return Ok((method.to_owned(), v["params"].clone()));
            }
        }
    }

    fn read(&mut self) -> Result<Value, CallError> {
        loop {
            let msg = self
                .ws
                .read()
                .map_err(|e| CallError::Unreachable(e.to_string()))?;
            let Message::Text(text) = msg else { continue };
            return serde_json::from_str(&text).map_err(|e| CallError::Unreachable(e.to_string()));
        }
    }

    pub fn close(mut self) {
        let _ = self.ws.close(None);
    }
}

/// 发一个请求并返回 `result`。
pub fn call(home: &Path, method: &str, params: Value) -> Result<Value, CallError> {
    let mut session = Session::connect(home)?;
    let result = session.request(method, params);
    session.close();
    result
}
