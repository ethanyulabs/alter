use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// alterd 写在数据目录下的健康文件 `alterd.json`，CLI 与 Electron Main 只读。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HealthFile {
    pub version: String,
    pub pid: u32,
    /// 启动时间，UTC 毫秒；JSON 里是数字，不是 bigint
    #[ts(type = "number")]
    pub started_at: i64,
    pub db_path: String,
}
