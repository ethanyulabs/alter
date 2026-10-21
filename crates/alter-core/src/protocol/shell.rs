//! Shell Control Channel 的消息：alterd 与 Electron Main 之间经 `ALTER_HOME/shell.sock` 交换，
//! 每行一个 JSON 对象，`type` 区分消息。契约见 add-spaces-sidebar 的 design.md「接口契约」。

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::ErrorCode;

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
}

/// 全量状态快照：Space 按 position 升序；标签先按所属 Space 的顺序、再按 position 升序。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ShellState {
    pub active_space_id: String,
    pub spaces: Vec<SpaceView>,
    pub tabs: Vec<TabView>,
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
    },
    #[serde(rename = "shell.event.tabActivated")]
    TabActivated { tab_id: String },
    #[serde(rename = "shell.event.tabClosed")]
    TabClosed { tab_id: String },
}
