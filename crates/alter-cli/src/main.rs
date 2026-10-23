//! alter：给用户与 agent 用的命令行。

mod exe;
mod gateway;
mod import;
mod nodejs;
mod skill;
mod spaces;
mod status;
mod upgrade;

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
    /// 从 Chrome / Arc 导入书签、历史与 Cookie 到某个 Space（Cookie 要从钥匙串取口令，macOS 会请求授权）
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
        /// 要导入的类别（bookmarks、history、cookies），逗号分隔；省略时为 bookmarks,history
        #[arg(long, value_enum, value_delimiter = ',')]
        items: Option<Vec<Item>>,
    },
    /// 管理 Space
    Spaces {
        #[command(subcommand)]
        command: SpacesCommand,
    },
    /// 升级到 GitHub 上的最新版本（经 install.sh）
    Upgrade {
        /// 只检查并打印比较结果，不升级
        #[arg(long)]
        check: bool,
        /// 与 --check 一起用：以单行 JSON 输出
        #[arg(long, requires = "check")]
        json: bool,
        /// 升级到指定版本（如 v0.2.0）
        #[arg(long)]
        version: Option<String>,
    },
    /// 安装或移除给 AI agent 用的 alter-browser skill
    Skill {
        #[command(subcommand)]
        command: SkillCommand,
    },
}

#[derive(Subcommand)]
enum SkillCommand {
    /// 把 alter-browser 复制到 agent 的 skills 目录（默认 ~/.claude/skills 与 ~/.codex/skills 中已存在的）
    Install {
        /// 目标 skills 目录，可重复
        #[arg(long)]
        target: Vec<std::path::PathBuf>,
    },
    /// 删除 alter 装进去的 alter-browser（只删带 .alter-skill 标记的）
    Uninstall {
        /// 目标 skills 目录，可重复
        #[arg(long)]
        target: Vec<std::path::PathBuf>,
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
        Command::Upgrade {
            check,
            json,
            version,
        } => upgrade::run(check, json, version),
        Command::Skill {
            command: SkillCommand::Install { target },
        } => skill::install(target),
        Command::Skill {
            command: SkillCommand::Uninstall { target },
        } => skill::uninstall(target),
    }
}
