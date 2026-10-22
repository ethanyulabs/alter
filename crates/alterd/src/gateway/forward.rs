//! CDP 转发端点：`cdpPort` 上的每个连接先读 HTTP 请求头校验 Bearer token（与 Gateway 相同），
//! 通过后把请求头原样交给 Chromium 的 remote-debugging 端口，之后双向拷贝字节（HTTP 发现接口与
//! WebSocket 都这样转发）。Chromium 按请求的 `Host` 头生成 `webSocketDebuggerUrl`，原样转发时它
//! 自然指向转发端点。缺少或错误的 token 回 401；Chromium 端口还没就绪时回 503。
//! 转发给的是整个浏览器，没有 TaskSpace 之分：只要有 TaskSpace 正由用户控制，就回 423 拒绝新连接，
//! 转为用户控制的那一刻断开正在转发的连接（`E_USER_CONTROL` 不可绕过）。

use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::Gateway;

/// 请求头的长度上限。
const MAX_HEAD: usize = 64 * 1024;

/// 接管时刻的保证依赖单线程运行时：TabService 把 ownership 写成 user 并发出接管信号是同一个
/// 不让出的回合，期间转发任务不可能运行；转发任务每次恢复都先看接管信号（`biased`），所以接管之后
/// 不会再写出任何字节，已写出的字节都发生在接管之前。换成多线程运行时会破坏这一点，所以在这里断言。
pub(crate) fn assert_single_threaded() {
    assert_eq!(
        tokio::runtime::Handle::current().runtime_flavor(),
        tokio::runtime::RuntimeFlavor::CurrentThread,
        "alterd 必须运行在单线程运行时：接管时刻的保证依赖它"
    );
}

pub async fn serve(listener: TcpListener, gw: Arc<Gateway>) {
    assert_single_threaded();
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                tokio::spawn(connection(stream, gw.clone()));
            }
            Err(e) => tracing::warn!(error = %e, "接受 CDP 转发连接失败"),
        }
    }
}

async fn connection(mut client: TcpStream, gw: Arc<Gateway>) {
    // 先订阅切断信号再检查：检查之后才发生的接管也能断开这条连接
    let mut cut = gw.takeover.subscribe_global();
    let Some(head) = read_head(&mut client).await else {
        return;
    };
    if !authorized(&head, &gw.authorization) {
        respond(&mut client, "401 Unauthorized").await;
        return;
    }
    let user_controlled = gw
        .with_tabs(|t| (t.any_user_controlled(), false))
        .await
        .unwrap_or(true);
    if user_controlled {
        respond(&mut client, "423 Locked").await;
        return;
    }
    let Some(port) = gw.cdp.port() else {
        respond(&mut client, "503 Service Unavailable").await;
        return;
    };
    let mut upstream = match TcpStream::connect(("127.0.0.1", port)).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, port, "连接 Chromium 调试端口失败");
            respond(&mut client, "502 Bad Gateway").await;
            return;
        }
    };
    // 切断信号优先：检查之后才发生的接管，连请求头也不会写给 Chromium
    tokio::select! {
        biased;
        // 有 TaskSpace 转为用户控制：返回即丢弃两端，连接断开
        _ = cut.changed() => {}
        _ = async {
            if upstream.write_all(&head).await.is_ok() {
                let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
            }
        } => {}
    }
}

/// 读到请求头结束（空行）为止，返回读到的全部字节（可能含请求体的开头）；连接提前关闭或头过长时为 None。
async fn read_head(stream: &mut TcpStream) -> Option<Vec<u8>> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 4096];
    loop {
        let n = stream.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            return Some(buf);
        }
        if buf.len() > MAX_HEAD {
            return None;
        }
    }
}

/// 请求头里的 `Authorization` 是否恰好为期望的值（头名不分大小写）。
fn authorized(head: &[u8], expected: &str) -> bool {
    let end = head
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .unwrap_or(head.len());
    let Ok(text) = std::str::from_utf8(&head[..end]) else {
        return false;
    };
    text.split("\r\n").skip(1).any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            name.trim().eq_ignore_ascii_case("authorization") && value.trim() == expected
        })
    })
}

async fn respond(stream: &mut TcpStream, status: &str) {
    let resp = format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    let _ = stream.write_all(resp.as_bytes()).await;
    let _ = stream.shutdown().await;
}

#[cfg(test)]
mod tests {
    use super::{assert_single_threaded, authorized};

    /// alterd 的运行时是单线程的（lifecycle 用 `new_current_thread`）；测试用的运行时同样如此。
    #[tokio::test]
    async fn runs_on_single_threaded_runtime() {
        assert_single_threaded();
    }

    #[test]
    fn authorization_header_must_match_exactly() {
        let ok = b"GET /json/version HTTP/1.1\r\nHost: x\r\nauthorization: Bearer t1\r\n\r\n";
        assert!(authorized(ok, "Bearer t1"));
        assert!(!authorized(ok, "Bearer t2"));
        let none = b"GET /json/version HTTP/1.1\r\nHost: x\r\n\r\n";
        assert!(!authorized(none, "Bearer t1"));
        // 请求行里出现的 token 不算
        let in_path = b"GET /authorization: Bearer t1 HTTP/1.1\r\n\r\n";
        assert!(!authorized(in_path, "Bearer t1"));
    }
}
