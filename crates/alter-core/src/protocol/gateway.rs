//! Gateway（alterd 对 SDK 与 CLI 的本机 WebSocket JSON-RPC）的消息类型。
//! 方法表见 .dev-flow/infra/api.md，本模块只放跨进程共用的结构；字段一律 camelCase。

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::ErrorCode;

/// Gateway 的协议版本；`hello` 时两边必须一致。
pub const PROTOCOL_VERSION: u32 = 1;

/// 每个 TaskSpace 同时打开的 Page 上限（默认值）。
pub const DEFAULT_PAGE_BUDGET: u32 = 8;

/// `ALTER_HOME/gateway.json`，权限 0600，alterd 退出时删除。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GatewayFile {
    pub port: u16,
    /// 64 个十六进制字符
    pub token: String,
    /// CDP 转发端点
    pub cdp_port: u16,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HelloParams {
    pub client_version: String,
    pub protocol_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HelloSpace {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HelloResult {
    pub protocol_version: u32,
    pub page_budget: u32,
    pub spaces: Vec<HelloSpace>,
}

/// JSON-RPC 错误的 `data`：`code` 总是 16 个错误码之一。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RpcErrorData {
    pub code: ErrorCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "Record<string, unknown>")]
    pub details: Option<serde_json::Value>,
}

/// `space.list` 的一项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SpaceSummary {
    pub id: String,
    pub name: String,
    pub color: String,
    /// 未归档的标签数
    pub tab_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SpaceListResult {
    pub spaces: Vec<SpaceSummary>,
}

/// TaskSpace 的控制权。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum Ownership {
    Agent,
    User,
}

/// ownership 变化的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum OwnershipReason {
    /// agent 调用 `taskSpace.handOff`
    HandOff,
    /// 用户点「停止 Agent」
    UserTookControl,
    /// 用户点「交还给 Agent」
    Returned,
}

/// 通知 `app.updateAvailable` 的参数，也是 Shell 快照里的 `updateAvailable`：GitHub 上有比自身新的版本。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAvailable {
    /// 新版本号，不带 `v`（如 `0.2.0`）
    pub version: String,
    /// 该版本的 Release 页面
    pub url: String,
}

/// 通知 `taskSpace.ownershipChanged` 的参数。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct OwnershipChanged {
    #[ts(type = "number")]
    pub id: i64,
    pub ownership: Ownership,
    pub reason: OwnershipReason,
}

/// `taskSpace.takeOver` 的结果：`user_page` 是所在 Space 激活标签的 Page 标签（没有激活标签时为 null）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TakeOverResult {
    pub ownership: Ownership,
    pub user_page: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum TaskSpaceStatus {
    Active,
    Finished,
    Inactive,
}

/// `taskSpace.create` / `resume` 的结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TaskSpaceInfo {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    pub space_id: String,
    pub ownership: Ownership,
    /// 未关闭的 Page 标签，按编号升序
    pub pages: Vec<String>,
}

/// `taskSpace.list` 的一项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TaskSpaceSummary {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    pub space_id: String,
    pub ownership: Ownership,
    pub status: TaskSpaceStatus,
}

/// `taskSpace.tabs` 的一项：该 TaskSpace 所在 Space 的标签，托管的带 `label`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TaskSpaceTab {
    pub tab_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub label: Option<String>,
    pub title: String,
    pub url: String,
    pub active: bool,
    /// `agent` | `user`；未托管的标签为 `user`
    pub opened_by: String,
}

/// 动作触发的新窗口，记为同一 TaskSpace 的下一个 Page。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Popup {
    pub label: String,
    pub target_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DialogInfo {
    #[serde(rename = "type")]
    pub kind: String,
    pub message: String,
}

/// 页面动作的回执：只描述已派发的动作和立即观察到的弹窗，不代表页面最终状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub ok: bool,
    pub duration_ms: u64,
    pub popups: Vec<Popup>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub dialog: Option<DialogInfo>,
}

/// 导入的来源浏览器。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum ImportSource {
    Chrome,
    Arc,
}

/// 导入的数据类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum ImportItem {
    Bookmarks,
    History,
    Cookies,
}

/// 导入任务的状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum ImportStatus {
    Running,
    Done,
    /// 部分完成（例如 Keychain 拒绝，Cookie 没有导入）
    Partial,
    Failed,
}

/// 各类已导入的条数，以及跳过的损坏记录数。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ImportCounts {
    pub bookmarks: u32,
    pub history: u32,
    pub cookies: u32,
    pub skipped: u32,
}

/// `import.run` 的结果：任务已开始。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ImportStarted {
    #[ts(type = "number")]
    pub import_id: i64,
}

/// `import.status` 的结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ImportState {
    pub status: ImportStatus,
    pub counts: ImportCounts,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub warning: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error: Option<String>,
}

/// 通知 `import.progress`：完成了一类数据。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgress {
    #[ts(type = "number")]
    pub import_id: i64,
    pub item: ImportItem,
    pub counts: ImportCounts,
}

/// 通知 `import.done`：导入结束。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ImportDone {
    #[ts(type = "number")]
    pub import_id: i64,
    pub status: ImportStatus,
    pub counts: ImportCounts,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub warning: Option<String>,
}
