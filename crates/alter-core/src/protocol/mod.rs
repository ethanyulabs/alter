//! 协议类型的唯一源头。TypeScript 声明由 `export-ts` 生成到 packages/protocol，禁止手改生成文件。

mod error;
mod gateway;
mod health;
mod shell;

use std::path::Path;

pub use error::ErrorCode;
pub use gateway::{
    DEFAULT_PAGE_BUDGET, DialogInfo, GatewayFile, HelloParams, HelloResult, HelloSpace,
    ImportCounts, ImportDone, ImportItem, ImportProgress, ImportSource, ImportStarted, ImportState,
    ImportStatus, Ownership, OwnershipChanged, OwnershipReason, PROTOCOL_VERSION, Popup, Receipt,
    RpcErrorData, SpaceListResult, SpaceSummary, TakeOverResult, TaskSpaceInfo, TaskSpaceStatus,
    TaskSpaceSummary, TaskSpaceTab, UpdateAvailable,
};
pub use health::HealthFile;
pub use shell::{
    ArchivedTab, ImportedCookie, SPACE_COLORS, SameSite, SearchItem, SearchKind, ShellCommand,
    ShellEvent, ShellState, SpaceView, TabView, TaskSpaceView,
};
use ts_rs::{Config, ExportError, TS};

/// 把全部协议类型导出到 `dir`，每个类型一个 `<类型名>.ts` 文件；
/// ts-rs 不导出常量，预设颜色另写成 `spaceColors.ts`。
pub fn export_ts(dir: &Path) -> Result<(), ExportError> {
    let cfg = Config::new().with_out_dir(dir);
    ErrorCode::export_all(&cfg)?;
    HealthFile::export_all(&cfg)?;
    ShellCommand::export_all(&cfg)?;
    ShellEvent::export_all(&cfg)?;
    for export in [
        GatewayFile::export_all,
        HelloParams::export_all,
        HelloResult::export_all,
        RpcErrorData::export_all,
        SpaceListResult::export_all,
        TaskSpaceInfo::export_all,
        TaskSpaceSummary::export_all,
        TaskSpaceTab::export_all,
        Receipt::export_all,
        OwnershipChanged::export_all,
        TakeOverResult::export_all,
        ImportCounts::export_all,
        ImportStarted::export_all,
        ImportState::export_all,
        ImportProgress::export_all,
        ImportDone::export_all,
        UpdateAvailable::export_all,
    ] {
        export(&cfg)?;
    }
    let colors: Vec<String> = SPACE_COLORS.iter().map(|c| format!("\"{c}\"")).collect();
    std::fs::write(
        dir.join("spaceColors.ts"),
        format!(
            "// 由 alter-core 的 export-ts 生成，禁止手改。\nexport const SPACE_COLORS = [{}] as const;\n",
            colors.join(", ")
        ),
    )?;
    Ok(())
}
