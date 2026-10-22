//! 来源 profile 的位置与检查：各浏览器的数据根目录、默认 profile、可导入的 profile 列表。

use std::path::{Path, PathBuf};

use alter_core::protocol::{ErrorCode, ImportSource};

use crate::tabs::OpError;

/// profile 里会读取的数据文件。
const DATA_FILES: [&str; 3] = ["Bookmarks", "History", "Cookies"];

/// 来源浏览器的数据根目录：`ALTER_CHROME_DIR` / `ALTER_ARC_DIR` 优先（测试用），
/// 否则为 `~/Library/Application Support` 下的默认位置。
pub(super) fn root(source: ImportSource) -> PathBuf {
    let (var, default) = match source {
        ImportSource::Chrome => ("ALTER_CHROME_DIR", "Google/Chrome"),
        ImportSource::Arc => ("ALTER_ARC_DIR", "Arc/User Data"),
    };
    if let Some(dir) = std::env::var_os(var) {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join("Library/Application Support").join(default)
}

/// 要导入的 profile 目录：省略时为数据根目录下的 `Default`。目录不存在或其中三个数据文件
/// 都不存在时为 `E_IMPORT_SOURCE_NOT_FOUND`。
pub(super) fn profile_dir(
    source: ImportSource,
    profile_path: Option<&str>,
) -> Result<PathBuf, OpError> {
    let dir = match profile_path {
        Some(path) => PathBuf::from(path),
        None => root(source).join("Default"),
    };
    if !has_data(&dir) {
        return Err(OpError::new(
            ErrorCode::ImportSourceNotFound,
            format!("找不到可导入的 profile：{}", dir.display()),
        ));
    }
    Ok(dir)
}

/// 数据根目录下可导入的 profile：含任一数据文件的子目录名，按名称排序；根目录不存在时为空。
pub(crate) fn list(source: ImportSource) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root(source)) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|e| has_data(&e.path()))
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    names.sort();
    names
}

/// 目录里至少有一个数据文件。
fn has_data(dir: &Path) -> bool {
    dir.is_dir() && DATA_FILES.iter().any(|f| dir.join(f).is_file())
}
