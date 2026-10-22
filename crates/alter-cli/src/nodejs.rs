//! `alter nodejs [-e <code>]`：用 Bun 运行脚本，预置 SDK 的全局函数（`bun --preload <runtime> <script>`）。
//! 退出码与脚本相同；没有 gateway.json、或它指向的端口连不上时不启动 Bun，以退出码 2 结束。

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::gateway::NOT_RUNNING;

/// 开发时 SDK runtime 在仓库里的位置（编译期确定）。
const DEV_RUNTIME: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../packages/sdk/src/runtime.ts"
);

pub fn run(home: &Path, code: Option<String>) -> ExitCode {
    let Some(gw) = alter_core::gateway_file::read(home) else {
        eprintln!(
            "{NOT_RUNNING}（{} 不存在）",
            alter_core::gateway_file::path(home).display()
        );
        return ExitCode::from(2);
    };
    // 文件可能是异常退出的 alterd 留下的：先确认端口有人监听
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], gw.port));
    if std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(1)).is_err() {
        eprintln!("{NOT_RUNNING}（端口 {} 连不上）", gw.port);
        return ExitCode::from(2);
    }
    let source = match code {
        Some(code) => code,
        None => {
            let mut buf = String::new();
            if let Err(e) = std::io::stdin().read_to_string(&mut buf) {
                eprintln!("读取 stdin 失败：{e}");
                return ExitCode::from(1);
            }
            buf
        }
    };
    let Some(bun) = find_bun() else {
        eprintln!("找不到 bun：设置 ALTER_BUN_PATH，或把 bun 放进 PATH");
        return ExitCode::from(1);
    };
    let Some(runtime) = find_runtime() else {
        eprintln!("找不到 SDK runtime：设置 ALTER_SDK_RUNTIME");
        return ExitCode::from(1);
    };
    // 脚本写进临时目录：-e 与 stdin 两种来源一样处理，报错行号与脚本一致
    let dir = std::env::temp_dir().join(format!("alter-nodejs-{}-{}", std::process::id(), nanos()));
    let script = dir.join("script.ts");
    if let Err(e) = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&script, source)) {
        eprintln!("写临时脚本失败：{e}");
        return ExitCode::from(1);
    }
    let status = Command::new(&bun)
        .arg("--preload")
        .arg(&runtime)
        .arg(&script)
        .env("ALTER_HOME", home)
        .status();
    let _ = std::fs::remove_dir_all(&dir);
    match status {
        Ok(s) => ExitCode::from(s.code().map_or(1, |c| c.clamp(0, 255) as u8)),
        Err(e) => {
            eprintln!("启动 bun（{}）失败：{e}", bun.display());
            ExitCode::from(1)
        }
    }
}

fn nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

/// `ALTER_BUN_PATH` → 与 `alter` 同目录的 `bun`（打包后）→ PATH。
fn find_bun() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("ALTER_BUN_PATH") {
        return Some(PathBuf::from(p));
    }
    if let Some(sibling) = std::env::current_exe()
        .ok()
        .map(|e| e.with_file_name("bun"))
        && sibling.is_file()
    {
        return Some(sibling);
    }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|d| d.join("bun"))
            .find(|p| p.is_file())
    })
}

/// `ALTER_SDK_RUNTIME` → `<alter 所在目录>/../sdk/runtime.js`（打包后）→ 仓库内的 runtime.ts（开发时）。
fn find_runtime() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("ALTER_SDK_RUNTIME") {
        return Some(PathBuf::from(p));
    }
    let packaged = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join("../sdk/runtime.js")));
    if let Some(p) = packaged
        && p.is_file()
    {
        return Some(p);
    }
    let dev = PathBuf::from(DEV_RUNTIME);
    dev.is_file().then_some(dev)
}
