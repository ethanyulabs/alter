//! `alterd run` 的生命周期：建库、写健康文件、等待终止信号、清理退出。

use std::fs::{File, TryLockError};
use std::path::Path;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use alter_core::protocol::HealthFile;
use alter_core::store::{Store, db_path};
use anyhow::Context;

use crate::shell;
use crate::tabs::TabService;
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::oneshot;

pub fn run(home: &Path, exit_on_stdin_close: bool) -> anyhow::Result<ExitCode> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("创建 tokio 运行时失败")?;
    runtime.block_on(run_async(home, exit_on_stdin_close))
}

/// 已有实例在运行时的退出码。
const EXIT_ALREADY_RUNNING: u8 = 3;

async fn run_async(home: &Path, exit_on_stdin_close: bool) -> anyhow::Result<ExitCode> {
    // 单实例：整个生命周期持有 alterd.lock 的排他锁，进程退出时由内核释放。
    // 拿到锁之后，任何已存在的健康文件都是残留，直接覆盖。
    std::fs::create_dir_all(home).context("创建数据目录失败")?;
    let lock = File::options()
        .create(true)
        .write(true)
        .truncate(false)
        .open(home.join("alterd.lock"))
        .context("打开实例锁文件失败")?;
    match lock.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => {
            let pid = alter_core::health::read(home)
                .map_or_else(|| "unknown".to_owned(), |h| h.pid.to_string());
            // 与其他退出路径一致，只输出带退出码的 JSON 行日志
            tracing::warn!(
                code = EXIT_ALREADY_RUNNING,
                pid = %pid,
                "alterd: already running (pid {pid})"
            );
            return Ok(ExitCode::from(EXIT_ALREADY_RUNNING));
        }
        Err(TryLockError::Error(e)) => return Err(e).context("获取实例锁失败"),
    }

    // 先装信号处理再写健康文件：外部看到文件时，SIGTERM 一定会走清理路径
    let mut term = signal(SignalKind::terminate()).context("注册 SIGTERM 失败")?;
    let mut int = signal(SignalKind::interrupt()).context("注册 SIGINT 失败")?;
    let stdin_closed = exit_on_stdin_close.then(watch_stdin);

    let store = Store::open(home).context("打开数据库失败")?;
    let schema_version: i64 = store
        .conn()
        .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
        .context("读取数据库版本失败")?;

    // 先让 shell.sock 可连，再写健康文件：外部看到 alterd.json 时就能连接
    let (tab_service, snapshots) = TabService::new(store)?;
    let sock = shell::sock_path(home);
    let listener = shell::bind(&sock).context("监听 shell.sock 失败")?;
    let (requests, request_rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(tab_service.run(request_rx));
    tokio::spawn(shell::serve(listener, snapshots, requests));
    let health = HealthFile {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        pid: std::process::id(),
        started_at: now_millis(),
        db_path: db_path(home).to_string_lossy().into_owned(),
    };
    alter_core::health::write(home, &health).context("写健康文件失败")?;
    tracing::info!(
        pid = health.pid,
        version = %health.version,
        db_path = %health.db_path,
        schema_version,
        "alterd 已启动"
    );

    let signal = tokio::select! {
        _ = term.recv() => "SIGTERM",
        _ = int.recv() => "SIGINT",
        closed = wait_stdin_closed(stdin_closed) => match closed {
            Ok(_) => "stdin-eof",
            Err(e) => {
                tracing::warn!(error = %e, "读取 stdin 出错，按宿主已退出处理");
                "stdin-error"
            }
        },
    };
    tracing::info!(signal, "收到终止信号");

    alter_core::health::remove(home).context("删除健康文件失败")?;
    let _ = std::fs::remove_file(&sock);
    tracing::info!(code = 0, "alterd 已退出");
    drop(lock);
    Ok(ExitCode::SUCCESS)
}

/// 在独立线程里把 stdin 读到 EOF 或读取出错，然后把结果发回主循环。
/// 不用 `tokio::io::stdin`：它在阻塞线程池里读且无法取消，会让运行时关闭一直挂住。
fn watch_stdin() -> oneshot::Receiver<std::io::Result<u64>> {
    let (tx, rx) = oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(std::io::copy(
            &mut std::io::stdin().lock(),
            &mut std::io::sink(),
        ));
    });
    rx
}

/// stdin 读完（`Ok`）或读取出错（`Err`）时完成；没有开启监视时永远不完成。
async fn wait_stdin_closed(
    rx: Option<oneshot::Receiver<std::io::Result<u64>>>,
) -> std::io::Result<u64> {
    match rx {
        // 发送端在发送前不会被丢弃；万一被丢弃，同样视为宿主通道已断
        Some(rx) => rx.await.unwrap_or(Ok(0)),
        None => std::future::pending().await,
    }
}

pub(crate) fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
