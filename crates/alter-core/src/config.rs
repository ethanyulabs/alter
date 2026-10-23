//! 用户配置 `ALTER_HOME/config.toml`。文件可以不存在；读不了或格式不对时记一条警告并按默认值处理，
//! 不阻止 alterd 启动。

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

/// config.toml 里认识的键；没写的键为 None，由使用方取默认值。
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
pub struct Config {
    /// TaskSpace 交给用户后多久没交还就标记为 inactive（分钟，默认 30）
    pub handoff_timeout_minutes: Option<u64>,
    /// 不活跃的标签多久后自动归档（小时，默认 12）
    pub archive_after_hours: Option<u64>,
    /// agent 页面的下载目录（默认 `~/Downloads`）
    pub download_dir: Option<String>,
    /// 是否定期检查新版本（默认 true）
    pub update_check: Option<bool>,
}

/// 配置文件的位置。
pub fn path(home: &Path) -> PathBuf {
    home.join("config.toml")
}

impl Config {
    /// 读 `ALTER_HOME/config.toml`；返回配置与（若有）读取或解析失败的原因。
    pub fn load(home: &Path) -> (Config, Option<String>) {
        let file = path(home);
        let text = match std::fs::read_to_string(&file) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (Config::default(), None),
            Err(e) => {
                return (
                    Config::default(),
                    Some(format!("读取 {} 失败：{e}", file.display())),
                );
            }
        };
        match Config::parse(&text) {
            Ok(c) => (c, None),
            Err(e) => (
                Config::default(),
                Some(format!("{} 格式不对：{e}", file.display())),
            ),
        }
    }

    pub fn parse(text: &str) -> Result<Config, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    /// 交还超时：`ALTER_HANDOFF_TIMEOUT_MS`（测试用）优先，其次 `handoff_timeout_minutes`，默认 30 分钟。
    pub fn handoff_timeout(&self) -> Duration {
        env_ms("ALTER_HANDOFF_TIMEOUT_MS")
            .or(self
                .handoff_timeout_minutes
                .map(|m| Duration::from_secs(m * 60)))
            .unwrap_or(Duration::from_secs(30 * 60))
    }

    /// 自动归档阈值：`ALTER_ARCHIVE_AFTER_MS`（测试用）优先，其次 `archive_after_hours`，默认 12 小时。
    pub fn archive_after(&self) -> Duration {
        env_ms("ALTER_ARCHIVE_AFTER_MS")
            .or(self
                .archive_after_hours
                .map(|h| Duration::from_secs(h * 3600)))
            .unwrap_or(Duration::from_secs(12 * 3600))
    }

    /// 下载目录：`ALTER_DOWNLOAD_DIR` 优先，其次 `download_dir`（`~/` 开头时按家目录展开），默认 `~/Downloads`。
    pub fn download_dir(&self) -> PathBuf {
        if let Some(dir) = std::env::var_os("ALTER_DOWNLOAD_DIR").filter(|d| !d.is_empty()) {
            return PathBuf::from(dir);
        }
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        match self.download_dir.as_deref() {
            Some(dir) if dir.starts_with("~/") => home.join(&dir[2..]),
            Some(dir) if !dir.is_empty() => PathBuf::from(dir),
            _ => home.join("Downloads"),
        }
    }
}

/// 以毫秒给出时长的环境变量；没设或不是数字时为 None。
pub fn env_ms(name: &str) -> Option<Duration> {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(Duration::from_millis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_keys_and_ignores_unknown() {
        let c = Config::parse("handoff_timeout_minutes = 5\narchive_after_hours = 2\ndownload_dir = \"/tmp/d\"\nother = 1\n").unwrap();
        assert_eq!(c.handoff_timeout_minutes, Some(5));
        assert_eq!(c.archive_after_hours, Some(2));
        assert_eq!(c.download_dir.as_deref(), Some("/tmp/d"));
        assert_eq!(Config::parse("").unwrap(), Config::default());
        assert!(Config::parse("handoff_timeout_minutes = \"x\"").is_err());
    }

    #[test]
    fn missing_file_is_default() {
        let dir = std::env::temp_dir().join(format!("alter-config-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(Config::load(&dir), (Config::default(), None));
        std::fs::write(path(&dir), "handoff_timeout_minutes = [").unwrap();
        let (c, warn) = Config::load(&dir);
        assert_eq!(c, Config::default());
        assert!(warn.is_some());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
