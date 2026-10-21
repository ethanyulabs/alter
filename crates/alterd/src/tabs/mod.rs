//! TabService：Space 与标签状态的唯一来源。独占 `Store`，处理 Electron 上报的事件，
//! 生成 Shell Control 的全量快照。

mod url;

use std::sync::Arc;

use alter_core::protocol::{ErrorCode, ShellCommand, ShellEvent, ShellState, SpaceView, TabView};
use alter_core::store::Store;
use anyhow::Context;
use tokio::sync::{broadcast, mpsc, watch};

use crate::lifecycle::now_millis;

/// 最新快照（已序列化的 `shell.stateChanged` 一行，不含换行）。
pub type Snapshot = Arc<str>;

/// 每次状态变化都广播一份快照；广播缓冲的条数（慢连接落后更多时会跳过最旧的几份）。
const UPDATES_CAPACITY: usize = 256;

/// 连接用来拿快照的句柄：当前快照（新连接的第一条）与之后每次变化的广播。
#[derive(Clone)]
pub struct Snapshots {
    latest: watch::Receiver<Snapshot>,
    updates: broadcast::Sender<Snapshot>,
}

impl Snapshots {
    /// 先订阅广播再取当前快照：两者之间发生的变化不会漏掉（最多重复收到一份全量快照）。
    pub fn subscribe(&self) -> (Snapshot, broadcast::Receiver<Snapshot>) {
        let updates = self.updates.subscribe();
        let current = self.latest.borrow().clone();
        (current, updates)
    }
}

pub struct TabService {
    store: Store,
    latest: watch::Sender<Snapshot>,
    updates: broadcast::Sender<Snapshot>,
}

/// 一个连接上报的事件，`reply` 用于只回给该连接的消息（错误、导航指令）。
pub struct Request {
    pub event: ShellEvent,
    pub reply: mpsc::UnboundedSender<ShellCommand>,
}

/// 事件被拒绝的原因，回给发送方。
struct Rejection {
    code: ErrorCode,
    message: String,
}

impl Rejection {
    fn new(code: ErrorCode, message: impl Into<String>) -> Rejection {
        Rejection {
            code,
            message: message.into(),
        }
    }
}

impl TabService {
    /// 确保默认 Space 存在，并生成初始快照。返回服务与快照的订阅句柄。
    pub fn new(store: Store) -> anyhow::Result<(TabService, Snapshots)> {
        let id = ulid::Ulid::generate().to_string();
        if store
            .ensure_default_space(&id, now_millis())
            .context("建默认 Space 失败")?
        {
            tracing::info!(space_id = %id, "已建默认 Space");
        }
        let (latest, latest_rx) = watch::channel(snapshot_line(&store)?);
        let (updates, _) = broadcast::channel(UPDATES_CAPACITY);
        let snapshots = Snapshots {
            latest: latest_rx,
            updates: updates.clone(),
        };
        Ok((
            TabService {
                store,
                latest,
                updates,
            },
            snapshots,
        ))
    }

    /// 逐个处理事件，直到所有连接与监听都已关闭。它是数据库的唯一写者。
    pub async fn run(self, mut requests: mpsc::UnboundedReceiver<Request>) {
        while let Some(req) = requests.recv().await {
            let kind = event_kind(&req.event);
            match self.handle(req.event, &req.reply) {
                Ok(true) => self.publish(),
                Ok(false) => {}
                Err(r) => {
                    tracing::warn!(code = ?r.code, event = kind, message = %r.message, "拒绝 shell 事件");
                    let _ = req.reply.send(ShellCommand::Error {
                        code: r.code,
                        message: r.message,
                    });
                }
            }
        }
    }

    /// 处理一个事件；返回状态是否变化（变化了才广播新快照）。
    fn handle(
        &self,
        event: ShellEvent,
        reply: &mpsc::UnboundedSender<ShellCommand>,
    ) -> Result<bool, Rejection> {
        match event {
            ShellEvent::OpenRequested {
                space_id,
                input,
                tab_id,
            } => {
                let url = url::normalize_input(&input)
                    .ok_or_else(|| Rejection::new(ErrorCode::InvalidArgument, "地址为空"))?;
                if !self.store.space_exists(&space_id).map_err(internal)? {
                    return Err(Rejection::new(
                        ErrorCode::SpaceNotFound,
                        format!("Space 不存在：{space_id}"),
                    ));
                }
                if let Some(tab_id) = tab_id {
                    // 在已有标签打开：只下发导航指令，URL 等页面真正导航后由 tabUpdated 回写
                    if !self.store.tab_exists(&tab_id).map_err(internal)? {
                        return Err(tab_not_found(&tab_id));
                    }
                    let _ = reply.send(ShellCommand::Navigate { tab_id, url });
                    return Ok(false);
                }
                let id = ulid::Ulid::generate().to_string();
                self.store
                    .insert_tab(&id, &space_id, &url, now_millis())
                    .map_err(internal)?;
                Ok(true)
            }
            ShellEvent::TabUpdated {
                tab_id,
                url,
                title,
                favicon_url,
            } => self
                .store
                .update_tab(
                    &tab_id,
                    url.as_deref(),
                    title.as_deref(),
                    favicon_url.as_deref(),
                )
                .map_err(internal)?
                .ok_or_else(|| tab_not_found(&tab_id)),
            ShellEvent::TabActivated { tab_id } => {
                if self
                    .store
                    .activate_tab(&tab_id, now_millis())
                    .map_err(internal)?
                {
                    Ok(true)
                } else {
                    Err(tab_not_found(&tab_id))
                }
            }
            ShellEvent::TabClosed { tab_id } => {
                if self
                    .store
                    .close_tab(&tab_id, now_millis())
                    .map_err(internal)?
                {
                    Ok(true)
                } else {
                    Err(tab_not_found(&tab_id))
                }
            }
        }
    }

    /// 重新读库生成快照并通知所有连接；读库失败只记日志，保留上一份快照。
    fn publish(&self) {
        match snapshot_line(&self.store) {
            Ok(line) => {
                self.latest.send_replace(line.clone());
                // 没有连接时 send 返回错误，可以忽略
                let _ = self.updates.send(line);
            }
            Err(e) => tracing::error!(error = %format!("{e:#}"), "生成快照失败"),
        }
    }
}

fn tab_not_found(tab_id: &str) -> Rejection {
    Rejection::new(ErrorCode::TabNotFound, format!("标签不存在：{tab_id}"))
}

fn internal(e: impl std::fmt::Display) -> Rejection {
    Rejection::new(ErrorCode::Internal, format!("数据库错误：{e}"))
}

fn event_kind(event: &ShellEvent) -> &'static str {
    match event {
        ShellEvent::OpenRequested { .. } => "shell.event.openRequested",
        ShellEvent::TabUpdated { .. } => "shell.event.tabUpdated",
        ShellEvent::TabActivated { .. } => "shell.event.tabActivated",
        ShellEvent::TabClosed { .. } => "shell.event.tabClosed",
    }
}

/// 读库生成快照：每个 Space 的激活标签是其中 `last_active_at` 最大的那个。
fn snapshot(store: &Store) -> anyhow::Result<ShellState> {
    let spaces = store.list_spaces().context("读取 Space 失败")?;
    let rows = store.list_open_tabs().context("读取标签失败")?;
    let active_space_id = spaces
        .first()
        .map(|s| s.id.clone())
        .context("没有未归档的 Space")?;
    let mut tabs = Vec::with_capacity(rows.len());
    for row in &rows {
        let latest = rows
            .iter()
            .filter(|r| r.space_id == row.space_id)
            .map(|r| r.last_active_at)
            .max();
        tabs.push(TabView {
            id: row.id.clone(),
            space_id: row.space_id.clone(),
            url: row.url.clone(),
            title: row.title.clone(),
            favicon_url: row.favicon_url.clone(),
            active: false,
        });
        if latest == Some(row.last_active_at) {
            // 同一 Space 里只标第一个达到最大值的（写入时保证严格递增，正常不会并列）
            let already = tabs.iter().any(|t| t.space_id == row.space_id && t.active);
            if !already {
                tabs.last_mut().unwrap().active = true;
            }
        }
    }
    Ok(ShellState {
        active_space_id,
        spaces: spaces
            .into_iter()
            .map(|s| SpaceView {
                id: s.id,
                name: s.name,
                color: s.color,
            })
            .collect(),
        tabs,
    })
}

fn snapshot_line(store: &Store) -> anyhow::Result<Snapshot> {
    let cmd = ShellCommand::StateChanged(snapshot(store)?);
    Ok(serde_json::to_string(&cmd)
        .context("序列化快照失败")?
        .into())
}
