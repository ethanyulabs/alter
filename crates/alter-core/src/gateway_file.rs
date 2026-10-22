//! `gateway.json`：alterd 写（0600），SDK 与 CLI 读，用来找到 Gateway 的端口与 token。

use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::protocol::GatewayFile;

pub fn path(home: &Path) -> PathBuf {
    home.join("gateway.json")
}

/// 读取；不存在或无法解析时返回 `None`。
pub fn read(home: &Path) -> Option<GatewayFile> {
    let bytes = std::fs::read(path(home)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// 原子写入：先以 0600 写临时文件再 rename，读者不会读到半截内容，也不会有一瞬间权限过宽。
pub fn write(home: &Path, file: &GatewayFile) -> io::Result<()> {
    let tmp = home.join(".gateway.json.tmp");
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)?;
    // 临时文件若是上次残留的，create 不会改它的权限
    f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    f.write_all(&serde_json::to_vec(file)?)?;
    f.sync_all()?;
    std::fs::rename(&tmp, path(home))
}

/// 删除；文件已不存在不算错误。
pub fn remove(home: &Path) -> io::Result<()> {
    match std::fs::remove_file(path(home)) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}
