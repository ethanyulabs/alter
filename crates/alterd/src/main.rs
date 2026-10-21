//! alterd：alter 的守护进程。

mod lifecycle;

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
    Run,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    // JSON 行日志写 stderr；日志文件与滚动留给网关需求
    tracing_subscriber::fmt()
        .json()
        .with_writer(std::io::stderr)
        .init();
    match cli.command {
        Command::Run => match lifecycle::run(&alter_core::paths::alter_home()) {
            Ok(code) => code,
            Err(e) => {
                // stderr 只输出 JSON 行；already running 的纯文本提示是另一条路径，属于契约
                tracing::error!(error = %format!("{e:#}"), code = 1, "alterd 异常退出");
                ExitCode::from(1)
            }
        },
    }
}
