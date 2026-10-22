//! CdpMux：alterd 直接对 Electron 内 Chromium 发 CDP 的最小客户端。一个浏览器级 WebSocket，
//! flatten 会话：响应按 id 分发，事件（带 sessionId）广播给订阅者。tab ↔ target 由 Main 上报的
//! targetId 决定（N2 T1 结论），会话按 targetId 缓存。

pub mod overlay;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::sync::{Mutex, broadcast, mpsc, oneshot, watch};
use tokio_tungstenite::tungstenite::Message;

/// CDP 调用失败。
#[derive(Debug, Clone)]
pub enum CdpError {
    /// 还没连上 Chromium（没收到 cdpReady 或连接已断）
    NotConnected,
    /// Chromium 返回了错误
    Protocol(String),
    /// 命令带的接管信号在写出之前变了（有 TaskSpace 被用户接管），没有发给 Chromium
    Interrupted,
}

impl std::fmt::Display for CdpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CdpError::NotConnected => write!(f, "还没连上 Chromium 的调试端口"),
            CdpError::Protocol(m) => write!(f, "CDP 错误：{m}"),
            CdpError::Interrupted => write!(f, "有 TaskSpace 被用户接管，命令没有发出"),
        }
    }
}

/// 一条 CDP 事件。
#[derive(Debug, Clone)]
pub struct CdpEvent {
    pub method: String,
    pub params: Value,
    pub session_id: Option<String>,
}

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, CdpError>>>>>;

tokio::task_local! {
    /// 当前 agent 调用的接管信号：`send` 没有显式给出时用它，让一次调用产生的全部 CDP 命令
    /// （包括建立页面会话的初始化命令）都在写出之前检查接管
    pub static GUARD: watch::Receiver<u64>;
}

/// 写出队列的一项。带 `guard` 的命令在真正写出之前检查它：入队之后信号变了就丢弃，不发给 Chromium。
struct Outgoing {
    id: u64,
    text: String,
    guard: Option<watch::Receiver<u64>>,
}

/// 一条到 Chromium 的连接。
struct Conn {
    out: mpsc::UnboundedSender<Outgoing>,
    pending: Pending,
    next_id: AtomicU64,
}

/// `send` 结束或被丢弃时从 pending 里移除自己的 id（已收到响应时 id 早已不在，移除是空操作）。
struct ForgetOnDrop {
    pending: Pending,
    id: u64,
}

impl Drop for ForgetOnDrop {
    fn drop(&mut self) {
        let id = self.id;
        // Drop 里不能等异步锁：拿得到就当场移除，拿不到交给一个任务
        if let Ok(mut map) = self.pending.try_lock() {
            map.remove(&id);
            return;
        }
        let pending = self.pending.clone();
        tokio::spawn(async move {
            pending.lock().await.remove(&id);
        });
    }
}

pub struct CdpMux {
    home: PathBuf,
    conn: Mutex<Option<Arc<Conn>>>,
    /// targetId → sessionId
    sessions: Mutex<HashMap<String, String>>,
    events: broadcast::Sender<CdpEvent>,
    /// Chromium 的 remote-debugging 端口（最近一次 connect 的），CDP 转发端点用
    port: std::sync::Mutex<Option<u16>>,
}

impl CdpMux {
    pub fn new(home: PathBuf) -> Arc<CdpMux> {
        let (events, _) = broadcast::channel(1024);
        Arc::new(CdpMux {
            home,
            conn: Mutex::new(None),
            sessions: Mutex::new(HashMap::new()),
            events,
            port: std::sync::Mutex::new(None),
        })
    }

    /// 订阅事件（先订阅再发命令，避免漏掉）。
    /// 会话对应的 targetId。
    pub async fn target_of(&self, session_id: &str) -> Option<String> {
        self.sessions
            .lock()
            .await
            .iter()
            .find(|(_, s)| s.as_str() == session_id)
            .map(|(t, _)| t.clone())
    }

    pub fn subscribe(&self) -> broadcast::Receiver<CdpEvent> {
        self.events.subscribe()
    }

    /// 连接 `127.0.0.1:port` 的浏览器端点；路径取 `DevToolsActivePort` 的第二行。已有连接时替换它。
    pub async fn connect(self: Arc<Self>, port: u16) -> anyhow::Result<()> {
        *self.port.lock().unwrap_or_else(|e| e.into_inner()) = Some(port);
        let file = self.home.join("chromium").join("DevToolsActivePort");
        // 文件很小，同步读即可
        let text = std::fs::read_to_string(&file)?;
        let path = text
            .lines()
            .nth(1)
            .filter(|l| l.starts_with("/devtools/browser/"))
            .ok_or_else(|| anyhow::anyhow!("DevToolsActivePort 没有浏览器端点"))?;
        let url = format!("ws://127.0.0.1:{port}{path}");
        let (ws, _) = tokio_tungstenite::connect_async(url.as_str()).await?;
        let (mut sink, mut source) = ws.split();
        let (out, mut out_rx) = mpsc::unbounded_channel::<Outgoing>();
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let conn = Arc::new(Conn {
            out,
            pending: pending.clone(),
            next_id: AtomicU64::new(1),
        });
        let writer_pending = pending.clone();
        tokio::spawn(async move {
            while let Some(o) = out_rx.recv().await {
                // 写出任务可能因背压晚于入队很久才轮到这条：此时再看一次接管信号。
                // 检查通过即开始写出，这一刻就是命令的发出时刻（见 N3 design.md「接管边界的定义」）：
                // 之后哪怕 `sink.send` 因背压在接管后才写完，也算接管前发出的命令
                if o.guard
                    .as_ref()
                    .is_some_and(|g| g.has_changed().unwrap_or(true))
                {
                    if let Some(tx) = writer_pending.lock().await.remove(&o.id) {
                        let _ = tx.send(Err(CdpError::Interrupted));
                    }
                    continue;
                }
                if sink.send(Message::Text(o.text.into())).await.is_err() {
                    break;
                }
            }
        });
        let mux = self.clone();
        let reader_conn = conn.clone();
        tokio::spawn(async move {
            while let Some(Ok(msg)) = source.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                if let Some(id) = v.get("id").and_then(Value::as_u64) {
                    let result = match v.get("error") {
                        Some(e) => Err(CdpError::Protocol(
                            e["message"].as_str().unwrap_or("未知错误").to_owned(),
                        )),
                        None => Ok(v.get("result").cloned().unwrap_or(Value::Null)),
                    };
                    if let Some(tx) = pending.lock().await.remove(&id) {
                        let _ = tx.send(result);
                    }
                    continue;
                }
                let event = CdpEvent {
                    method: v["method"].as_str().unwrap_or_default().to_owned(),
                    params: v.get("params").cloned().unwrap_or(Value::Null),
                    session_id: v["sessionId"].as_str().map(str::to_owned),
                };
                if event.method == "Target.detachedFromTarget"
                    && let Some(sid) = event.params["sessionId"].as_str()
                {
                    mux.sessions.lock().await.retain(|_, s| s != sid);
                }
                let _ = mux.events.send(event);
            }
            // 连接断了：等着响应的调用都失败，清掉会话与连接
            for (_, tx) in pending.lock().await.drain() {
                let _ = tx.send(Err(CdpError::NotConnected));
            }
            mux.sessions.lock().await.clear();
            let mut slot = mux.conn.lock().await;
            if slot.as_ref().is_some_and(|c| Arc::ptr_eq(c, &reader_conn)) {
                *slot = None;
            }
        });
        self.sessions.lock().await.clear();
        *self.conn.lock().await = Some(conn);
        tracing::info!(port, "已连接 Chromium 的调试端口");
        Ok(())
    }

    /// 发一个 CDP 命令；`session` 为空时发到浏览器级。
    pub async fn send(
        &self,
        session: Option<&str>,
        method: &str,
        params: Value,
    ) -> Result<Value, CdpError> {
        self.send_inner(session, method, params, None).await
    }

    /// 同 `send`，但 `guard` 在命令真正写出之前变了（相对于调用方订阅时）就不发，返回 `Interrupted`。
    pub async fn send_guarded(
        &self,
        session: Option<&str>,
        method: &str,
        params: Value,
        guard: watch::Receiver<u64>,
    ) -> Result<Value, CdpError> {
        self.send_inner(session, method, params, Some(guard)).await
    }

    async fn send_inner(
        &self,
        session: Option<&str>,
        method: &str,
        params: Value,
        guard: Option<watch::Receiver<u64>>,
    ) -> Result<Value, CdpError> {
        let guard = guard.or_else(|| GUARD.try_with(|g| g.clone()).ok());
        let conn = self
            .conn
            .lock()
            .await
            .clone()
            .ok_or(CdpError::NotConnected)?;
        let id = conn.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        conn.pending.lock().await.insert(id, tx);
        // 调用方放弃等待（例如被外层超时丢弃）时，移除这条待响应请求
        let _forget = ForgetOnDrop {
            pending: conn.pending.clone(),
            id,
        };
        let mut msg = json!({"id": id, "method": method, "params": params});
        if let Some(s) = session {
            msg["sessionId"] = json!(s);
        }
        let outgoing = Outgoing {
            id,
            text: msg.to_string(),
            guard,
        };
        if conn.out.send(outgoing).is_err() {
            conn.pending.lock().await.remove(&id);
            return Err(CdpError::NotConnected);
        }
        rx.await.unwrap_or(Err(CdpError::NotConnected))
    }

    /// 某个 target 的会话：没有就 attach（flatten），并开 Page / Runtime 域。
    pub async fn session(&self, target_id: &str) -> Result<String, CdpError> {
        if let Some(s) = self.sessions.lock().await.get(target_id) {
            return Ok(s.clone());
        }
        let r = self
            .send(
                None,
                "Target.attachToTarget",
                json!({"targetId": target_id, "flatten": true}),
            )
            .await?;
        let sid = r["sessionId"]
            .as_str()
            .ok_or_else(|| CdpError::Protocol("attachToTarget 没有 sessionId".to_owned()))?
            .to_owned();
        for method in ["Page.enable", "Runtime.enable"] {
            self.send(Some(&sid), method, json!({})).await?;
        }
        // 生命周期事件带 loaderId：导航后等的 load 一定属于这次导航
        self.send(
            Some(&sid),
            "Page.setLifecycleEventsEnabled",
            json!({"enabled": true}),
        )
        .await?;
        self.sessions
            .lock()
            .await
            .insert(target_id.to_owned(), sid.clone());
        Ok(sid)
    }

    /// 是否已连上。
    /// Chromium 的 remote-debugging 端口；还没收到 cdpReady 时为 None。
    pub fn port(&self) -> Option<u16> {
        *self.port.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub async fn connected(&self) -> bool {
        self.conn.lock().await.is_some()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures_util::StreamExt;

    use super::*;

    /// 调用方放弃等待（外层超时丢弃了 send）后，这条请求不再留在 pending 里。
    #[tokio::test]
    async fn abandoned_send_is_forgotten() {
        // 只收不回的假 CDP 端
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            while ws.next().await.is_some() {}
        });
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join("chromium")).unwrap();
        std::fs::write(
            home.path().join("chromium/DevToolsActivePort"),
            format!("{port}\n/devtools/browser/x\n"),
        )
        .unwrap();
        let mux = CdpMux::new(home.path().to_path_buf());
        mux.clone().connect(port).await.unwrap();

        let r = tokio::time::timeout(
            Duration::from_millis(50),
            mux.send(None, "Runtime.evaluate", json!({})),
        )
        .await;
        assert!(r.is_err(), "假端不回，应当超时");
        tokio::task::yield_now().await;
        let conn = mux.conn.lock().await.clone().unwrap();
        assert!(conn.pending.lock().await.is_empty());
    }

    /// 带接管信号的命令：入队之后、写出之前信号变了（接管），写出任务丢弃它，调用方得到 Interrupted；
    /// 同一队列里后面的普通命令照常写出。
    #[tokio::test]
    async fn guarded_send_is_dropped_after_takeover() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let port = listener.local_addr().unwrap().port();
        let (seen_tx, mut seen) = tokio::sync::mpsc::unbounded_channel::<String>();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            while let Some(Ok(msg)) = ws.next().await {
                if let Message::Text(t) = msg {
                    let v: Value = serde_json::from_str(&t).unwrap();
                    let _ = seen_tx.send(v["method"].as_str().unwrap_or("").to_owned());
                }
            }
        });
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join("chromium")).unwrap();
        std::fs::write(
            home.path().join("chromium/DevToolsActivePort"),
            format!("{port}\n/devtools/browser/x\n"),
        )
        .unwrap();
        let mux = CdpMux::new(home.path().to_path_buf());
        mux.clone().connect(port).await.unwrap();

        let (takeover, guard) = watch::channel(0u64);
        // 订阅之后发生了接管
        takeover.send_modify(|n| *n += 1);
        // 假端从不回应：命令要是被写出，这里只会等到超时
        let r = tokio::time::timeout(
            Duration::from_secs(1),
            mux.send_guarded(None, "Target.closeTarget", json!({}), guard),
        )
        .await;
        assert!(matches!(r, Ok(Err(CdpError::Interrupted))), "{r:?}");
        let _ = tokio::time::timeout(
            Duration::from_millis(100),
            mux.send(None, "Browser.getVersion", json!({})),
        )
        .await;
        let first = tokio::time::timeout(Duration::from_secs(1), seen.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(first, "Browser.getVersion", "被丢弃的命令不应写出");
    }

    /// 在 `GUARD` 作用域里的普通 `send`（例如建立页面会话的初始化命令）也带上接管信号：接管后不写出。
    #[tokio::test]
    async fn scoped_guard_applies_to_plain_send() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let port = listener.local_addr().unwrap().port();
        let (seen_tx, mut seen) = tokio::sync::mpsc::unbounded_channel::<String>();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            while let Some(Ok(msg)) = ws.next().await {
                if let Message::Text(t) = msg {
                    let v: Value = serde_json::from_str(&t).unwrap();
                    let _ = seen_tx.send(v["method"].as_str().unwrap_or("").to_owned());
                }
            }
        });
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join("chromium")).unwrap();
        std::fs::write(
            home.path().join("chromium/DevToolsActivePort"),
            format!("{port}\n/devtools/browser/x\n"),
        )
        .unwrap();
        let mux = CdpMux::new(home.path().to_path_buf());
        mux.clone().connect(port).await.unwrap();

        let (takeover, guard) = watch::channel(0u64);
        takeover.send_modify(|n| *n += 1);
        let r = tokio::time::timeout(
            Duration::from_secs(1),
            GUARD.scope(guard, mux.send(None, "Target.attachToTarget", json!({}))),
        )
        .await;
        assert!(matches!(r, Ok(Err(CdpError::Interrupted))), "{r:?}");
        let _ = tokio::time::timeout(
            Duration::from_millis(100),
            mux.send(None, "Browser.getVersion", json!({})),
        )
        .await;
        let first = tokio::time::timeout(Duration::from_secs(1), seen.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(first, "Browser.getVersion", "作用域里的命令不应写出");
    }

    /// N3 CR13：守卫命令已入队、写出任务正因背压卡在前一条上时用户接管——轮到它时写出任务丢弃它，
    /// 调用方得到 Interrupted。（检查若挪到入队之前、写出任务不再看信号，它会在接管之后被写出。）
    #[tokio::test]
    async fn queued_guarded_send_is_dropped_when_takeover_happens_during_backpressure() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let port = listener.local_addr().unwrap().port();
        let (seen_tx, mut seen) = tokio::sync::mpsc::unbounded_channel::<String>();
        let (start_reading, can_read) = tokio::sync::oneshot::channel::<()>();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            // 握手之后先不读：大命令写满两端的缓冲区，写出任务卡在 sink.send
            let _ = can_read.await;
            while let Some(Ok(msg)) = ws.next().await {
                if let Message::Text(t) = msg {
                    let v: Value = serde_json::from_str(&t).unwrap();
                    let _ = seen_tx.send(v["method"].as_str().unwrap_or("").to_owned());
                }
            }
        });
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join("chromium")).unwrap();
        std::fs::write(
            home.path().join("chromium/DevToolsActivePort"),
            format!("{port}\n/devtools/browser/x\n"),
        )
        .unwrap();
        let mux = CdpMux::new(home.path().to_path_buf());
        mux.clone().connect(port).await.unwrap();

        // 12 MB 的普通命令（低于 tungstenite 默认 16 MB 的帧上限）
        let big = "x".repeat(12 * 1024 * 1024);
        let blocker = {
            let mux = mux.clone();
            tokio::spawn(async move {
                let _ = mux
                    .send(None, "Runtime.evaluate", json!({"expression": big}))
                    .await;
            })
        };
        tokio::time::sleep(Duration::from_millis(200)).await;
        let (takeover, guard) = watch::channel(0u64);
        let guarded = {
            let mux = mux.clone();
            tokio::spawn(async move {
                mux.send_guarded(None, "Target.closeTarget", json!({}), guard)
                    .await
            })
        };
        // 等它确实入队（入队时信号还没变）：`send_inner` 先登记 pending 再入队，两步之间没有让出点，
        // 单线程运行时下看到 pending 里有两条（大命令与它），它就已经在写出队列里了
        let conn = mux.conn.lock().await.clone().unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            while conn.pending.lock().await.len() < 2 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("守卫命令应当在 2 秒内入队");
        takeover.send_modify(|n| *n += 1);
        let _ = start_reading.send(());

        let r = tokio::time::timeout(Duration::from_secs(5), guarded)
            .await
            .expect("守卫命令应当在 5 秒内结束")
            .unwrap();
        assert!(matches!(r, Err(CdpError::Interrupted)), "{r:?}");
        // 之后的标记命令照常写出；在它之前到达的只能是大命令
        let marker = {
            let mux = mux.clone();
            tokio::spawn(async move {
                let _ = tokio::time::timeout(
                    Duration::from_millis(500),
                    mux.send(None, "Browser.getVersion", json!({})),
                )
                .await;
            })
        };
        let mut methods = Vec::new();
        while let Ok(Some(m)) = tokio::time::timeout(Duration::from_secs(5), seen.recv()).await {
            let done = m == "Browser.getVersion";
            methods.push(m);
            if done {
                break;
            }
        }
        assert_eq!(
            methods,
            vec![
                "Runtime.evaluate".to_owned(),
                "Browser.getVersion".to_owned()
            ],
            "接管后不应写出守卫命令"
        );
        blocker.abort();
        marker.abort();
    }
}
