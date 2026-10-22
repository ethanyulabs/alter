//! TaskSpace 的 ownership 状态机：handOff / 用户交还 / takeOver / 用户主动接管，以及交还超时标记 inactive。
//! ownership 写库并推送 `taskSpace.ownershipChanged`；转为 user 的通知由 Gateway 用来取消该 TaskSpace
//! 正在执行的命令。进入 user 的时间只记在内存，alterd 重启后从启动时刻重新计时。

use std::time::{Duration, Instant};

use alter_core::protocol::{
    ErrorCode, Ownership, OwnershipChanged, OwnershipReason, TakeOverResult,
};
use alter_core::store::TaskSpaceRow;

use super::task_space::{Op, OpError, db};
use super::{Rejection, TabService};

fn owner_str(o: Ownership) -> &'static str {
    match o {
        Ownership::Agent => "agent",
        Ownership::User => "user",
    }
}

impl TabService {
    /// ownership 为 user 时回 `E_USER_CONTROL`：agent 的命令都先过这一关。
    pub(crate) fn ensure_agent(&self, id: i64) -> Op<()> {
        let row = self.task_space_row(id)?;
        check_agent(&row)?;
        Ok(((), false))
    }

    /// 原始 CDP 透传（`taskSpace.cdp`、`page.cdp`）的检查：命令可以作用于任何页面，
    /// 所以只要有 TaskSpace 正由用户控制就拒绝，而不只看请求的那个 TaskSpace。
    pub(crate) fn ensure_raw_cdp_allowed(&self, id: i64) -> Op<()> {
        self.ensure_agent(id)?;
        if self.any_user_controlled() {
            return Err(OpError::new(
                ErrorCode::UserControl,
                "有 TaskSpace 正由用户控制，等用户交还后才能使用原始 CDP 透传",
            ));
        }
        Ok(((), false))
    }

    /// `taskSpace.handOff`：交给用户。已经是 user 时什么也不做。
    pub(crate) fn task_space_hand_off(&mut self, id: i64) -> Op<()> {
        let row = self.task_space_row(id)?;
        let changed = self
            .set_owner(&row, Ownership::User, OwnershipReason::HandOff)
            .map_err(db)?;
        Ok(((), changed))
    }

    /// `taskSpace.takeOver`：用户已交还时接回，返回所在 Space 激活标签的 Page 标签（未托管时先托管）。
    pub(crate) fn task_space_take_over(&mut self, id: i64) -> Op<TakeOverResult> {
        let row = self.task_space_row(id)?;
        check_agent(&row)?;
        let active = self
            .store
            .list_open_tabs()
            .map_err(db)?
            .into_iter()
            .filter(|t| t.space_id == row.space_id && !self.discarded.contains(&t.id))
            .max_by_key(|t| t.last_active_at);
        let Some(tab) = active else {
            return Ok((
                TakeOverResult {
                    ownership: Ownership::Agent,
                    user_page: None,
                },
                false,
            ));
        };
        let (user_page, changed) = match self.store.page_of_tab(&tab.id).map_err(db)? {
            Some(page) if page.task_space_id == id => (Some(page.label), false),
            // 被别的 TaskSpace 托管着：不抢
            Some(_) => (None, false),
            None => {
                let label = self.store.next_page_label(id).map_err(db)?;
                self.store
                    .add_page(id, &label, &tab.id, "user")
                    .map_err(db)?;
                (Some(label), true)
            }
        };
        Ok((
            TakeOverResult {
                ownership: Ownership::Agent,
                user_page,
            },
            changed,
        ))
    }

    /// Shell 事件 `ownershipReturned` / `userTookControl`。
    pub(super) fn ownership_event(
        &mut self,
        task_space_id: i64,
        to: Ownership,
        reason: OwnershipReason,
    ) -> Result<bool, Rejection> {
        let row = self
            .store
            .task_space(task_space_id)
            .map_err(super::internal)?
            .ok_or_else(|| {
                Rejection::new(
                    ErrorCode::TaskSpaceNotFound,
                    format!("TaskSpace 不存在：{task_space_id}"),
                )
            })?;
        self.set_owner(&row, to, reason).map_err(super::internal)
    }

    /// ownership 改为 `to`：写库、记进入 user 的时间、推送通知。已是目标值时什么也不做，返回 false。
    fn set_owner(
        &mut self,
        row: &TaskSpaceRow,
        to: Ownership,
        reason: OwnershipReason,
    ) -> anyhow::Result<bool> {
        if row.ownership == owner_str(to) {
            return Ok(false);
        }
        self.store.set_ownership(row.id, owner_str(to))?;
        match to {
            Ownership::User => {
                self.user_since.insert(row.id, Instant::now());
                // 与写库在同一步：该 TaskSpace 的命令与全局的原始 CDP / 转发都在此刻之后中止
                self.takeover.fire(row.id);
            }
            Ownership::Agent => {
                self.user_since.remove(&row.id);
            }
        }
        let changed = OwnershipChanged {
            id: row.id,
            ownership: to,
            reason,
        };
        match serde_json::to_value(&changed) {
            Ok(params) => self.notify(row.id, "taskSpace.ownershipChanged", params),
            Err(e) => tracing::error!(error = %e, "序列化 ownershipChanged 失败"),
        }
        Ok(true)
    }

    /// 进入 user 超过 `threshold` 的 TaskSpace 标记为 inactive；返回是否有变化。
    pub(crate) fn mark_inactive(&mut self, threshold: Duration) -> bool {
        let due: Vec<i64> = self
            .user_since
            .iter()
            .filter(|(_, since)| since.elapsed() >= threshold)
            .map(|(id, _)| *id)
            .collect();
        let mut changed = false;
        for id in due {
            match self.store.mark_inactive(id) {
                Ok(c) => changed |= c,
                Err(e) => tracing::error!(error = %e, task_space_id = id, "标记 inactive 失败"),
            }
        }
        changed
    }
}

fn check_agent(row: &TaskSpaceRow) -> Result<(), OpError> {
    if row.ownership == "user" {
        return Err(OpError::new(
            ErrorCode::UserControl,
            format!("TaskSpace {} 正由用户控制，等用户交还后再继续", row.id),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use alter_core::store::Store;
    use tokio::sync::mpsc;

    use crate::tabs::TabService;

    /// 进程内的 TabService：临时数据目录、没有连 Chromium、通知与请求都没人收。
    fn service(home: &std::path::Path) -> (TabService, Arc<crate::takeover::Takeover>) {
        let store = Store::open(home).unwrap();
        let cdp = crate::cdp::CdpMux::new(home.to_path_buf());
        let (notices, _) = mpsc::unbounded_channel();
        let (requests, _) = mpsc::unbounded_channel();
        let takeover = crate::takeover::Takeover::new();
        let imports = crate::import::Imports::new(requests, home.to_path_buf());
        let svc = TabService::new(store, cdp, notices, takeover.clone(), imports).unwrap();
        (svc, takeover)
    }

    /// N3 CR12：ownership 写成 user 与接管信号在同一个同步步骤里——`task_space_hand_off` 返回时
    /// 全局与该 TaskSpace 的信号都已变化，不等任何异步的通知分发。
    #[test]
    fn hand_off_fires_takeover_in_the_same_step() {
        let home = tempfile::tempdir().unwrap();
        let (mut svc, takeover) = service(home.path());
        let (info, _) = svc.task_space_create("demo", None).unwrap();
        let global = takeover.subscribe_global();
        let own = takeover.subscribe(info.id);

        svc.task_space_hand_off(info.id).unwrap();
        assert!(global.has_changed().unwrap(), "全局接管信号应当已经变化");
        assert!(
            own.has_changed().unwrap(),
            "该 TaskSpace 的接管信号应当已经变化"
        );
    }
}
