//! 协议类型的唯一源头。TypeScript 声明由 `export-ts` 生成到 packages/protocol，禁止手改生成文件。

mod error;
mod health;
mod shell;

use std::path::Path;

pub use error::ErrorCode;
pub use health::HealthFile;
pub use shell::{ShellCommand, ShellEvent, ShellState, SpaceView, TabView};
use ts_rs::{Config, ExportError, TS};

/// 把全部协议类型导出到 `dir`，每个类型一个 `<类型名>.ts` 文件。
pub fn export_ts(dir: &Path) -> Result<(), ExportError> {
    let cfg = Config::new().with_out_dir(dir);
    ErrorCode::export_all(&cfg)?;
    HealthFile::export_all(&cfg)?;
    ShellCommand::export_all(&cfg)?;
    ShellEvent::export_all(&cfg)?;
    Ok(())
}
