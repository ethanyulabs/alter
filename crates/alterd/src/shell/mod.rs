//! Shell Control Channel 的服务端：`ALTER_HOME/shell.sock`，每行一个 JSON。
//! 连接建立即发送当前全量快照，之后每次快照变化再发一次。

use std::io;
use std::path::{Path, PathBuf};

use alter_core::protocol::{ShellCommand, ShellEvent};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::OwnedWriteHalf;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;

use crate::tabs::{Request, Snapshots};

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
pub async fn serve(
    listener: UnixListener,
    snapshots: Snapshots,
    requests: mpsc::UnboundedSender<Request>,
) {
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                tokio::spawn(connection(stream, snapshots.clone(), requests.clone()));
            }
            Err(e) => tracing::warn!(error = %e, "接受 shell 连接失败"),
        }
    }
}

/// 一个连接：先发当前快照；之后每次状态变化发一份新快照，收到的每行解析为事件交给 TabService，
/// 只回给本连接的消息（错误、导航指令）经私有通道写出。对端关闭或写失败即结束。
async fn connection(
    stream: UnixStream,
    snapshots: Snapshots,
    requests: mpsc::UnboundedSender<Request>,
) {
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();
    let (reply_tx, mut reply_rx) = mpsc::unbounded_channel::<ShellCommand>();

    let (first, mut updates) = snapshots.subscribe();
    if write_line(&mut write, &first).await.is_err() {
        return;
    }
    loop {
        tokio::select! {
            update = updates.recv() => match update {
                Ok(line) => {
                    if write_line(&mut write, &line).await.is_err() {
                        return;
                    }
                }
                // 落后太多：最旧的几份被丢弃，缓冲里更新的快照仍会依次送达
                Err(RecvError::Lagged(skipped)) => {
                    tracing::warn!(skipped, "shell 连接落后，跳过了部分快照");
                }
                Err(RecvError::Closed) => return,
            },
            Some(cmd) = reply_rx.recv() => {
                let Ok(line) = serde_json::to_string(&cmd) else { continue };
                if write_line(&mut write, &line).await.is_err() {
                    return;
                }
            }
            line = lines.next_line() => match line {
                Ok(Some(line)) => match serde_json::from_str::<ShellEvent>(&line) {
                    Ok(event) => {
                        let req = Request { event, reply: reply_tx.clone() };
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

async fn write_line(write: &mut OwnedWriteHalf, line: &str) -> io::Result<()> {
    write.write_all(line.as_bytes()).await?;
    write.write_all(b"\n").await
}
