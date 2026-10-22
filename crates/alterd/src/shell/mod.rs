//! Shell Control Channel 的服务端：`ALTER_HOME/shell.sock`，每行一个 JSON。
//! 连接建立即发送当前全量快照，之后每次快照变化再发一次；每个连接上的写出顺序与 TabService
//! 处理事件的顺序一致（快照与只回给它的消息走同一条发件箱）。

use std::io;
use std::path::{Path, PathBuf};

use alter_core::protocol::ShellEvent;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;

use crate::tabs::Request;

/// socket 文件路径。
pub fn sock_path(home: &Path) -> PathBuf {
    home.join("shell.sock")
}

/// 删除残留的 socket 文件后监听；socket 文件权限为 0600。
/// 调用方已持有 `alterd.lock`，此时存在的 socket 文件一定是上一次异常退出留下的。
pub fn bind(path: &Path) -> io::Result<UnixListener> {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    // bind 按 umask 创建文件：临时收紧到只有属主可读写，避免 bind 与 chmod 之间的窗口
    // SAFETY：umask 只改本进程的文件创建掩码；此时处于单线程的启动阶段
    let old = unsafe { libc::umask(0o177) };
    let listener = UnixListener::bind(path);
    // SAFETY：同上，恢复原掩码
    unsafe { libc::umask(old) };
    listener
}

/// 接受连接，每个连接一个任务；事件经 `requests` 交给 TabService。
pub async fn serve(listener: UnixListener, requests: mpsc::UnboundedSender<Request>) {
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                tokio::spawn(connection(stream, requests.clone()));
            }
            Err(e) => tracing::warn!(error = %e, "接受 shell 连接失败"),
        }
    }
}

/// 一个连接：向 TabService 登记自己的发件箱（先收到当前快照），之后按顺序写出发件箱里的每一行；
/// 收到的每行解析为事件交给 TabService。对端关闭或写失败即结束。
async fn connection(stream: UnixStream, requests: mpsc::UnboundedSender<Request>) {
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();
    let (outbox, mut out_rx) = mpsc::unbounded_channel::<Arc<str>>();
    if requests.send(Request::Connect(outbox.clone())).is_err() {
        return;
    }
    loop {
        tokio::select! {
            // TabService 已结束：不会再有状态更新，关掉连接让客户端知道
            _ = requests.closed() => return,
            Some(line) = out_rx.recv() => {
                if write_line(&mut write, &line).await.is_err() {
                    return;
                }
            }
            line = lines.next_line() => match line {
                Ok(Some(line)) => match serde_json::from_str::<ShellEvent>(&line) {
                    Ok(event) => {
                        let req = Request::Event { event, outbox: outbox.clone() };
                        if requests.send(req).is_err() {
                            return;
                        }
                    }
                    Err(e) => {
                        let head: String = line.chars().take(200).collect();
                        tracing::warn!(error = %e, line = %head, "无法解析的 shell 消息，已丢弃");
                    }
                },
                Ok(None) | Err(_) => return,
            },
        }
    }
}

async fn write_line<W: AsyncWrite + Unpin>(write: &mut W, line: &str) -> io::Result<()> {
    write.write_all(line.as_bytes()).await?;
    write.write_all(b"\n").await
}
