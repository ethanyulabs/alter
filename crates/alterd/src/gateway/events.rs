//! 推给 SDK 的页面事件：连接登记关注的 TaskSpace，TabService 与 CDP 事件产生的通知按 TaskSpace 分发；
//! 对话框与下载由这里跟着 CDP 事件维护。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use alter_core::protocol::DialogInfo;
use serde_json::{Value, json};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;

use super::Gateway;
use crate::tabs::Notice;

/// 一个连接：它关注的 TaskSpace（请求里出现过的 `id`）与写出队列。
pub(super) struct Listener {
    pub interest: Arc<Mutex<HashSet<i64>>>,
    pub out: mpsc::UnboundedSender<String>,
}

/// 把通知发给关注该 TaskSpace 的连接；顺带移除已断开的连接。
pub(super) async fn dispatch(gw: Arc<Gateway>, mut notices: mpsc::UnboundedReceiver<Notice>) {
    while let Some(n) = notices.recv().await {
        // 转为用户控制：隐藏这个 TaskSpace 页面上的叠层（命令的中止已由 TabService 同步发出）
        if n.method == "taskSpace.ownershipChanged" && n.params["ownership"] == "user" {
            tokio::spawn(hide_overlays(gw.clone(), n.task_space_id));
        }
        let line = json!({"jsonrpc": "2.0", "method": n.method, "params": n.params}).to_string();
        gw.listeners
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|l| {
                let wants = l
                    .interest
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .contains(&n.task_space_id);
                if wants {
                    l.out.send(line.clone()).is_ok()
                } else {
                    !l.out.is_closed()
                }
            });
    }
}

/// 转为用户控制：隐藏该 TaskSpace 各页面上的 agent 叠层。
async fn hide_overlays(gw: Arc<Gateway>, task_space_id: i64) {
    let Ok(targets) = gw
        .with_tabs(move |t| (t.task_space_targets(task_space_id), false))
        .await
    else {
        return;
    };
    for target in targets {
        if let Ok(sid) = gw.cdp.session(&target).await
            && let Err(e) = crate::cdp::overlay::hide(&gw.cdp, &sid, &target).await
        {
            tracing::warn!(error = %e, task_space_id, "隐藏 agent 叠层失败");
        }
    }
}

/// 在 TabService 里按 targetId 找到 Page 并推送通知；不是 agent 页面时什么也不做。
async fn notify_target(gw: &Gateway, target: String, method: &'static str, params: Value) {
    let _ = gw
        .with_tabs(move |t| {
            if let Some((id, label)) = t.page_of_target(&target) {
                let mut params = params;
                params["label"] = json!(label);
                t.notify(id, method, params);
            }
            ((), false)
        })
        .await;
}

/// 跟着 CDP 事件记下哪些页面会话正开着 JS 对话框，打开时推送 `page.dialog`。
pub(super) async fn track_dialogs(gw: Arc<Gateway>) {
    let mut events = gw.cdp.subscribe();
    loop {
        let ev = match events.recv().await {
            Ok(ev) => ev,
            Err(RecvError::Lagged(_)) => continue,
            Err(RecvError::Closed) => return,
        };
        let Some(sid) = ev.session_id else { continue };
        match ev.method.as_str() {
            "Page.javascriptDialogOpening" => {
                let dialog = DialogInfo {
                    kind: ev.params["type"].as_str().unwrap_or("alert").to_owned(),
                    message: ev.params["message"].as_str().unwrap_or("").to_owned(),
                };
                let params = json!({"type": dialog.kind, "message": dialog.message});
                gw.dialogs
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(sid.clone(), dialog);
                if let Some(target) = gw.cdp.target_of(&sid).await {
                    notify_target(&gw, target, "page.dialog", params).await;
                }
            }
            "Page.javascriptDialogClosed" => {
                gw.dialogs
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&sid);
            }
            _ => {}
        }
    }
}

/// 跟着 `downloadWillBegin` / `downloadProgress`（`Page.` 或 `Browser.` 域，同一下载只处理一次）
/// 处理 agent 页面的下载：完成后把文件从 guid 改名为建议文件名（重名时加序号），推送 `page.download`。
pub(super) async fn track_downloads(gw: Arc<Gateway>) {
    let mut events = gw.cdp.subscribe();
    // guid → (frameId, url, 建议文件名)
    let mut pending: HashMap<String, (String, String, String)> = HashMap::new();
    loop {
        let ev = match events.recv().await {
            Ok(ev) => ev,
            Err(RecvError::Lagged(_)) => continue,
            Err(RecvError::Closed) => return,
        };
        let p = &ev.params;
        let guid = p["guid"].as_str().unwrap_or("").to_owned();
        let method = ev
            .method
            .strip_prefix("Page.")
            .or_else(|| ev.method.strip_prefix("Browser."))
            .unwrap_or("");
        match method {
            "downloadWillBegin" => {
                pending.insert(
                    guid,
                    (
                        p["frameId"].as_str().unwrap_or("").to_owned(),
                        p["url"].as_str().unwrap_or("").to_owned(),
                        p["suggestedFilename"]
                            .as_str()
                            .unwrap_or("download")
                            .to_owned(),
                    ),
                );
            }
            "downloadProgress" if p["state"] == "canceled" => {
                pending.remove(&guid);
            }
            "downloadProgress" if p["state"] == "completed" => {
                let Some((frame, url, name)) = pending.remove(&guid) else {
                    continue;
                };
                let path = match finish_download(&gw.download_dir, &guid, &name) {
                    Ok(path) => path,
                    Err(e) => {
                        tracing::error!(error = %e, guid, "整理下载文件失败");
                        continue;
                    }
                };
                // 主框架的 frameId 就是页面的 targetId；子框架触发的下载不归到页面
                let params = json!({
                    "url": url,
                    "suggestedFilename": name,
                    "path": path.to_string_lossy(),
                });
                notify_target(&gw, frame, "page.download", params).await;
            }
            _ => {}
        }
    }
}

/// 把 `<dir>/<guid>` 改名为 `<dir>/<name>`；已有同名文件时用 `<stem> (n).<ext>`。
fn finish_download(dir: &Path, guid: &str, name: &str) -> std::io::Result<PathBuf> {
    // 建议文件名来自网页，只取最后一段，不让它跳出下载目录
    let name = Path::new(name)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "download".to_owned());
    let mut target = dir.join(&name);
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_owned(), format!(".{e}")),
        _ => (name.clone(), String::new()),
    };
    let mut n = 1;
    while target.exists() {
        target = dir.join(format!("{stem} ({n}){ext}"));
        n += 1;
    }
    std::fs::rename(dir.join(guid), &target)?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finish_download_renames_and_avoids_overwrite() {
        let dir = std::env::temp_dir().join(format!("alter-dl-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("g1"), "a").unwrap();
        std::fs::write(dir.join("g2"), "b").unwrap();
        assert_eq!(
            finish_download(&dir, "g1", "r.txt").unwrap(),
            dir.join("r.txt")
        );
        assert_eq!(
            finish_download(&dir, "g2", "../r.txt").unwrap(),
            dir.join("r (1).txt")
        );
        assert_eq!(std::fs::read_to_string(dir.join("r (1).txt")).unwrap(), "b");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
