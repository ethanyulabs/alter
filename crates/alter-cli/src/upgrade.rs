//! `alter upgrade [--check] [--json] [--version v<x.y.z>]`：从 Release API 取最新版本并与自身比较；
//! 有更新时下载该版本的 install.sh 执行（安装步骤只有 install.sh 一份），再刷新升级前装过的 skill。
//! HTTP 用系统的 `/usr/bin/curl`：CLI 不引入 TLS 依赖，冷启动不受影响。

use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::PathBuf;
use std::process::{Command, ExitCode};

const DEFAULT_API: &str = "https://api.github.com/repos/ethanyulabs/alter";
const DEFAULT_BASE: &str = "https://github.com/ethanyulabs/alter/releases/download";
const CURRENT: &str = env!("CARGO_PKG_VERSION");

pub fn run(check: bool, json: bool, version: Option<String>) -> ExitCode {
    let explicit = version.is_some();
    let target = match version {
        Some(v) => match v.strip_prefix('v').filter(|rest| parse(rest).is_some()) {
            Some(rest) => rest.to_owned(),
            None => {
                eprintln!("版本号应为 v<x.y.z>，收到「{v}」");
                return ExitCode::from(1);
            }
        },
        None => match latest() {
            Ok(v) => v,
            Err(e) => {
                eprintln!("无法获取最新版本：{e}");
                return ExitCode::from(1);
            }
        },
    };
    let newer = is_newer(&target, CURRENT);
    if check {
        if json {
            println!(
                "{}",
                serde_json::json!({"current": CURRENT, "latest": target, "updateAvailable": newer})
            );
        } else if newer {
            println!("alter {target} is available (current {CURRENT})");
        } else {
            println!("alter {CURRENT} is up to date");
        }
        return ExitCode::SUCCESS;
    }
    // 没指定版本时只升不降：latest 不比自身新（包括 Release 被撤回、latest 变旧）就是最新
    if target == CURRENT || (!explicit && !newer) {
        println!("alter {CURRENT} is up to date");
        return ExitCode::SUCCESS;
    }
    let Some(app) = bundle_root() else {
        eprintln!("alter upgrade 只能用于 install.sh 安装的 alter，请重新运行 install.sh");
        return ExitCode::from(1);
    };
    let Some(prefix) = app.parent().map(PathBuf::from) else {
        eprintln!("升级失败：找不到 {} 所在的目录", app.display());
        return ExitCode::from(1);
    };
    let bin_dir = invoked_bin_dir();
    // 升级前装过 skill 的目录：install.sh 换掉 app 之后，用新版的 alter 重新装一遍
    let skill_dirs = installed_skill_dirs();
    match run_install_sh(&target, &prefix, &bin_dir) {
        Ok(()) => {}
        Err(e) => {
            eprintln!("升级失败：{e}");
            return ExitCode::from(1);
        }
    }
    for dir in skill_dirs {
        let status = Command::new(bin_dir.join("alter"))
            .args(["skill", "install", "--target"])
            .arg(&dir)
            .status();
        if !status.is_ok_and(|s| s.success()) {
            eprintln!(
                "提示：刷新 {} 里的 skill 没有成功，可以之后运行 alter skill install --target {}",
                dir.display(),
                dir.display()
            );
        }
    }
    println!("upgraded to {target}");
    ExitCode::SUCCESS
}

/// 下载 `v<version>` 的 install.sh，以 `--version`、`--prefix`、`--bin-dir` 执行。
fn run_install_sh(
    version: &str,
    prefix: &std::path::Path,
    bin_dir: &std::path::Path,
) -> Result<(), String> {
    let base = std::env::var("ALTER_RELEASE_BASE").unwrap_or_else(|_| DEFAULT_BASE.to_owned());
    let script = curl(&format!("{base}/v{version}/install.sh"))?;
    // 独占创建一个只有自己能访问的临时目录，脚本用 create_new 写入：不复用、不跟随预置的链接
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir().join(format!("alter-upgrade-{}-{nanos}", std::process::id()));
    let file = dir.join("install.sh");
    let written = std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&dir)
        .and_then(|()| {
            use std::io::Write;
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&file)?
                .write_all(&script)
        });
    if let Err(e) = written {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(format!("写 install.sh 失败：{e}"));
    }
    let status = Command::new("/bin/bash")
        .arg(&file)
        .arg("--version")
        .arg(format!("v{version}"))
        .arg("--prefix")
        .arg(prefix)
        .arg("--bin-dir")
        .arg(bin_dir)
        .status();
    let _ = std::fs::remove_dir_all(&dir);
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("install.sh 退出码 {}", s.code().unwrap_or(-1))),
        Err(e) => Err(format!("启动 bash 失败：{e}")),
    }
}

/// CLI 链接所在的目录：用户经符号链接（如 `~/.local/bin/alter`）调用时取链接的目录，否则为 `~/.local/bin`。
fn invoked_bin_dir() -> PathBuf {
    let fallback = || {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join(".local/bin")
    };
    let Some(argv0) = std::env::args_os().next().map(PathBuf::from) else {
        return fallback();
    };
    let invoked = if argv0.components().count() > 1 {
        std::env::current_dir()
            .map(|d| d.join(&argv0))
            .unwrap_or(argv0)
    } else {
        // 只有名字：在 PATH 里找
        let found = std::env::var_os("PATH").and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|d| d.join(&argv0))
                .find(|p| p.is_file())
        });
        match found {
            Some(p) => p,
            None => return fallback(),
        }
    };
    let real = crate::exe::real_exe();
    let is_link = std::fs::symlink_metadata(&invoked).is_ok_and(|m| m.file_type().is_symlink());
    match invoked.parent() {
        Some(dir) if is_link && real.as_deref() != Some(invoked.as_path()) => dir.to_path_buf(),
        _ => fallback(),
    }
}

/// `~/.claude/skills`、`~/.codex/skills` 中装着 alter 的 skill（带 `.alter-skill`）的那些。
fn installed_skill_dirs() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    [".claude/skills", ".codex/skills"]
        .iter()
        .map(|d| home.join(d))
        .filter(|d| d.join("alter-browser/.alter-skill").is_file())
        .collect()
}

/// 用 curl 取 `<Release API>/releases/latest` 的 `tag_name`，去掉开头的 `v`。
fn latest() -> Result<String, String> {
    let api = std::env::var("ALTER_RELEASE_API").unwrap_or_else(|_| DEFAULT_API.to_owned());
    let body = curl(&format!("{api}/releases/latest"))?;
    let v: serde_json::Value =
        serde_json::from_slice(&body).map_err(|e| format!("Release API 返回的不是 JSON：{e}"))?;
    let tag = v["tag_name"]
        .as_str()
        .ok_or_else(|| "Release API 的结果里没有 tag_name".to_owned())?;
    let version = tag.trim_start_matches('v');
    parse(version).ok_or_else(|| format!("版本号不是三段数字：{tag}"))?;
    Ok(version.to_owned())
}

/// `curl -fsSL` 取一个地址，返回内容。
fn curl(url: &str) -> Result<Vec<u8>, String> {
    let out = Command::new("/usr/bin/curl")
        .args(["-fsSL", "--max-time", "30", "-A"])
        .arg(format!("alter/{CURRENT}"))
        .arg(url)
        .output()
        .map_err(|e| format!("启动 curl 失败：{e}"))?;
    if !out.status.success() {
        let why = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        return Err(format!("{url}：{why}"));
    }
    Ok(out.stdout)
}

/// 三段整数的版本号。
fn parse(v: &str) -> Option<(u64, u64, u64)> {
    let mut it = v.split('.').map(|p| p.parse::<u64>().ok());
    let r = (it.next()??, it.next()??, it.next()??);
    it.next().is_none().then_some(r)
}

fn is_newer(candidate: &str, current: &str) -> bool {
    matches!((parse(candidate), parse(current)), (Some(a), Some(b)) if a > b)
}

/// `alter` 在安装包里时（恰好是 `…/<名字>.app/Contents/Resources/bin/alter`），返回 `…/<名字>.app`。
fn bundle_root() -> Option<PathBuf> {
    let exe = crate::exe::real_exe()?;
    let bin = exe.parent()?;
    let resources = bin.parent()?;
    let contents = resources.parent()?;
    let app = contents.parent()?;
    let ok = exe.file_name()? == "alter"
        && bin.file_name()? == "bin"
        && resources.file_name()? == "Resources"
        && contents.file_name()? == "Contents"
        && app.extension()? == "app";
    ok.then(|| app.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions_numerically() {
        assert!(is_newer("0.10.0", "0.9.9"));
        assert!(is_newer("1.0.0", "0.99.0"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.1.9", "0.2.0"));
        assert!(!is_newer("0.2", "0.1.0"));
        assert!(!is_newer("0.2.0-beta", "0.1.0"));
    }
}
