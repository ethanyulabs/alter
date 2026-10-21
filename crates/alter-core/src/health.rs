//! 健康文件 `alterd.json`：alterd 写，CLI 与 Electron Main 只读。

use std::io;
use std::path::{Path, PathBuf};

use crate::protocol::HealthFile;

/// 数据目录下健康文件的路径。
pub fn path(home: &Path) -> PathBuf {
    home.join("alterd.json")
}

/// 读取健康文件；不存在或内容无法解析时返回 `None`。
pub fn read(home: &Path) -> Option<HealthFile> {
    let bytes = std::fs::read(path(home)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// 原子地写入健康文件：先写临时文件再 rename，读者不会读到半截内容。
pub fn write(home: &Path, health: &HealthFile) -> io::Result<()> {
    let tmp = home.join(".alterd.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(health)?)?;
    std::fs::rename(&tmp, path(home))
}

/// 删除健康文件；文件已不存在不算错误。
pub fn remove(home: &Path) -> io::Result<()> {
    match std::fs::remove_file(path(home)) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// 判断 pid 对应的进程是否存活：`kill(pid, 0)` 成功，或因权限不足失败（进程存在但属于别人）。
/// pid 复用会让已退出的 alterd 被误判为存活，见 design.md 风险表。
pub fn is_alive(pid: u32) -> bool {
    // pid 0 表示进程组，超出 pid_t 范围的值不可能是真实进程
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    if pid == 0 {
        return false;
    }
    // SAFETY：信号 0 只做存在性与权限检查，不会投递信号
    if unsafe { libc::kill(pid, 0) } == 0 {
        return true;
    }
    io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}
