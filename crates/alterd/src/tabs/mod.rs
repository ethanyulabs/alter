//! TabService：Space 与标签状态的唯一来源。独占 `Store`，处理 Electron 上报的事件，
//! 生成 Shell Control 的全量快照。

mod ownership;
mod search;
mod task_space;
mod url;

pub(crate) use task_space::OpError;
pub(crate) use task_space::db as db_error;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use alter_core::protocol::{
    ArchivedTab, ErrorCode, SPACE_COLORS, ShellCommand, ShellEvent, ShellState, SpaceSummary,
    SpaceView, TabView,
};
use alter_core::store::{SpaceDeletion, Store, TabClosing};
use anyhow::Context;
use tokio::sync::mpsc;

use crate::lifecycle::now_millis;

/// 一个连接的发件箱：TabService 按处理事件的顺序把要写给它的行（快照或只回给它的消息）
/// 放进来，连接任务依次写出。每个连接只有这一条队列，写出顺序因此与处理顺序一致。
pub type Outbox = mpsc::UnboundedSender<Arc<str>>;

/// 交给 TabService 的请求。
pub enum Request {
    /// 新连接：先收到当前快照，之后收到每一份新快照
    Connect(Outbox),
    /// 连接上报的事件；只回给它的消息（错误、导航、卸载指令）放进它的发件箱
    Event { event: ShellEvent, outbox: Outbox },
    /// Gateway 的操作：在本循环里执行，返回状态是否变化
    Gateway(GatewayJob),
}

/// Gateway 交给 TabService 执行的一段操作；结果由它自己经 oneshot 送回。
pub type GatewayJob = Box<dyn FnOnce(&mut TabService) -> bool + Send>;

/// 推给 SDK 的页面事件（JSON-RPC 通知），由 Gateway 发给关注该 TaskSpace 的连接。
#[derive(Debug, Clone)]
pub struct Notice {
    pub task_space_id: i64,
    pub method: &'static str,
    pub params: serde_json::Value,
}

/// agent 页面打开、还没拿到 targetId 的弹窗。
struct PendingPopup {
    task_space_id: i64,
    opener_tab: String,
    opener_label: String,
    label: String,
}

pub struct TabService {
    store: Store,
    /// 最新快照（已序列化的 `shell.stateChanged` 一行，不含换行），新连接的第一条
    latest: Arc<str>,
    /// 仍连着的连接；发件箱写不进去（连接已结束）时移除
    outboxes: Vec<Outbox>,
    /// 页面已卸载的固定标签（关闭固定标签时加入，再次激活时移除）。只在内存里：重启后页面本来就没加载
    discarded: HashSet<String>,
    /// 标签视图的 CDP targetId（Main 经 tabUpdated 上报），只在内存里
    targets: HashMap<String, String>,
    /// Chromium 的 remote-debugging 端口（Main 经 cdpReady 上报）
    cdp_port: Option<u16>,
    /// 收到 cdpReady 时由它去连接
    cdp: Arc<crate::cdp::CdpMux>,
    /// 页面事件的出口（Gateway 接收）
    notices: mpsc::UnboundedSender<Notice>,
    /// 新标签 → 等它上报 targetId 的弹窗
    popups_pending: HashMap<String, PendingPopup>,
    /// 来源标签 → 已就绪、还没被动作回执领走的弹窗
    popups_ready: HashMap<String, Vec<alter_core::protocol::Popup>>,
    /// 标签 → 它最近一次导航写入的历史行：之后的标题回写只更新这一行
    history_rows: HashMap<String, i64>,
    /// 接管信号（与 Gateway 共享）：ownership 转为 user 的同一步递增
    takeover: Arc<crate::takeover::Takeover>,
    /// 导入服务：向导的导入经它开始
    imports: Arc<crate::import::Imports>,
    /// ownership 为 user 的 TaskSpace 进入 user 的时刻（只在内存；启动时对库里已是 user 的从此刻算）
    user_since: HashMap<i64, std::time::Instant>,
}

/// 处理一个事件期间只回给请求方的出口。
struct ReplyTo<'a> {
    outbox: &'a Outbox,
}

impl ReplyTo<'_> {
    fn send(&self, cmd: ShellCommand) {
        match serde_json::to_string(&cmd) {
            // 连接已断开时没有人要这条回复
            Ok(line) => {
                let _ = self.outbox.send(line.into());
            }
            Err(e) => tracing::error!(error = %e, "序列化 shell 回复失败"),
        }
    }
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
    /// 确保默认 Space 存在，并生成初始快照。
    pub fn new(
        store: Store,
        cdp: Arc<crate::cdp::CdpMux>,
        notices: mpsc::UnboundedSender<Notice>,
        takeover: Arc<crate::takeover::Takeover>,
        imports: Arc<crate::import::Imports>,
    ) -> anyhow::Result<TabService> {
        let id = ulid::Ulid::generate().to_string();
        if store
            .ensure_default_space(&id, now_millis())
            .context("建默认 Space 失败")?
        {
            tracing::info!(space_id = %id, "已建默认 Space");
        }
        let discarded = HashSet::new();
        let latest = snapshot_line(&store, &discarded)?;
        let now = std::time::Instant::now();
        let user_since = store
            .list_task_spaces()
            .context("读取 TaskSpace 失败")?
            .into_iter()
            .filter(|t| t.ownership == "user" && t.status == "active")
            .map(|t| (t.id, now))
            .collect();
        Ok(TabService {
            store,
            latest,
            outboxes: Vec::new(),
            discarded,
            targets: HashMap::new(),
            cdp_port: None,
            cdp,
            notices,
            popups_pending: HashMap::new(),
            popups_ready: HashMap::new(),
            user_since,
            history_rows: HashMap::new(),
            takeover,
            imports,
        })
    }

    /// 逐个处理事件，直到所有连接与监听都已关闭。它是数据库的唯一写者。
    pub async fn run(mut self, mut requests: mpsc::UnboundedReceiver<Request>) {
        crate::gateway::forward::assert_single_threaded();
        while let Some(req) = requests.recv().await {
            let (event, outbox) = match req {
                Request::Connect(outbox) => {
                    // 顺便移除已结束的连接：只有连接与断开、没有状态变化时也不会累积
                    self.outboxes.retain(|o| !o.is_closed());
                    if outbox.send(self.latest.clone()).is_ok() {
                        self.outboxes.push(outbox);
                    }
                    continue;
                }
                Request::Event { event, outbox } => (event, outbox),
                Request::Gateway(job) => {
                    if job(&mut self) {
                        self.publish();
                    }
                    continue;
                }
            };
            let kind = event_kind(&event);
            let reply = ReplyTo { outbox: &outbox };
            match self.handle(event, &reply) {
                Ok(true) => self.publish(),
                Ok(false) => {}
                Err(r) => {
                    tracing::warn!(code = ?r.code, event = kind, message = %r.message, "拒绝 shell 事件");
                    reply.send(ShellCommand::Error {
                        code: r.code,
                        message: r.message,
                    });
                }
            }
        }
    }

    /// 全部未归档的 Space 及其未归档标签数，顺序同侧边栏。
    /// 数据库（导入服务在本循环里经它读写）。
    pub(crate) fn store(&self) -> &Store {
        &self.store
    }

    pub fn space_summaries(&self) -> anyhow::Result<Vec<SpaceSummary>> {
        let spaces = self.store.list_spaces()?;
        let tabs = self.store.list_open_tabs()?;
        Ok(spaces
            .into_iter()
            .map(|s| SpaceSummary {
                tab_count: tabs.iter().filter(|t| t.space_id == s.id).count() as u32,
                id: s.id,
                name: s.name,
                color: s.color,
            })
            .collect())
    }

    /// 处理一个事件；返回状态是否变化（变化了才广播新快照）。
    fn handle(&mut self, event: ShellEvent, reply: &ReplyTo<'_>) -> Result<bool, Rejection> {
        match event {
            ShellEvent::OpenRequested {
                space_id,
                input,
                tab_id,
                opener_tab_id,
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
                    if self.discarded.contains(&tab_id) {
                        // 已卸载的标签没有页面可导航：直接改 url 并广播，Main 会按新 url 建视图。
                        // 写库成功后才清除卸载状态，失败时它保持卸载
                        self.store
                            .update_tab(&tab_id, Some(&url), None, None)
                            .map_err(internal)?;
                        self.discarded.remove(&tab_id);
                        return Ok(true);
                    }
                    reply.send(ShellCommand::Navigate { tab_id, url });
                    return Ok(false);
                }
                // agent 页面打开的弹窗：记为同一 TaskSpace 的下一个 Page，不抢用户的激活标签
                if let Some(opener) = opener_tab_id
                    && let Some(page) = self.store.page_of_tab(&opener).map_err(internal)?
                {
                    self.open_popup(&opener, &page, &url).map_err(internal)?;
                    return Ok(true);
                }
                let id = ulid::Ulid::generate().to_string();
                self.store
                    .insert_tab(&id, &space_id, &url, now_millis())
                    .map_err(internal)?;
                Ok(true)
            }
            // 已卸载的固定标签不会再有页面：这时到达的回写一定来自旧页面，忽略
            ShellEvent::TabUpdated { tab_id, .. } if self.discarded.contains(&tab_id) => Ok(false),
            ShellEvent::TabUpdated {
                tab_id,
                url,
                title,
                favicon_url,
                target_id,
            } => {
                let changed = self
                    .store
                    .update_tab(
                        &tab_id,
                        url.as_deref(),
                        title.as_deref(),
                        favicon_url.as_deref(),
                    )
                    .map_err(internal)?
                    .ok_or_else(|| tab_not_found(&tab_id))?;
                self.record_history(&tab_id, url.as_deref(), title.as_deref())
                    .map_err(internal)?;
                // targetId 只记在内存，不改库、不广播
                if let Some(target_id) = target_id {
                    self.popup_ready(&tab_id, &target_id);
                    self.targets.insert(tab_id, target_id);
                }
                Ok(changed)
            }
            ShellEvent::CdpReady { port } => {
                self.cdp_port = Some(port);
                let cdp = self.cdp.clone();
                tokio::spawn(async move {
                    if let Err(e) = cdp.connect(port).await {
                        tracing::error!(error = %format!("{e:#}"), port, "连接 Chromium 调试端口失败");
                    }
                });
                Ok(false)
            }
            ShellEvent::OwnershipReturned { task_space_id } => self.ownership_event(
                task_space_id,
                alter_core::protocol::Ownership::Agent,
                alter_core::protocol::OwnershipReason::Returned,
            ),
            ShellEvent::UserTookControl { task_space_id } => self.ownership_event(
                task_space_id,
                alter_core::protocol::Ownership::User,
                alter_core::protocol::OwnershipReason::UserTookControl,
            ),
            ShellEvent::SearchRequested { request_id, query } => {
                let items = self.search(&query).map_err(internal)?;
                reply.send(ShellCommand::SearchResults { request_id, items });
                Ok(false)
            }
            ShellEvent::SplitRequested { tab_ids } => {
                let [left, right] = tab_ids.as_slice() else {
                    return Err(Rejection::new(
                        ErrorCode::InvalidArgument,
                        format!("分屏需要恰好两个标签，收到 {} 个", tab_ids.len()),
                    ));
                };
                if left == right {
                    return Err(Rejection::new(
                        ErrorCode::InvalidArgument,
                        "分屏需要两个不同的标签",
                    ));
                }
                let l = self
                    .store
                    .tab(left)
                    .map_err(internal)?
                    .ok_or_else(|| tab_not_found(left))?;
                let r = self
                    .store
                    .tab(right)
                    .map_err(internal)?
                    .ok_or_else(|| tab_not_found(right))?;
                if l.space_id != r.space_id {
                    return Err(Rejection::new(
                        ErrorCode::InvalidArgument,
                        "分屏的两个标签要在同一个 Space",
                    ));
                }
                self.store.split(left, right).map_err(internal)?;
                Ok(true)
            }
            ShellEvent::SplitDissolved { tab_id } => {
                if !self.store.tab_exists(&tab_id).map_err(internal)? {
                    return Err(tab_not_found(&tab_id));
                }
                self.store.dissolve_split(&tab_id).map_err(internal)
            }
            ShellEvent::TabRestored { tab_id } => {
                if self
                    .store
                    .restore_tab(&tab_id, now_millis())
                    .map_err(internal)?
                {
                    Ok(true)
                } else {
                    Err(Rejection::new(
                        ErrorCode::TabNotFound,
                        format!("没有已归档的标签：{tab_id}"),
                    ))
                }
            }
            // 列目录很快，直接在循环里做
            ShellEvent::ImportSourcesRequested { request_id } => {
                use alter_core::protocol::ImportSource;
                reply.send(ShellCommand::ImportSources {
                    request_id,
                    chrome: crate::import::sources::list(ImportSource::Chrome),
                    arc: crate::import::sources::list(ImportSource::Arc),
                });
                Ok(false)
            }
            // 导入的读写要经过本循环：放到独立任务里等它，进度只回发起的连接
            ShellEvent::ImportRequested {
                request_id,
                source,
                profile,
                space_id,
                items,
            } => {
                tokio::spawn(crate::import::run_for_shell(
                    self.imports.clone(),
                    request_id,
                    source,
                    profile,
                    space_id,
                    items,
                    reply.outbox.clone(),
                ));
                Ok(false)
            }
            ShellEvent::TabActivated { tab_id } => {
                if self
                    .store
                    .activate_tab(&tab_id, now_millis())
                    .map_err(internal)?
                {
                    self.discarded.remove(&tab_id);
                    Ok(true)
                } else {
                    Err(tab_not_found(&tab_id))
                }
            }
            ShellEvent::TabClosed { tab_id } => {
                match self
                    .store
                    .close_tab(&tab_id, now_millis())
                    .map_err(internal)?
                {
                    TabClosing::Archived => {
                        self.history_rows.remove(&tab_id);
                        // 用户关掉了 TaskSpace 的页面：该 Page 随之关闭
                        self.store
                            .close_pages_of_tab(&tab_id, now_millis())
                            .map_err(internal)?;
                        Ok(true)
                    }
                    TabClosing::Discarded => {
                        self.discarded.insert(tab_id.clone());
                        // 固定标签只卸载页面：让请求方关掉它的视图，下次激活时按快照里的 url 重建
                        reply.send(ShellCommand::Discard { tab_id });
                        Ok(true)
                    }
                    TabClosing::NotFound => Err(tab_not_found(&tab_id)),
                }
            }
            ShellEvent::SpaceCreated { name, color } => {
                let name = valid_space_name(&name)?;
                valid_space_color(&color)?;
                let id = ulid::Ulid::generate().to_string();
                self.store
                    .create_space(&id, name, &color, now_millis())
                    .map_err(internal)?;
                Ok(true)
            }
            ShellEvent::SpaceActivated { space_id } => {
                if self
                    .store
                    .activate_space(&space_id, now_millis())
                    .map_err(internal)?
                {
                    Ok(true)
                } else {
                    Err(space_not_found(&space_id))
                }
            }
            ShellEvent::SpaceUpdated {
                space_id,
                name,
                color,
            } => {
                let name = name.as_deref().map(valid_space_name).transpose()?;
                if let Some(color) = &color {
                    valid_space_color(color)?;
                }
                self.store
                    .update_space(&space_id, name, color.as_deref())
                    .map_err(internal)?
                    .ok_or_else(|| space_not_found(&space_id))
            }
            ShellEvent::SpaceDeleted { space_id } => {
                match self
                    .store
                    .delete_space(&space_id, now_millis())
                    .map_err(internal)?
                {
                    SpaceDeletion::Deleted => {
                        // 被删除的 Space 的标签不再回写：只留仍打开的标签的历史行
                        let open: HashSet<String> = self
                            .store
                            .list_open_tabs()
                            .map_err(internal)?
                            .into_iter()
                            .map(|t| t.id)
                            .collect();
                        self.history_rows.retain(|tab, _| open.contains(tab));
                        Ok(true)
                    }
                    SpaceDeletion::NotFound => Err(space_not_found(&space_id)),
                    SpaceDeletion::LastSpace => Err(Rejection::new(
                        ErrorCode::InvalidArgument,
                        "不能删除最后一个 Space",
                    )),
                }
            }
            ShellEvent::TabPinned { tab_id, pinned } => self
                .store
                .set_pinned(&tab_id, pinned)
                .map_err(internal)?
                .ok_or_else(|| tab_not_found(&tab_id)),
            ShellEvent::TabMoved {
                tab_id,
                pinned,
                index,
            } => {
                if self
                    .store
                    .move_tab(&tab_id, pinned, index as usize)
                    .map_err(internal)?
                {
                    Ok(true)
                } else {
                    Err(tab_not_found(&tab_id))
                }
            }
        }
    }

    /// 重新读库生成快照并放进每个连接的发件箱；读库失败只记日志，保留上一份快照。
    fn publish(&mut self) {
        // 已不存在（被删除的 Space 里）的标签不必再记
        if !self.discarded.is_empty() {
            let store = &self.store;
            self.discarded
                .retain(|id| store.tab_exists(id).unwrap_or(true));
        }
        match snapshot_line(&self.store, &self.discarded) {
            Ok(line) => {
                self.latest = line.clone();
                // 各连接共享同一份内容；发件箱已关闭的连接（任务已结束）顺便移除
                self.outboxes.retain(|o| o.send(line.clone()).is_ok());
            }
            Err(e) => tracing::error!(error = %format!("{e:#}"), "生成快照失败"),
        }
    }
}

/// Space 名称去掉首尾空白后须为 1–32 个字符（按字符而不是字节计）。
fn valid_space_name(name: &str) -> Result<&str, Rejection> {
    let name = name.trim();
    let len = name.chars().count();
    if (1..=32).contains(&len) {
        Ok(name)
    } else {
        Err(Rejection::new(
            ErrorCode::InvalidArgument,
            format!("Space 名称须为 1–32 个字符，实际 {len} 个"),
        ))
    }
}

/// Space 颜色须是预设之一。
fn valid_space_color(color: &str) -> Result<(), Rejection> {
    if SPACE_COLORS.contains(&color) {
        Ok(())
    } else {
        Err(Rejection::new(
            ErrorCode::InvalidArgument,
            format!("颜色不在预设内：{color}"),
        ))
    }
}

fn space_not_found(space_id: &str) -> Rejection {
    Rejection::new(
        ErrorCode::SpaceNotFound,
        format!("Space 不存在：{space_id}"),
    )
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
        ShellEvent::SpaceCreated { .. } => "shell.event.spaceCreated",
        ShellEvent::SpaceUpdated { .. } => "shell.event.spaceUpdated",
        ShellEvent::SpaceActivated { .. } => "shell.event.spaceActivated",
        ShellEvent::SpaceDeleted { .. } => "shell.event.spaceDeleted",
        ShellEvent::TabPinned { .. } => "shell.event.tabPinned",
        ShellEvent::TabMoved { .. } => "shell.event.tabMoved",
        ShellEvent::CdpReady { .. } => "shell.event.cdpReady",
        ShellEvent::OwnershipReturned { .. } => "shell.event.ownershipReturned",
        ShellEvent::UserTookControl { .. } => "shell.event.userTookControl",
        ShellEvent::SearchRequested { .. } => "shell.event.searchRequested",
        ShellEvent::SplitRequested { .. } => "shell.event.splitRequested",
        ShellEvent::SplitDissolved { .. } => "shell.event.splitDissolved",
        ShellEvent::TabRestored { .. } => "shell.event.tabRestored",
        ShellEvent::ImportSourcesRequested { .. } => "shell.event.importSourcesRequested",
        ShellEvent::ImportRequested { .. } => "shell.event.importRequested",
    }
}

impl TabService {
    /// 自动归档；返回是否有标签被归档（有才发布快照）。
    pub(crate) fn archive_stale(&mut self, now: i64, threshold_ms: i64) -> bool {
        match self.store.archive_stale(now, threshold_ms) {
            Ok(ids) => {
                for id in &ids {
                    self.discarded.remove(id);
                    self.history_rows.remove(id);
                }
                !ids.is_empty()
            }
            Err(e) => {
                tracing::error!(error = %e, "自动归档失败");
                false
            }
        }
    }
}

impl TabService {
    /// 标签回写写历史：带 url（一次主框架导航）就追加一条，标题取标签当前的标题；只带标题时更新
    /// 该标签最近一次导航写入的那一行（alterd 重启后、该标签下一次导航之前的标题回写不写历史）。`about:blank`（新开的空白页、agent 页面的初始页）不算访问。
    fn record_history(
        &mut self,
        tab_id: &str,
        url: Option<&str>,
        title: Option<&str>,
    ) -> anyhow::Result<()> {
        let Some(tab) = self.store.tab(tab_id)? else {
            return Ok(());
        };
        if tab.url == "about:blank" {
            return Ok(());
        }
        if url.is_some() {
            let row = self
                .store
                .add_history(&tab.space_id, &tab.url, &tab.title, now_millis())?;
            self.history_rows.insert(tab_id.to_owned(), row);
        } else if let Some(title) = title
            && let Some(&row) = self.history_rows.get(tab_id)
        {
            self.store.update_history_title(row, title)?;
        }
        Ok(())
    }
}

/// 定时把交还超时的 TaskSpace 标记为 inactive：周期取 `min(阈值 / 3, 60 s)`。
pub async fn inactive_checker(
    requests: mpsc::UnboundedSender<Request>,
    threshold: std::time::Duration,
) {
    let period = (threshold / 3).clamp(
        std::time::Duration::from_millis(10),
        std::time::Duration::from_secs(60),
    );
    let mut tick = tokio::time::interval(period);
    loop {
        tick.tick().await;
        let job: GatewayJob = Box::new(move |t| t.mark_inactive(threshold));
        if requests.send(Request::Gateway(job)).is_err() {
            return;
        }
    }
}

/// 快照里每个 Space 至多列出的已归档标签数。
const ARCHIVED_PER_SPACE: usize = 50;

/// 读库生成快照：每个 Space 的激活标签是其中 `last_active_at` 最大的那个。
fn snapshot(store: &Store, discarded: &HashSet<String>) -> anyhow::Result<ShellState> {
    let spaces = store.list_spaces().context("读取 Space 失败")?;
    let rows = store.list_open_tabs().context("读取标签失败")?;
    let active_space_id = store
        .active_space_id()
        .context("读取当前 Space 失败")?
        .context("没有未归档的 Space")?;
    // 托管关系：标签 → (TaskSpace, Page 标签)
    let pages: HashMap<String, (i64, String)> = store
        .all_open_pages()
        .context("读取 TaskSpace 页面失败")?
        .into_iter()
        .map(|p| (p.tab_id, (p.task_space_id, p.label)))
        .collect();
    // 每个 Space 的激活标签：未卸载标签里 last_active_at 最大的（写入时保证严格递增，取第一个）
    let mut active: HashMap<&str, (&str, i64)> = HashMap::new();
    for row in rows.iter().filter(|r| !discarded.contains(&r.id)) {
        let best = active
            .entry(row.space_id.as_str())
            .or_insert((row.id.as_str(), row.last_active_at));
        if row.last_active_at > best.1 {
            *best = (row.id.as_str(), row.last_active_at);
        }
    }
    let tabs = rows
        .iter()
        .map(|row| {
            let page = pages.get(&row.id);
            TabView {
                id: row.id.clone(),
                space_id: row.space_id.clone(),
                url: row.url.clone(),
                title: row.title.clone(),
                favicon_url: row.favicon_url.clone(),
                active: active.get(row.space_id.as_str()).map(|a| a.0) == Some(row.id.as_str()),
                pinned: row.pinned,
                discarded: discarded.contains(&row.id),
                task_space_id: page.map(|p| p.0),
                label: page.map(|p| p.1.clone()),
                split_group: row.split_group.clone(),
            }
        })
        .collect();
    let task_spaces = store
        .list_task_spaces()
        .context("读取 TaskSpace 失败")?
        .into_iter()
        // 交还超时的（inactive）仍显示：用户还可以从横幅交还
        .filter(|t| t.status != "finished")
        .map(task_space::view)
        .collect();
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
        task_spaces,
        archived: store
            .list_archived(ARCHIVED_PER_SPACE)
            .context("读取已归档标签失败")?
            .into_iter()
            .map(|a| ArchivedTab {
                id: a.id,
                space_id: a.space_id,
                url: a.url,
                title: a.title,
                archived_at: a.archived_at,
            })
            .collect(),
    })
}

fn snapshot_line(store: &Store, discarded: &HashSet<String>) -> anyhow::Result<Arc<str>> {
    let cmd = ShellCommand::StateChanged(snapshot(store, discarded)?);
    Ok(serde_json::to_string(&cmd)
        .context("序列化快照失败")?
        .into())
}
