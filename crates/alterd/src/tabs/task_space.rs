//! TabService 里的 TaskSpace 账目：agent 的 TaskSpace 与 Page（`p1`、`p2`…）。在单写者循环里执行，
//! 由 Gateway 调用；返回 `(结果, 状态是否变化)`。

use alter_core::protocol::{
    DEFAULT_PAGE_BUDGET, ErrorCode, Ownership, TaskSpaceInfo, TaskSpaceStatus, TaskSpaceSummary,
    TaskSpaceTab, TaskSpaceView,
};
use alter_core::store::{PageRow, TabClosing, TaskSpaceRow};
use serde_json::{Value, json};

use super::TabService;
use crate::lifecycle::now_millis;

/// TaskSpace 操作失败的原因，Gateway 把它转成 JSON-RPC 错误。
#[derive(Debug)]
pub(crate) struct OpError {
    pub code: ErrorCode,
    pub message: String,
    pub details: Option<Value>,
}

impl OpError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> OpError {
        OpError {
            code,
            message: message.into(),
            details: None,
        }
    }
}

pub(crate) fn db(e: impl std::fmt::Display) -> OpError {
    OpError::new(ErrorCode::Internal, format!("数据库错误：{e}"))
}

pub(super) type Op<T> = Result<(T, bool), OpError>;

fn ownership(s: &str) -> Ownership {
    if s == "user" {
        Ownership::User
    } else {
        Ownership::Agent
    }
}

fn status(s: &str) -> TaskSpaceStatus {
    match s {
        "finished" => TaskSpaceStatus::Finished,
        "inactive" => TaskSpaceStatus::Inactive,
        _ => TaskSpaceStatus::Active,
    }
}

/// 快照里的 TaskSpace。
pub(super) fn view(t: TaskSpaceRow) -> TaskSpaceView {
    TaskSpaceView {
        id: t.id,
        ownership: ownership(&t.ownership),
        status: status(&t.status),
        space_id: t.space_id,
        name: t.name,
    }
}

impl TabService {
    pub(super) fn task_space_row(&self, id: i64) -> Result<TaskSpaceRow, OpError> {
        self.store.task_space(id).map_err(db)?.ok_or_else(|| {
            OpError::new(
                ErrorCode::TaskSpaceNotFound,
                format!("TaskSpace 不存在：{id}"),
            )
        })
    }

    fn task_space_info(&self, row: &TaskSpaceRow) -> Result<TaskSpaceInfo, OpError> {
        let pages = self.store.open_pages(row.id).map_err(db)?;
        Ok(TaskSpaceInfo {
            id: row.id,
            name: row.name.clone(),
            space_id: row.space_id.clone(),
            ownership: ownership(&row.ownership),
            pages: pages.into_iter().map(|p| p.label).collect(),
        })
    }

    /// 在 TaskSpace 的 Space 末尾新建一个不抢激活的空白标签，记为下一个 Page。
    fn open_agent_page(&self, row: &TaskSpaceRow) -> Result<String, OpError> {
        let label = self.store.next_page_label(row.id).map_err(db)?;
        let tab_id = ulid::Ulid::generate().to_string();
        self.store
            .insert_agent_tab(&tab_id, &row.space_id, "about:blank", now_millis())
            .map_err(db)?;
        self.store
            .add_page(row.id, &label, &tab_id, "agent")
            .map_err(db)?;
        Ok(label)
    }

    pub(crate) fn task_space_create(
        &mut self,
        name: &str,
        space_id: Option<&str>,
    ) -> Op<TaskSpaceInfo> {
        let space_id = match space_id {
            Some(id) => id.to_owned(),
            None => self
                .store
                .active_space_id()
                .map_err(db)?
                .ok_or_else(|| OpError::new(ErrorCode::Internal, "没有 Space"))?,
        };
        if !self.store.space_exists(&space_id).map_err(db)? {
            return Err(OpError::new(
                ErrorCode::SpaceNotFound,
                format!("Space 不存在：{space_id}"),
            ));
        }
        let id = self
            .store
            .create_task_space(&space_id, name, DEFAULT_PAGE_BUDGET as i64, now_millis())
            .map_err(db)?;
        let row = self.task_space_row(id)?;
        self.open_agent_page(&row)?;
        Ok((self.task_space_info(&row)?, true))
    }

    pub(crate) fn task_space_resume(&self, id: i64) -> Op<TaskSpaceInfo> {
        let row = self.task_space_row(id)?;
        Ok((self.task_space_info(&row)?, false))
    }

    pub(crate) fn task_space_list(&self) -> Op<Vec<TaskSpaceSummary>> {
        let rows = self.store.list_task_spaces().map_err(db)?;
        Ok((
            rows.into_iter()
                .map(|t| TaskSpaceSummary {
                    id: t.id,
                    ownership: ownership(&t.ownership),
                    status: status(&t.status),
                    space_id: t.space_id,
                    name: t.name,
                })
                .collect(),
            false,
        ))
    }

    pub(crate) fn task_space_new_page(&mut self, id: i64) -> Op<String> {
        self.ensure_agent(id)?;
        let row = self.task_space_row(id)?;
        let open = self.store.open_pages(id).map_err(db)?.len() as i64;
        if open >= row.page_budget {
            return Err(OpError {
                details: Some(json!({"budget": row.page_budget, "open": open})),
                ..OpError::new(
                    ErrorCode::PageBudget,
                    format!("页面数已达上限 {}", row.page_budget),
                )
            });
        }
        Ok((self.open_agent_page(&row)?, true))
    }

    /// TaskSpace 所在 Space 的全部标签；托管在本 TaskSpace 的带 `label`。
    pub(crate) fn task_space_tabs(&self, id: i64) -> Op<Vec<TaskSpaceTab>> {
        let row = self.task_space_row(id)?;
        let pages = self.store.open_pages(id).map_err(db)?;
        let tabs: Vec<_> = self
            .store
            .list_open_tabs()
            .map_err(db)?
            .into_iter()
            .filter(|t| t.space_id == row.space_id)
            .collect();
        let active = tabs
            .iter()
            .filter(|t| !self.discarded.contains(&t.id))
            .max_by_key(|t| t.last_active_at)
            .map(|t| t.id.clone());
        Ok((
            tabs.into_iter()
                .map(|t| {
                    let page = pages.iter().find(|p| p.tab_id == t.id);
                    TaskSpaceTab {
                        active: active.as_deref() == Some(t.id.as_str()),
                        label: page.map(|p| p.label.clone()),
                        opened_by: page.map_or("user", |p| p.opened_by.as_str()).to_owned(),
                        title: t.title,
                        url: t.url,
                        tab_id: t.id,
                    }
                })
                .collect(),
            false,
        ))
    }

    pub(crate) fn task_space_adopt(&mut self, id: i64, tab_id: &str) -> Op<String> {
        self.ensure_agent(id)?;
        let row = self.task_space_row(id)?;
        if self.store.tab_space(tab_id).map_err(db)?.as_deref() != Some(row.space_id.as_str()) {
            return Err(OpError::new(
                ErrorCode::TabNotFound,
                format!("该 TaskSpace 的 Space 里没有标签：{tab_id}"),
            ));
        }
        if let Some(page) = self.store.page_of_tab(tab_id).map_err(db)? {
            return Err(OpError::new(
                ErrorCode::InvalidArgument,
                format!("标签已被 TaskSpace {} 托管", page.task_space_id),
            ));
        }
        let label = self.store.next_page_label(id).map_err(db)?;
        self.store
            .add_page(id, &label, tab_id, "user")
            .map_err(db)?;
        Ok((label, true))
    }

    fn open_page_row(&self, id: i64, label: &str) -> Result<PageRow, OpError> {
        self.task_space_row(id)?;
        self.store.open_page(id, label).map_err(db)?.ok_or_else(|| {
            OpError::new(
                ErrorCode::PageNotFound,
                format!("TaskSpace {id} 没有页面 {label}"),
            )
        })
    }

    pub(crate) fn task_space_release(&mut self, id: i64, label: &str) -> Op<()> {
        self.ensure_agent(id)?;
        self.open_page_row(id, label)?;
        self.store.close_page(id, label, now_millis()).map_err(db)?;
        Ok(((), true))
    }

    /// 关掉 agent 打开的标签（归档），并关闭 Page。
    fn close_agent_page(&mut self, page: &PageRow) -> Result<(), OpError> {
        if page.opened_by == "agent" {
            self.history_rows.remove(&page.tab_id);
        }
        if page.opened_by == "agent"
            && self
                .store
                .close_tab(&page.tab_id, now_millis())
                .map_err(db)?
                == TabClosing::Discarded
        {
            self.discarded.insert(page.tab_id.clone());
        }
        self.store
            .close_page(page.task_space_id, &page.label, now_millis())
            .map_err(db)?;
        Ok(())
    }

    pub(crate) fn task_space_finish(
        &mut self,
        id: i64,
        keep: &[String],
    ) -> Op<(Vec<String>, Vec<String>)> {
        self.ensure_agent(id)?;
        self.task_space_row(id)?;
        let (mut closed, mut kept) = (Vec::new(), Vec::new());
        for page in self.store.open_pages(id).map_err(db)? {
            if page.opened_by == "agent" && !keep.contains(&page.label) {
                self.close_agent_page(&page)?;
                closed.push(page.label);
            } else {
                self.store
                    .close_page(id, &page.label, now_millis())
                    .map_err(db)?;
                kept.push(page.label);
            }
        }
        self.store.finish_task_space(id, now_millis()).map_err(db)?;
        Ok(((closed, kept), true))
    }

    pub(crate) fn page_close(&mut self, id: i64, label: &str) -> Op<()> {
        let page = self.open_page_row(id, label)?;
        self.close_agent_page(&page)?;
        Ok(((), true))
    }

    /// Page 对应视图的 CDP targetId；视图还没建好（Main 还没上报）时为 None。
    pub(crate) fn page_target(&self, id: i64, label: &str) -> Op<Option<String>> {
        let page = self.open_page_row(id, label)?;
        Ok((self.targets.get(&page.tab_id).cloned(), false))
    }

    /// 页面的地址与标题（来自标签的回写）。
    pub(crate) fn page_info(&self, id: i64, label: &str) -> Op<Value> {
        let page = self.open_page_row(id, label)?;
        let tab = self
            .store
            .list_open_tabs()
            .map_err(db)?
            .into_iter()
            .find(|t| t.id == page.tab_id)
            .ok_or_else(|| OpError::new(ErrorCode::PageNotFound, "页面的标签已关闭"))?;
        Ok((
            json!({"url": tab.url, "title": tab.title, "loading": false}),
            false,
        ))
    }

    /// 来源标签是 `opener` 页面的弹窗：新建 agent 标签并记为下一个 Page，等它上报 targetId。
    pub(super) fn open_popup(
        &mut self,
        opener: &str,
        page: &PageRow,
        url: &str,
    ) -> anyhow::Result<()> {
        let row = self
            .store
            .task_space(page.task_space_id)?
            .ok_or_else(|| anyhow::anyhow!("TaskSpace 不存在：{}", page.task_space_id))?;
        let label = self.store.next_page_label(row.id)?;
        let tab_id = ulid::Ulid::generate().to_string();
        self.store
            .insert_agent_tab(&tab_id, &row.space_id, url, now_millis())?;
        self.store.add_page(row.id, &label, &tab_id, "agent")?;
        self.popups_pending.insert(
            tab_id,
            super::PendingPopup {
                task_space_id: row.id,
                opener_tab: opener.to_owned(),
                opener_label: page.label.clone(),
                label,
            },
        );
        Ok(())
    }

    /// 弹窗的视图上报了 targetId：放进来源页面待领取的列表，并推送 `page.popup`。
    pub(super) fn popup_ready(&mut self, tab_id: &str, target_id: &str) {
        let Some(p) = self.popups_pending.remove(tab_id) else {
            return;
        };
        self.notify(
            p.task_space_id,
            "page.popup",
            json!({"label": p.opener_label, "newLabel": p.label, "targetId": target_id}),
        );
        self.popups_ready
            .entry(p.opener_tab)
            .or_default()
            .push(alter_core::protocol::Popup {
                label: p.label,
                target_id: target_id.to_owned(),
            });
    }

    /// 领走页面打开的、已就绪的弹窗。
    pub(crate) fn take_popups(
        &mut self,
        id: i64,
        label: &str,
    ) -> Op<Vec<alter_core::protocol::Popup>> {
        let page = self.open_page_row(id, label)?;
        Ok((
            self.popups_ready.remove(&page.tab_id).unwrap_or_default(),
            false,
        ))
    }

    /// TaskSpace 各未关闭 Page 视图的 targetId（还没上报的跳过）。
    pub(crate) fn task_space_targets(&self, id: i64) -> Vec<String> {
        match self.store.open_pages(id) {
            Ok(pages) => pages
                .iter()
                .filter_map(|p| self.targets.get(&p.tab_id).cloned())
                .collect(),
            Err(e) => {
                tracing::error!(error = %e, task_space_id = id, "读取 Page 失败");
                Vec::new()
            }
        }
    }

    /// 是否有 TaskSpace 正由用户控制（CDP 转发端点据此拒绝连接）。
    pub(crate) fn any_user_controlled(&self) -> bool {
        match self.store.list_task_spaces() {
            Ok(rows) => rows
                .iter()
                .any(|t| t.ownership == "user" && t.status != "finished"),
            // 读不了库时按「有」处理：宁可拒绝转发，也不放过用户控制期间的操作
            Err(e) => {
                tracing::error!(error = %e, "读取 TaskSpace 失败");
                true
            }
        }
    }

    /// targetId 对应的 Page（TaskSpace 与标签）；不是 agent 页面时为 None。
    pub(crate) fn page_of_target(&self, target_id: &str) -> Option<(i64, String)> {
        let tab_id = self
            .targets
            .iter()
            .find(|(_, t)| t.as_str() == target_id)
            .map(|(tab, _)| tab.clone())?;
        match self.store.page_of_tab(&tab_id) {
            Ok(page) => page.map(|p| (p.task_space_id, p.label)),
            Err(e) => {
                tracing::error!(error = %e, "查询标签所属页面失败");
                None
            }
        }
    }

    /// 推送页面事件；`params` 补上 `id`。
    pub(crate) fn notify(&self, task_space_id: i64, method: &'static str, mut params: Value) {
        params["id"] = json!(task_space_id);
        let _ = self.notices.send(super::Notice {
            task_space_id,
            method,
            params,
        });
    }
}
