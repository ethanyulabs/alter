//! 从 macOS 钥匙串取来源浏览器的 Safe Storage 口令（用来派生 Cookie 的加密密钥）。
//! 读的是用户本机自己的浏览器条目，macOS 会弹授权框；口令只在内存里，不写日志。
//! debug 构建先看 `ALTER_KEYCHAIN_FAKE`：值为 `denied` 时当作拒绝授权，否则当作取到的口令，
//! 测试因此不会弹出真实的授权框。release 构建忽略它。

use alter_core::protocol::ImportSource;

/// 钥匙串里的条目：(服务名, 账户名)。
fn entry(source: ImportSource) -> (&'static str, &'static str) {
    match source {
        ImportSource::Chrome => ("Chrome Safe Storage", "Chrome"),
        ImportSource::Arc => ("Arc Safe Storage", "Arc"),
    }
}

/// 取口令；用户拒绝授权、没有这一项或读取出错时返回 None。会阻塞（等用户点授权框），要在阻塞线程里调用。
pub(super) fn safe_storage_password(source: ImportSource) -> Option<Vec<u8>> {
    let (service, account) = entry(source);
    let password = lookup(service, account);
    tracing::info!(service, ok = password.is_some(), "取 Safe Storage 口令");
    password
}

#[cfg(debug_assertions)]
fn lookup(service: &str, account: &str) -> Option<Vec<u8>> {
    match std::env::var("ALTER_KEYCHAIN_FAKE") {
        Ok(v) if v == "denied" => None,
        Ok(v) => Some(v.into_bytes()),
        Err(_) => from_keychain(service, account),
    }
}

#[cfg(not(debug_assertions))]
fn lookup(service: &str, account: &str) -> Option<Vec<u8>> {
    from_keychain(service, account)
}

fn from_keychain(service: &str, account: &str) -> Option<Vec<u8>> {
    match security_framework::passwords::get_generic_password(service, account) {
        Ok(password) => Some(password),
        Err(e) => {
            // 只记 OSStatus：errSecUserCanceled / errSecAuthFailed / errSecItemNotFound 等都按拒绝处理
            tracing::warn!(service, code = e.code(), "钥匙串没有给出口令");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// release 构建按 (服务名, 账户名) 查真实钥匙串：两者都要与浏览器写入的一致。
    #[test]
    fn keychain_entries_match_browsers() {
        assert_eq!(
            entry(ImportSource::Chrome),
            ("Chrome Safe Storage", "Chrome")
        );
        assert_eq!(entry(ImportSource::Arc), ("Arc Safe Storage", "Arc"));
    }
}
