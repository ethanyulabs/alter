//! `alter import`：经 Gateway 运行导入，等到 `import.done` 后打印各类计数。

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use alter_core::protocol::{ImportDone, ImportItem, ImportSource, ImportStatus};
use serde_json::json;

use crate::gateway::{self, CallError, Session};

/// 等 `import.done` 的上限：大的 profile 也应在这之内结束。
const WAIT: Duration = Duration::from_secs(600);

pub fn run(
    home: &Path,
    from: ImportSource,
    profile: Option<PathBuf>,
    space: String,
    items: Option<Vec<ImportItem>>,
) -> ExitCode {
    // alterd 的工作目录与这里不同：相对路径先按当前目录变成绝对路径
    let profile = profile
        .map(|p| std::path::absolute(&p).unwrap_or(p))
        .map(|p| p.to_string_lossy().into_owned());
    let mut session = match Session::connect(home) {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    let mut params = json!({"source": from, "spaceId": space});
    if let Some(p) = profile {
        params["profilePath"] = json!(p);
    }
    if let Some(items) = items {
        params["items"] = json!(items);
    }
    let import_id = match session.request("import.run", params) {
        Ok(v) => v["importId"].clone(),
        Err(e) => return fail(e),
    };
    session.set_timeout(WAIT);
    let done = loop {
        match session.notification() {
            Ok((method, params)) if method == "import.done" && params["importId"] == import_id => {
                break params;
            }
            Ok(_) => continue,
            Err(e) => return fail(e),
        }
    };
    session.close();
    let Ok(done) = serde_json::from_value::<ImportDone>(done) else {
        eprintln!("无法解析 import.done");
        return ExitCode::from(1);
    };
    let c = done.counts;
    if done.status == ImportStatus::Failed {
        eprintln!("导入失败（import {}），见 alter 的日志", done.import_id);
        return ExitCode::from(1);
    }
    println!(
        "imported bookmarks={} history={} cookies={} skipped={}",
        c.bookmarks, c.history, c.cookies, c.skipped
    );
    if let Some(w) = done.warning {
        eprintln!("warning: {w}");
    }
    ExitCode::SUCCESS
}

/// 连不上为 2，其余错误为 1。
fn fail(e: CallError) -> ExitCode {
    match e {
        CallError::Unreachable(why) => {
            eprintln!("{}（{why}）", gateway::NOT_RUNNING);
            ExitCode::from(2)
        }
        CallError::Rpc { code, message } => {
            eprintln!("{code}: {message}");
            ExitCode::from(1)
        }
    }
}
