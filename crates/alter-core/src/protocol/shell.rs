//! Shell Control Channel 的消息：alterd 与 Electron Main 之间经 `ALTER_HOME/shell.sock` 交换，
//! 每行一个 JSON 对象，`type` 区分消息。契约见 add-spaces-sidebar 的 design.md「接口契约」。

use super::gateway::{ImportCounts, ImportItem, ImportSource, ImportStatus};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{ErrorCode, Ownership, TaskSpaceStatus};

/// Space 的预设颜色。alterd 校验新建 / 编辑请求，Renderer 的色板也用这一份（导出为 `spaceColors.ts`）。
pub const SPACE_COLORS: [&str; 8] = [
    "#6366f1", "#ef4444", "#f97316", "#eab308", "#22c55e", "#06b6d4", "#3b82f6", "#ec4899",
];

/// 快照里的一个 Space。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SpaceView {
    pub id: String,
    pub name: String,
    pub color: String,
}

/// 快照里的一个未归档标签。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TabView {
    pub id: String,
    pub space_id: String,
    pub url: String,
    pub title: String,
    pub favicon_url: Option<String>,
    /// 是否为所在 Space 的激活标签（每个 Space 至多一个）
    pub active: bool,
    /// 是否在固定分区；同一 Space 内固定标签总排在普通标签之前
    pub pinned: bool,
    /// 固定标签被「关闭」后页面已卸载，用户再次激活前不应加载（只在 alterd 运行期间记住）
    pub discarded: bool,
    /// 托管它的 TaskSpace；用户标签为空
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub task_space_id: Option<i64>,
    /// 在该 TaskSpace 里的 Page 标签（`p1`、`p2`…）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub label: Option<String>,
    /// 分屏组：同组的两个标签值相同，值是左边那个标签的 id；不在分屏里为 null
    #[serde(default)]
    pub split_group: Option<String>,
}

/// 快照里一个活跃的 TaskSpace。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TaskSpaceView {
    #[ts(type = "number")]
    pub id: i64,
    pub space_id: String,
    pub name: String,
    pub ownership: Ownership,
    pub status: TaskSpaceStatus,
}

/// 全量状态快照：Space 按 position 升序；标签先按所属 Space 的顺序、再按 position 升序（固定在前）。
/// `active_space_id` 是 `last_active_at` 最大的 Space，全为空时取排在最前的。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ShellState {
    pub active_space_id: String,
    pub spaces: Vec<SpaceView>,
    pub tabs: Vec<TabView>,
    /// 未结束的 TaskSpace
    pub task_spaces: Vec<TaskSpaceView>,
    /// 每个 Space 最近归档的至多 50 个标签，按归档时间降序
    #[serde(default)]
    pub archived: Vec<ArchivedTab>,
}

/// 快照里的一个已归档标签。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedTab {
    pub id: String,
    pub space_id: String,
    pub url: String,
    pub title: String,
    #[ts(type = "number")]
    pub archived_at: i64,
}

/// 命令栏搜索结果的种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum SearchKind {
    Tab,
    Space,
    Bookmark,
    History,
    Open,
}

/// 命令栏的一项搜索结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SearchItem {
    pub kind: SearchKind,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tab_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub space_id: Option<String>,
    /// `open` 项：原样的输入，执行时按地址框的规则打开或搜索
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub input: Option<String>,
}

/// alterd → Electron。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum ShellCommand {
    /// 全量快照，每次状态变化后广播给所有连接
    #[serde(rename = "shell.stateChanged")]
    StateChanged(ShellState),
    /// 在已有标签里打开 URL，只发给请求方
    #[serde(rename = "shell.navigate")]
    Navigate { tab_id: String, url: String },
    /// 事件被拒绝，只发给请求方
    #[serde(rename = "shell.error")]
    Error { code: ErrorCode, message: String },
    /// 卸载该标签的页面（关闭固定标签时），只发给请求方；下次激活时按快照里的 url 重建
    #[serde(rename = "shell.discard")]
    Discard { tab_id: String },
    /// 命令栏的搜索结果，只发给请求方；`request_id` 原样带回，用来丢弃过期结果
    #[serde(rename = "shell.searchResults")]
    SearchResults {
        request_id: u32,
        items: Vec<SearchItem>,
    },
    /// 可导入的 profile（各数据根目录下含 Bookmarks、History 或 Cookies 的子目录名，按名称排序），只发给请求方
    #[serde(rename = "shell.importSources")]
    ImportSources {
        request_id: u32,
        chrome: Vec<String>,
        arc: Vec<String>,
    },
    /// 导入的进度，只发给发起的连接；最后一条的 status 为 done、partial 或 failed。
    /// 没能开始（来源不存在、Space 不存在等）时只有一条 failed，importId 为 0，error 为 `<错误码>: <说明>`
    #[serde(rename = "shell.importProgress")]
    ImportProgress {
        request_id: u32,
        #[ts(type = "number")]
        import_id: i64,
        status: ImportStatus,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        item: Option<ImportItem>,
        counts: ImportCounts,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        warning: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        error: Option<String>,
    },
}

/// Electron → alterd。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum ShellEvent {
    /// 地址输入：不带 `tab_id` 时新建标签，带上时在该标签打开
    #[serde(rename = "shell.event.openRequested")]
    OpenRequested {
        space_id: String,
        input: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        tab_id: Option<String>,
        /// 网页弹窗（`window.open`、`target=_blank`）的来源标签；来源是 agent 页面时新标签记为同一 TaskSpace 的下一个 Page
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        opener_tab_id: Option<String>,
    },
    /// 页面状态回写；没有变化的字段省略
    #[serde(rename = "shell.event.tabUpdated")]
    TabUpdated {
        tab_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        favicon_url: Option<String>,
        /// 该标签视图的 CDP targetId（Main 建视图后上报一次）；alterd 只记在内存
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        target_id: Option<String>,
    },
    #[serde(rename = "shell.event.tabActivated")]
    TabActivated { tab_id: String },
    #[serde(rename = "shell.event.tabClosed")]
    TabClosed { tab_id: String },
    /// 在末尾新建 Space 并设为当前 Space；名称去首尾空白后 1–32 个字符，颜色是预设之一
    #[serde(rename = "shell.event.spaceCreated")]
    SpaceCreated { name: String, color: String },
    /// 只改给出的字段，两者都没给时什么也不做
    #[serde(rename = "shell.event.spaceUpdated")]
    SpaceUpdated {
        space_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        color: Option<String>,
    },
    #[serde(rename = "shell.event.spaceActivated")]
    SpaceActivated { space_id: String },
    /// 硬删 Space 及其标签；不能删最后一个
    #[serde(rename = "shell.event.spaceDeleted")]
    SpaceDeleted { space_id: String },
    /// 固定：移到固定分区末尾；取消固定：移到普通分区开头
    #[serde(rename = "shell.event.tabPinned")]
    TabPinned { tab_id: String, pinned: bool },
    /// 移到目标分区（`pinned`）的第 `index` 位，超出时放末尾
    #[serde(rename = "shell.event.tabMoved")]
    TabMoved {
        tab_id: String,
        pinned: bool,
        index: u32,
    },
    /// Chromium 的 remote-debugging 端口已就绪；alterd 据此连接 CdpMux
    #[serde(rename = "shell.event.cdpReady")]
    CdpReady { port: u16 },
    /// 用户在接管横幅上点了「交还给 Agent」
    #[serde(rename = "shell.event.ownershipReturned")]
    OwnershipReturned {
        #[ts(type = "number")]
        task_space_id: i64,
    },
    /// 用户点了「停止 Agent」，主动接管
    #[serde(rename = "shell.event.userTookControl")]
    UserTookControl {
        #[ts(type = "number")]
        task_space_id: i64,
    },
    /// 命令栏输入变化，请求搜索结果（只回请求方 `shell.searchResults`）
    #[serde(rename = "shell.event.searchRequested")]
    SearchRequested { request_id: u32, query: String },
    /// 把同一 Space 的两个标签组成分屏，左边是 `tab_ids[0]`
    #[serde(rename = "shell.event.splitRequested")]
    SplitRequested { tab_ids: Vec<String> },
    /// 解散该标签所在的分屏
    #[serde(rename = "shell.event.splitDissolved")]
    SplitDissolved { tab_id: String },
    /// 恢复一个已归档的标签：放到普通分区末尾并激活
    #[serde(rename = "shell.event.tabRestored")]
    TabRestored { tab_id: String },
    /// 导入向导打开：列出可导入的 profile
    #[serde(rename = "shell.event.importSourcesRequested")]
    ImportSourcesRequested { request_id: u32 },
    /// 导入向导点「开始导入」
    #[serde(rename = "shell.event.importRequested")]
    ImportRequested {
        request_id: u32,
        source: ImportSource,
        profile: String,
        space_id: String,
        items: Vec<ImportItem>,
    },
}
