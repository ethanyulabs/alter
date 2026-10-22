//! 启动时清理已删除 Space 留下的 Chromium partition 目录。
//!
//! 删除 Space 时 Electron 只清空 Cookie / storage（`clearStorageData`）：Chromium 可能还持有
//! 该 partition 的文件句柄，目录要等下次启动、Electron 还没打开它时由 alterd 删除。

use std::collections::HashSet;
use std::path::Path;

/// Electron 的 partition 目录：`<ALTER_HOME>/chromium/Partitions/<partition 名的小写>`。
pub fn dir(home: &Path) -> std::path::PathBuf {
    home.join("chromium").join("Partitions")
}

/// 删除 `Partitions/` 下名为 `space-<id>`、而 id 不属于 `space_ids`（不区分大小写）的目录。
/// 其他目录保持不动；删除失败只记 WARN，不影响启动。
pub fn sweep(home: &Path, space_ids: &[String]) {
    let root = dir(home);
    let Ok(entries) = std::fs::read_dir(&root) else {
        // 还没有 Partitions 目录（首次启动或从未打开过页面）
        return;
    };
    let live: HashSet<String> = space_ids.iter().map(|id| id.to_lowercase()).collect();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(id) = name.to_str().and_then(partition_space_id) else {
            continue;
        };
        if live.contains(&id.to_ascii_lowercase()) {
            continue;
        }
        let path = entry.path();
        match std::fs::remove_dir_all(&path) {
            Ok(()) => tracing::info!(path = %path.display(), "已删除孤儿 partition 目录"),
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "删除孤儿 partition 目录失败")
            }
        }
    }
}

/// `space-<26 位 ULID>` 中的 id（大小写都认，比较时转小写）；其他名字返回 `None`。
fn partition_space_id(name: &str) -> Option<&str> {
    let id = name.strip_prefix("space-")?;
    let ok = id.len() == 26 && id.bytes().all(|b| b.is_ascii_alphanumeric());
    ok.then_some(id)
}
