//! export-ts：把协议类型导出为 TypeScript 声明。
//!
//! 用法：`export-ts <目录>`；目录不存在会被创建。成功退出 0，失败打印原因并退出 1。

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(dir) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("用法：export-ts <目录>");
        return ExitCode::from(1);
    };
    match alter_core::protocol::export_ts(&dir) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("export-ts: 导出到 {} 失败：{e}", dir.display());
            ExitCode::from(1)
        }
    }
}
