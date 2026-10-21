//! alter：给用户与 agent 用的命令行。

mod status;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "alter", version, about = "alter 浏览器的命令行")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 查看 alterd 守护进程是否在运行
    Status {
        /// 以单行 JSON 输出
        #[arg(long)]
        json: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Status { json } => status::run(&alter_core::paths::alter_home(), json),
    }
}
