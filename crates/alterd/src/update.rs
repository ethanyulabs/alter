//! 检查新版本：alterd 启动后（`ALTER_UPDATE_CHECK_DELAY_MS`，默认 5 秒）与之后每 24 小时，
//! 用系统的 curl 取 Release API 的 latest；比自身新时经 Gateway 推送 `app.updateAvailable`，
//! 并写进 Shell 快照给侧边栏显示。检查失败只记 WARN。
//! debug 构建只在设置了 `ALTER_RELEASE_API` 时检查：开发与测试不联网。

use std::sync::Arc;
use std::time::Duration;

use alter_core::protocol::UpdateAvailable;

use crate::gateway::Gateway;

const DEFAULT_API: &str = "https://api.github.com/repos/ethanyulabs/alter";
const CURRENT: &str = env!("CARGO_PKG_VERSION");
const INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// 按配置开始定期检查；`enabled` 为 false（config.toml 的 `update_check = false`）时什么也不做。
pub fn spawn(gateway: Arc<Gateway>, enabled: bool) {
    if !enabled {
        return;
    }
    let api = match std::env::var("ALTER_RELEASE_API") {
        Ok(api) => api,
        Err(_) if cfg!(debug_assertions) => return,
        Err(_) => DEFAULT_API.to_owned(),
    };
    let delay = std::env::var("ALTER_UPDATE_CHECK_DELAY_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map_or(Duration::from_secs(5), Duration::from_millis);
    tokio::spawn(async move {
        tokio::time::sleep(delay).await;
        loop {
            let url = format!("{api}/releases/latest");
            match tokio::task::spawn_blocking(move || check(&url)).await {
                Ok(Ok(Some(update))) => publish(&gateway, update).await,
                Ok(Ok(None)) => {}
                Ok(Err(e)) => tracing::warn!(error = %e, "检查更新失败"),
                Err(e) => tracing::warn!(error = %e, "检查更新失败"),
            }
            tokio::time::sleep(INTERVAL).await;
        }
    });
}

/// 推给 Gateway 的连接，并写进快照。
async fn publish(gateway: &Arc<Gateway>, update: UpdateAvailable) {
    tracing::info!(version = %update.version, "有新版本");
    gateway.announce_update(update.clone());
    let _ = gateway
        .with_tabs(move |t| {
            let changed = t.set_update(update);
            ((), changed)
        })
        .await;
}

/// 取 latest；比自身新时返回它。
fn check(url: &str) -> Result<Option<UpdateAvailable>, String> {
    // -q 必须是第一个参数：不读用户的 .curlrc，请求只带 User-Agent
    let out = std::process::Command::new("/usr/bin/curl")
        .args(["-q", "-fsSL", "--max-time", "10", "-A"])
        .arg(format!("alter/{CURRENT}"))
        .arg(url)
        .output()
        .map_err(|e| format!("启动 curl 失败：{e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{url}：{}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(|e| format!("返回的不是 JSON：{e}"))?;
    let tag = v["tag_name"].as_str().ok_or("结果里没有 tag_name")?;
    let version = tag.trim_start_matches('v');
    let latest = parse(version).ok_or_else(|| format!("版本号不是三段数字：{tag}"))?;
    let current = parse(CURRENT).ok_or("自身版本号无法解析")?;
    if latest <= current {
        return Ok(None);
    }
    // 只接受 https（以及本机测试用的 http://127.0.0.1）：这个地址会被侧边栏打开
    let url = v["html_url"].as_str().unwrap_or_default();
    if !allowed_url(url) {
        return Err(format!("html_url 不是 https 地址：{url:?}"));
    }
    Ok(Some(UpdateAvailable {
        version: version.to_owned(),
        url: url.to_owned(),
    }))
}

/// 侧边栏会打开的 Release 地址：只接受 https，以及本机测试用的 http://127.0.0.1。
fn allowed_url(url: &str) -> bool {
    url.starts_with("https://")
        || url.starts_with("http://127.0.0.1:")
        || url.starts_with("http://127.0.0.1/")
}

/// 三段整数的版本号（与 alter-cli 的 upgrade.rs 各写一份：alter-core 不该依赖进程调用）。
fn parse(v: &str) -> Option<(u64, u64, u64)> {
    let mut it = v.split('.').map(|p| p.parse::<u64>().ok());
    let r = (it.next()??, it.next()??, it.next()??);
    it.next().is_none().then_some(r)
}

#[cfg(test)]
mod tests {
    use super::{allowed_url, parse};

    #[test]
    fn accepts_https_and_exact_loopback_only() {
        assert!(allowed_url(
            "https://github.com/ethanyulabs/alter/releases/tag/v0.2.0"
        ));
        assert!(allowed_url("http://127.0.0.1:8080/release"));
        assert!(allowed_url("http://127.0.0.1/release"));
        assert!(!allowed_url("http://127.0.0.1.evil.com/x"));
        assert!(!allowed_url("http://example.com/x"));
        assert!(!allowed_url("javascript:alert(1)"));
        assert!(!allowed_url(""));
    }

    #[test]
    fn parses_three_part_versions_only() {
        assert_eq!(parse("0.10.2"), Some((0, 10, 2)));
        assert_eq!(parse("1.2"), None);
        assert_eq!(parse("1.2.3.4"), None);
        assert_eq!(parse("1.2.x"), None);
        assert!(parse("0.10.0") > parse("0.9.9"));
    }
}
