//! `alter skill install|uninstall`：把随 app 打包的 agent skill（`alter-browser`）装进 agent 的 skills 目录，或移出。
//! 不给 `--target` 时目标为 `~/.claude/skills` 与 `~/.codex/skills` 中已经存在的那些。
//! 装好的目录里写 `.alter-skill`（内容为 CLI 版本），卸载只删带这个标记的目录。

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const NAME: &str = "alter-browser";
const MARKER: &str = ".alter-skill";

/// 开发时 skill 在仓库里的位置（编译期确定）。
const DEV_SKILL: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../skills/alter-browser");

pub fn install(targets: Vec<PathBuf>) -> ExitCode {
    let Some(src) = find_source() else {
        eprintln!("找不到 {NAME} skill：设置 ALTER_SKILL_DIR，或重新运行 install.sh 安装 alter");
        return ExitCode::from(1);
    };
    let targets = resolve_targets(targets);
    if targets.is_empty() {
        eprintln!("没有找到 ~/.claude/skills 或 ~/.codex/skills，用 --target <dir> 指定");
        return ExitCode::from(1);
    }
    for dir in targets {
        let dest = dir.join(NAME);
        if same_or_nested(&src, &dest) {
            eprintln!(
                "skill 的源与目标相同（或互相包含）：{} 与 {}",
                src.display(),
                dest.display()
            );
            return ExitCode::from(1);
        }
        if let Err(e) = replace_with_copy(&src, &dir, &dest) {
            eprintln!("安装到 {} 失败：{e}", dest.display());
            return ExitCode::from(1);
        }
        println!("installed {NAME} -> {}", dest.display());
    }
    ExitCode::SUCCESS
}

pub fn uninstall(targets: Vec<PathBuf>) -> ExitCode {
    for dir in resolve_targets(targets) {
        let dest = dir.join(NAME);
        if dest.join(MARKER).is_file() {
            if let Err(e) = std::fs::remove_dir_all(&dest) {
                eprintln!("删除 {} 失败：{e}", dest.display());
                return ExitCode::from(1);
            }
            println!("removed {}", dest.display());
        } else if dest.exists() {
            // 用户自己放的同名 skill：没有 alter 的标记，不碰
            eprintln!("{} 不是 alter 安装的，未删除", dest.display());
        }
    }
    ExitCode::SUCCESS
}

/// `ALTER_SKILL_DIR` → `<alter 所在目录>/../skills/alter-browser`（打包后）→ 仓库内的 skills/alter-browser（开发时）。
fn find_source() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("ALTER_SKILL_DIR") {
        let p = PathBuf::from(p);
        return p.is_dir().then_some(p);
    }
    let packaged =
        crate::exe::real_exe().and_then(|e| e.parent().map(|d| d.join("../skills").join(NAME)));
    if let Some(p) = packaged
        && p.is_dir()
    {
        return Some(p);
    }
    let dev = PathBuf::from(DEV_SKILL);
    dev.is_dir().then_some(dev)
}

/// 显式给出的目标原样使用（去重）；没给时取 HOME 下已存在的 agent skills 目录。
fn resolve_targets(given: Vec<PathBuf>) -> Vec<PathBuf> {
    if !given.is_empty() {
        let mut out: Vec<PathBuf> = Vec::new();
        for dir in given {
            if !out.contains(&dir) {
                out.push(dir);
            }
        }
        return out;
    }
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    [".claude/skills", ".codex/skills"]
        .iter()
        .map(|d| home.join(d))
        .filter(|d| d.is_dir())
        .collect()
}

/// 覆盖成当前版本：先复制到同级的暂存目录并写好标记，再把旧目录换下来；任何一步失败时旧版本原样保留。
fn replace_with_copy(src: &Path, dir: &Path, dest: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let pid = std::process::id();
    let staging = dir.join(format!(".{NAME}.tmp-{pid}"));
    let old = dir.join(format!(".{NAME}.old-{pid}"));
    remove_if_exists(&staging)?;
    let staged = copy_dir(src, &staging)
        .and_then(|()| std::fs::write(staging.join(MARKER), env!("CARGO_PKG_VERSION")));
    if let Err(e) = staged {
        let _ = remove_if_exists(&staging);
        return Err(e);
    }
    let had_old = dest.exists();
    if had_old {
        remove_if_exists(&old)?;
        if let Err(e) = std::fs::rename(dest, &old) {
            let _ = remove_if_exists(&staging);
            return Err(e);
        }
    }
    if let Err(e) = std::fs::rename(&staging, dest) {
        if had_old {
            let _ = std::fs::rename(&old, dest);
        }
        let _ = remove_if_exists(&staging);
        return Err(e);
    }
    if had_old {
        remove_if_exists(&old)?;
    }
    Ok(())
}

/// 两个路径是同一个目录，或一个在另一个里面（规范化后比较；目标还不存在时按它的父目录）。
fn same_or_nested(src: &Path, dest: &Path) -> bool {
    let canon = |p: &Path| {
        std::fs::canonicalize(p).unwrap_or_else(|_| {
            p.parent()
                .and_then(|d| std::fs::canonicalize(d).ok())
                .map(|d| d.join(p.file_name().unwrap_or_default()))
                .unwrap_or_else(|| p.to_path_buf())
        })
    };
    let (a, b) = (canon(src), canon(dest));
    a.starts_with(&b) || b.starts_with(&a)
}

fn remove_if_exists(dir: &Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(dir) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// 递归复制目录；符号链接按链接复制，不跟随。
fn copy_dir(src: &Path, dest: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let to = dest.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            std::os::unix::fs::symlink(std::fs::read_link(entry.path())?, &to)?;
        } else if kind.is_dir() {
            copy_dir(&entry.path(), &to)?;
        } else {
            std::fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}
