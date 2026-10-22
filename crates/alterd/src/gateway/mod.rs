//! Gateway：127.0.0.1 上的 WebSocket JSON-RPC，供 SDK 与 CLI 调用。握手时校验 Bearer token，
//! 不通过回 401；连接内不再校验。每条请求一个任务，响应经该连接的写出队列发回。

mod actions;
mod events;
pub mod forward;
mod imports;
mod page;
mod rpc;

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;

use crate::tabs;

/// Gateway 处理请求需要的共享状态。
pub struct Gateway {
    /// 握手时要求的 `Authorization` 头的完整值：`Bearer <token>`
    authorization: String,
    pub(crate) tabs: mpsc::UnboundedSender<tabs::Request>,
    pub(crate) cdp: Arc<crate::cdp::CdpMux>,
    /// 正开着 JS 对话框的会话：sessionId → 对话框（由 CDP 事件维护）
    pub(crate) dialogs:
        Arc<std::sync::Mutex<std::collections::HashMap<String, alter_core::protocol::DialogInfo>>>,
    /// 每个页面最近一次 snapshot 的 `@e<n>` 引用，键为 (TaskSpace, Page 标签)
    pub(crate) refs: std::sync::Mutex<std::collections::HashMap<(i64, String), actions::Refs>>,
    /// 连着的客户端：页面事件按它们关注的 TaskSpace 推送
    listeners: std::sync::Mutex<Vec<events::Listener>>,
    /// agent 页面的下载目录
    download_dir: std::path::PathBuf,
    /// 已设置过下载行为的浏览器上下文（每个 Space 的 partition 一个）
    download_contexts: tokio::sync::Mutex<std::collections::HashSet<String>>,
    /// 接管信号（与 TabService 共享，由它在写 ownership 的同一步递增）
    pub(crate) takeover: Arc<crate::takeover::Takeover>,
    /// 每个 TaskSpace 一把锁：同一 TaskSpace 的页面动作串行执行
    queues: std::sync::Mutex<std::collections::HashMap<i64, Arc<tokio::sync::Mutex<()>>>>,
    /// 导入服务（与 Shell 入口共用）
    pub(crate) imports: Arc<crate::import::Imports>,
}

impl Gateway {
    /// 在 TabService 的循环里执行 `job`（它是数据库唯一写者），并等结果；`job` 返回的布尔值
    /// 表示状态是否变化，变化了 TabService 会发布新快照。
    pub(crate) async fn with_tabs<T: Send + 'static>(
        &self,
        job: impl FnOnce(&mut tabs::TabService) -> (T, bool) + Send + 'static,
    ) -> Result<T, rpc::RpcError> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let boxed: tabs::GatewayJob = Box::new(move |tabs| {
            let (value, changed) = job(tabs);
            let _ = tx.send(value);
            changed
        });
        self.tabs
            .send(tabs::Request::Gateway(boxed))
            .map_err(|_| rpc::RpcError::internal("TabService 已停止"))?;
        rx.await
            .map_err(|_| rpc::RpcError::internal("TabService 已停止"))
    }

    /// 在 TabService 里执行一个 TaskSpace 操作：成功时按它的返回决定是否发布快照。
    pub(crate) async fn op<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut tabs::TabService) -> Result<(T, bool), tabs::OpError> + Send + 'static,
    ) -> Result<T, rpc::RpcError> {
        self.with_tabs(move |t| match f(t) {
            Ok((value, changed)) => (Ok(value), changed),
            Err(e) => (Err(e), false),
        })
        .await?
        .map_err(rpc::RpcError::from)
    }

    pub fn new(
        token: &str,
        tabs: mpsc::UnboundedSender<tabs::Request>,
        cdp: Arc<crate::cdp::CdpMux>,
        notices: mpsc::UnboundedReceiver<tabs::Notice>,
        download_dir: std::path::PathBuf,
        takeover: Arc<crate::takeover::Takeover>,
        imports: Arc<crate::import::Imports>,
    ) -> Arc<Gateway> {
        let gw = Arc::new(Gateway {
            authorization: format!("Bearer {token}"),
            tabs,
            cdp,
            dialogs: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            refs: std::sync::Mutex::new(std::collections::HashMap::new()),
            listeners: std::sync::Mutex::new(Vec::new()),
            download_dir,
            takeover,
            download_contexts: tokio::sync::Mutex::new(std::collections::HashSet::new()),
            queues: std::sync::Mutex::new(std::collections::HashMap::new()),
            imports,
        });
        tokio::spawn(events::dispatch(gw.clone(), notices));
        tokio::spawn(events::track_dialogs(gw.clone()));
        tokio::spawn(events::track_downloads(gw.clone()));
        gw
    }

    /// 该 TaskSpace 的串行队列。
    pub(crate) fn queue(&self, task_space_id: i64) -> Arc<tokio::sync::Mutex<()>> {
        self.queues
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(task_space_id)
            .or_default()
            .clone()
    }
}

/// 接受连接，每个连接一个任务。
pub async fn serve(listener: TcpListener, gateway: Arc<Gateway>) {
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                tokio::spawn(connection(stream, gateway.clone()));
            }
            Err(e) => tracing::warn!(error = %e, "接受 Gateway 连接失败"),
        }
    }
}

async fn connection(stream: TcpStream, gateway: Arc<Gateway>) {
    let expected = gateway.authorization.clone();
    // 回调签名由 tungstenite 规定，错误类型就是一个完整的 HTTP 响应
    #[allow(clippy::result_large_err)]
    let check = move |req: &Request, resp: Response| -> Result<Response, ErrorResponse> {
        let token_ok = req
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v == expected);
        if req.uri().path() == "/rpc" && token_ok {
            Ok(resp)
        } else {
            let mut err = ErrorResponse::new(Some("unauthorized".to_owned()));
            *err.status_mut() = StatusCode::UNAUTHORIZED;
            Err(err)
        }
    };
    let ws = match tokio_tungstenite::accept_hdr_async(stream, check).await {
        Ok(ws) => ws,
        Err(e) => {
            tracing::debug!(error = %e, "Gateway 握手未完成");
            return;
        }
    };
    let (mut sink, mut source) = ws.split();
    // 写出队列：请求任务把响应放进来，由这个任务依次写出
    let (out, mut out_rx) = mpsc::unbounded_channel::<String>();
    // 登记这个连接：它请求过的 TaskSpace 的页面事件推给它
    let interest = Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));
    gateway
        .listeners
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(events::Listener {
            interest: interest.clone(),
            out: out.clone(),
        });
    let writer = tokio::spawn(async move {
        while let Some(text) = out_rx.recv().await {
            if sink.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    });
    while let Some(msg) = source.next().await {
        let text = match msg {
            Ok(Message::Text(text)) => text.to_string(),
            Ok(Message::Close(_)) | Err(_) => break,
            Ok(_) => continue,
        };
        let gateway = gateway.clone();
        let out = out.clone();
        let interest = interest.clone();
        tokio::spawn(async move {
            let req: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
            if let Some(id) = req["params"]["id"].as_i64() {
                interest
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(id);
            }
            if let Some(resp) = rpc::handle(&gateway, &text, &out).await {
                // 新建 / 恢复的 TaskSpace 的 id 在响应里
                if req["method"]
                    .as_str()
                    .is_some_and(|m| m.starts_with("taskSpace."))
                    && let Ok(v) = serde_json::from_str::<serde_json::Value>(&resp)
                    && let Some(id) = v["result"]["id"].as_i64()
                {
                    interest
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .insert(id);
                }
                let _ = out.send(resp);
            }
        });
    }
    // 移除自己的 listener：它持有写队列的发送端，不移除时写出任务等不到队列关闭
    gateway
        .listeners
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|l| !Arc::ptr_eq(&l.interest, &interest));
    drop(out);
    let _ = writer.await;
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    use super::*;

    /// 客户端断开后，连接任务结束，它登记的 listener 被移除（不留下写队列与任务）。
    #[tokio::test]
    async fn connection_ends_and_unregisters_after_client_closes() {
        let (tabs, _tabs_rx) = mpsc::unbounded_channel();
        let (_notices, notices_rx) = mpsc::unbounded_channel();
        let cdp = crate::cdp::CdpMux::new(std::path::PathBuf::from("/nonexistent"));
        let imports =
            crate::import::Imports::new(tabs.clone(), std::path::PathBuf::from("/nonexistent"));
        let gw = Gateway::new(
            "tok",
            tabs,
            cdp,
            notices_rx,
            std::path::PathBuf::from("/nonexistent"),
            crate::takeover::Takeover::new(),
            imports,
        );

        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = {
            let gw = gw.clone();
            tokio::spawn(async move {
                let (stream, _) = listener.accept().await.unwrap();
                connection(stream, gw).await;
            })
        };

        let mut req = format!("ws://{addr}/rpc").into_client_request().unwrap();
        req.headers_mut()
            .insert("authorization", "Bearer tok".parse().unwrap());
        let stream = TcpStream::connect(addr).await.unwrap();
        let (mut ws, _) = tokio_tungstenite::client_async(req, stream).await.unwrap();
        // 握手完成后连接已登记
        tokio::time::timeout(Duration::from_secs(1), async {
            while gw.listeners.lock().unwrap().is_empty() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("握手后应当登记 listener");
        ws.close(None).await.unwrap();
        drop(ws);

        tokio::time::timeout(Duration::from_secs(1), server)
            .await
            .expect("客户端断开后连接任务应当结束")
            .unwrap();
        assert!(gw.listeners.lock().unwrap().is_empty());
    }
}
