//! `alter` 自身的位置。安装后用户经 `~/.local/bin/alter` 这个符号链接调用它，
//! macOS 上 `current_exe()` 返回的是链接本身；找同目录的 bun、`../sdk`、`../skills` 要用解析后的真实路径。

use std::path::PathBuf;

/// `alter` 可执行文件的真实路径（解析掉符号链接）；拿不到时为 None。
pub fn real_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(std::fs::canonicalize(&exe).unwrap_or(exe))
}
