//! `alterd run` 的生命周期：建库、写健康文件、等待终止信号、清理退出。

use std::fs::{File, TryLockError};
use std::path::Path;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use alter_core::protocol::{GatewayFile, HealthFile};
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

    let (config, config_warning) = alter_core::config::Config::load(home);
    if let Some(w) = config_warning {
        tracing::warn!(warning = %w, "config.toml 无效，按默认值运行");
    }
    let store = Store::open(home).context("打开数据库失败")?;
    let schema_version: i64 = store
        .conn()
        .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
        .context("读取数据库版本失败")?;
    // 上次退出时被丢下的导入：记为失败，删掉留在 import-tmp 里的源数据副本
    let interrupted = store
        .fail_interrupted_imports(now_millis())
        .context("收尾中断的导入失败")?;
    if interrupted > 0 {
        tracing::warn!(count = interrupted, "上次退出时有导入没做完，已记为失败");
    }
    crate::import::remove_leftover_copies(home);
    // 在 Electron 打开任何 partition 之前清理已删除 Space 的目录
    let space_ids = store.list_space_ids().context("读取 Space 失败")?;
    crate::partitions::sweep(home, &space_ids);

    // 先让 shell.sock 可连，再写健康文件：外部看到 alterd.json 时就能连接
    let cdp = crate::cdp::CdpMux::new(home.to_path_buf());
    let (notices, notice_rx) = tokio::sync::mpsc::unbounded_channel();
    let takeover = crate::takeover::Takeover::new();
    let (requests, request_rx) = tokio::sync::mpsc::unbounded_channel();
    let imports = crate::import::Imports::new(requests.clone(), home.to_path_buf());
    let tab_service = TabService::new(
        store,
        cdp.clone(),
        notices,
        takeover.clone(),
        imports.clone(),
    )?;
    let sock = shell::sock_path(home);
    let listener = shell::bind(&sock).context("监听 shell.sock 失败")?;
    tokio::spawn(tab_service.run(request_rx));
    tokio::spawn(shell::serve(listener, requests.clone()));
    tokio::spawn(crate::archive::run(
        requests.clone(),
        config.archive_after(),
    ));
    tokio::spawn(crate::tabs::inactive_checker(
        requests.clone(),
        config.handoff_timeout(),
    ));

    // Gateway 与 CDP 转发端点：只监听 127.0.0.1 的随机端口，地址与 token 写进 gateway.json。
    // 在 alterd.json 之前写：外部看到健康文件时 Gateway 一定已可连
    let token = new_token().context("生成 Gateway token 失败")?;
    let gw_listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .context("监听 Gateway 端口失败")?;
    let cdp_listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .context("监听 CDP 转发端口失败")?;
    let gateway_file = GatewayFile {
        port: gw_listener.local_addr()?.port(),
        token: token.clone(),
        cdp_port: cdp_listener.local_addr()?.port(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    };
    let gateway = crate::gateway::Gateway::new(
        &token,
        requests,
        cdp.clone(),
        notice_rx,
        config.download_dir(),
        takeover,
        imports,
    );
    tokio::spawn(crate::gateway::serve(gw_listener, gateway.clone()));
    tokio::spawn(crate::gateway::forward::serve(cdp_listener, gateway));
    alter_core::gateway_file::write(home, &gateway_file).context("写 gateway.json 失败")?;
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
    if let Err(e) = alter_core::gateway_file::remove(home) {
        tracing::warn!(error = %e, "删除 gateway.json 失败");
    }
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

/// Gateway token：32 字节随机数的十六进制（64 个字符）。
fn new_token() -> anyhow::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
