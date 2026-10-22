//! Gateway 的 `import.run` / `import.status`：进度转成通知，推给发起导入的连接。

use alter_core::protocol::{ImportItem, ImportSource};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::mpsc;

use super::Gateway;
use super::rpc::{RpcError, parse};
use crate::import::{Event, StartRequest};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunParams {
    source: ImportSource,
    #[serde(default)]
    profile_path: Option<String>,
    space_id: String,
    #[serde(default)]
    items: Option<Vec<ImportItem>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatusParams {
    import_id: i64,
}

/// `import.run`：开始导入，立即返回 `{importId}`；之后的 `import.progress` / `import.done` 写进 `out`。
pub(super) async fn run(
    gw: &Gateway,
    params: Value,
    out: &mpsc::UnboundedSender<String>,
) -> Result<Value, RpcError> {
    let p: RunParams = parse(params)?;
    let (tx, mut rx) = mpsc::unbounded_channel();
    let id = gw
        .imports
        .start(
            StartRequest {
                source: p.source,
                profile_path: p.profile_path,
                space_id: p.space_id,
                items: p.items,
            },
            tx,
        )
        .await?;
    // alterd 跑在单线程运行时上：从这里返回到响应进入写出队列之间没有让出点，
    // 导入任务与这个转发任务都要等之后才运行，所以通知一定排在响应之后
    let out = out.clone();
    tokio::spawn(async move {
        while let Some(ev) = rx.recv().await {
            let (method, params) = match ev {
                Event::Progress(p) => ("import.progress", serde_json::to_value(p)),
                Event::Done(d) => ("import.done", serde_json::to_value(d)),
            };
            let Ok(params) = params else { continue };
            let line = json!({"jsonrpc": "2.0", "method": method, "params": params}).to_string();
            if out.send(line).is_err() {
                break;
            }
        }
    });
    Ok(json!({ "importId": id }))
}

/// `import.status`：状态与计数。
pub(super) async fn status(gw: &Gateway, params: Value) -> Result<Value, RpcError> {
    let p: StatusParams = parse(params)?;
    let state = gw.imports.status(p.import_id).await?;
    serde_json::to_value(state).map_err(RpcError::internal)
}
