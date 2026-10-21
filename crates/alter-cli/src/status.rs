//! `alter status`：根据健康文件报告 alterd 状态。

use std::path::Path;
use std::process::ExitCode;

use alter_core::protocol::HealthFile;
use serde::Serialize;

/// alterd 不在运行时的退出码。
const EXIT_NOT_RUNNING: u8 = 1;

/// 从健康文件推断出的守护进程状态。
enum State {
    /// 没有健康文件
    Missing,
    /// 健康文件存在但记录的 pid 已不存活
    Stale,
    Running(HealthFile),
}

/// `--json` 的输出结构；字段顺序即输出顺序，缺省字段不输出。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusJson<'a> {
    running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    db_path: Option<&'a str>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stale: bool,
}

pub fn run(home: &Path, json: bool) -> ExitCode {
    let state = match alter_core::health::read(home) {
        None => State::Missing,
        Some(h) if !alter_core::health::is_alive(h.pid) => State::Stale,
        Some(h) => State::Running(h),
    };
    if json {
        print_json(&state);
    } else {
        print_text(&state);
    }
    match state {
        State::Running(_) => ExitCode::SUCCESS,
        State::Missing | State::Stale => ExitCode::from(EXIT_NOT_RUNNING),
    }
}

fn print_text(state: &State) {
    match state {
        State::Missing => println!("not running"),
        State::Stale => println!("not running (stale)"),
        State::Running(h) => println!("alterd running (pid {}, version {})", h.pid, h.version),
    }
}

fn print_json(state: &State) {
    let out = match state {
        State::Missing | State::Stale => StatusJson {
            running: false,
            pid: None,
            version: None,
            db_path: None,
            stale: matches!(state, State::Stale),
        },
        State::Running(h) => StatusJson {
            running: true,
            pid: Some(h.pid),
            version: Some(&h.version),
            db_path: Some(&h.db_path),
            stale: false,
        },
    };
    println!("{}", serde_json::to_string(&out).expect("状态可以序列化"));
}
