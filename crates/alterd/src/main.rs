//! alterd：alter 的守护进程。

mod archive;
mod cdp;
mod gateway;
mod import;
mod lifecycle;
mod partitions;
mod shell;
mod tabs;
mod takeover;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "alterd", version, about = "alter 守护进程")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 在数据目录（ALTER_HOME）上启动守护进程，直到收到终止信号
    Run {
        /// stdin 读到 EOF（宿主关闭管道或已退出）时按 SIGTERM 的流程退出
        #[arg(long)]
        exit_on_stdin_close: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    // JSON 行日志写 stderr；日志文件与滚动留给网关需求。
    // 宿主被强制结束后 stderr 管道已断：写失败时不能再用 eprintln! 报告，否则会 panic，跳过退出清理
    tracing_subscriber::fmt()
        .json()
        .with_writer(std::io::stderr)
        .log_internal_errors(false)
        .init();
    match cli.command {
        Command::Run {
            exit_on_stdin_close,
        } => match lifecycle::run(&alter_core::paths::alter_home(), exit_on_stdin_close) {
            Ok(code) => code,
            Err(e) => {
                // stderr 只输出 JSON 行；already running 的纯文本提示是另一条路径，属于契约
                tracing::error!(error = %format!("{e:#}"), code = 1, "alterd 异常退出");
                ExitCode::from(1)
            }
        },
    }
}
