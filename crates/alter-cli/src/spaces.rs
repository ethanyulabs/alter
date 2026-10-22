//! `alter spaces list`：经 Gateway 列出全部 Space。

use std::path::Path;
use std::process::ExitCode;

use alter_core::protocol::SpaceListResult;
use serde_json::json;

use crate::gateway::{self, CallError};

pub fn list(home: &Path, as_json: bool) -> ExitCode {
    let result = match gateway::call(home, "space.list", json!({})) {
        Ok(v) => v,
        Err(CallError::Unreachable(why)) => {
            eprintln!("{}（{why}）", gateway::NOT_RUNNING);
            return ExitCode::from(2);
        }
        Err(CallError::Rpc { code, message }) => {
            eprintln!("{code}: {message}");
            return ExitCode::from(1);
        }
    };
    if as_json {
        println!("{result}");
        return ExitCode::SUCCESS;
    }
    let Ok(list) = serde_json::from_value::<SpaceListResult>(result) else {
        eprintln!("无法解析 space.list 的结果");
        return ExitCode::from(1);
    };
    for s in list.spaces {
        println!("{}  {}  {} tabs", s.id, s.name, s.tab_count);
    }
    ExitCode::SUCCESS
}
