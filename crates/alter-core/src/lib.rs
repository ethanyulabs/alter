//! alter-core：协议类型、SQLite 存储与迁移，供 alterd 与 alter CLI 共用。

pub mod health;
pub mod paths;
pub mod protocol;
pub mod store;

/// 当前 crate 的版本号，来自 Cargo workspace 的 `version`。
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_matches_workspace() {
        assert_eq!(version(), "0.1.0");
    }
}
