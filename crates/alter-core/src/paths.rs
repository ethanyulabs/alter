//! 数据目录解析。三个进程（alterd、alter CLI、Electron Main）用同一套规则。

use std::path::PathBuf;

/// 数据目录：环境变量 `ALTER_HOME`，未设置或为空时为 `~/Library/Application Support/alter`。
pub fn alter_home() -> PathBuf {
    if let Some(dir) = std::env::var_os("ALTER_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join("Library/Application Support/alter")
}
