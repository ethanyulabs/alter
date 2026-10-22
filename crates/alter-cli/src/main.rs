//! alter：给用户与 agent 用的命令行。

mod gateway;
mod import;
mod nodejs;
mod spaces;
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
    /// 用 Bun 运行 agent 脚本（-e 或 stdin），预置 taskSpace 等全局函数
    Nodejs {
        /// 要运行的代码；不给时从 stdin 读
        #[arg(short = 'e', long = "eval")]
        code: Option<String>,
    },
    /// 从 Chrome / Arc 导入书签、历史（Cookie 暂不支持）到某个 Space
    Import {
        /// 来源浏览器
        #[arg(long, value_enum)]
        from: Source,
        /// profile 目录；省略时用该浏览器的 Default profile
        #[arg(long)]
        profile: Option<std::path::PathBuf>,
        /// 目标 Space 的 id（见 `alter spaces list`）
        #[arg(long)]
        space: String,
        /// 要导入的类别，逗号分隔；省略时为 bookmarks,history
        #[arg(long, value_enum, value_delimiter = ',')]
        items: Option<Vec<Item>>,
    },
    /// 管理 Space
    Spaces {
        #[command(subcommand)]
        command: SpacesCommand,
    },
}

#[derive(Subcommand)]
enum SpacesCommand {
    /// 列出全部 Space
    List {
        /// 输出 space.list 的结果 JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum Source {
    Chrome,
    Arc,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum Item {
    Bookmarks,
    History,
    Cookies,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Status { json } => status::run(&alter_core::paths::alter_home(), json),
        Command::Nodejs { code } => nodejs::run(&alter_core::paths::alter_home(), code),
        Command::Import {
            from,
            profile,
            space,
            items,
        } => {
            use alter_core::protocol::{ImportItem, ImportSource};
            let from = match from {
                Source::Chrome => ImportSource::Chrome,
                Source::Arc => ImportSource::Arc,
            };
            let items = items.map(|v| {
                v.into_iter()
                    .map(|i| match i {
                        Item::Bookmarks => ImportItem::Bookmarks,
                        Item::History => ImportItem::History,
                        Item::Cookies => ImportItem::Cookies,
                    })
                    .collect()
            });
            import::run(
                &alter_core::paths::alter_home(),
                from,
                profile,
                space,
                items,
            )
        }
        Command::Spaces {
            command: SpacesCommand::List { json },
        } => spaces::list(&alter_core::paths::alter_home(), json),
    }
}
