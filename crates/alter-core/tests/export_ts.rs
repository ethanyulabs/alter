//! protocol 能力的场景测试：从 `export-ts` 二进制驱动。

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

/// api.md 定义的全部错误码。
const ERROR_CODES: [&str; 16] = [
    "E_UNAUTHORIZED",
    "E_PROTOCOL_VERSION",
    "E_SPACE_NOT_FOUND",
    "E_TASK_SPACE_NOT_FOUND",
    "E_PAGE_NOT_FOUND",
    "E_PAGE_BUDGET",
    "E_USER_CONTROL",
    "E_AMBIGUOUS_SELECTOR",
    "E_NO_MATCH",
    "E_TIMEOUT",
    "E_DIALOG_OPEN",
    "E_IMPORT_SOURCE_NOT_FOUND",
    "E_KEYCHAIN_DENIED",
    "E_INTERNAL",
    "E_TAB_NOT_FOUND",
    "E_INVALID_ARGUMENT",
];

/// 去掉 `//` 行注释与 `/* */` 块注释，只留类型声明本身。
fn strip_comments(src: &str) -> String {
    let mut out = String::new();
    let mut rest = src;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        rest = match rest[start..].find("*/") {
            Some(end) => &rest[start + end + 2..],
            None => "",
        };
    }
    out.push_str(rest);
    out.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn read(dir: &Path, file: &str) -> String {
    let path = dir.join(file);
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("读取 {} 失败：{e}", path.display()));
    strip_comments(&src)
}

/// R13.S1 导出错误码与健康文件类型。
#[test]
fn r13_s1_export_error_code_and_health_file() {
    let out = tempfile::tempdir().unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_export-ts"))
        .arg(out.path())
        .status()
        .expect("export-ts 应当能启动");
    assert_eq!(status.code(), Some(0));

    // ErrorCode：字符串字面量联合，恰好 16 个错误码
    let error_code = read(out.path(), "ErrorCode.ts");
    assert!(
        error_code.contains("export type ErrorCode ="),
        "ErrorCode.ts 内容：{error_code}"
    );
    let literals: BTreeSet<&str> = error_code.split('"').skip(1).step_by(2).collect();
    let expected: BTreeSet<&str> = ERROR_CODES.into_iter().collect();
    assert_eq!(literals, expected);

    // HealthFile：字段恰好为 version / pid / startedAt / dbPath，类型与 JSON 一致
    let health = read(out.path(), "HealthFile.ts");
    let body = health
        .split_once('{')
        .and_then(|(_, b)| b.rsplit_once('}'))
        .map(|(b, _)| b)
        .unwrap_or_else(|| panic!("HealthFile.ts 不是对象类型：{health}"));
    let fields: BTreeSet<(String, String)> = body
        .split([',', ';'])
        .filter_map(|f| f.split_once(':'))
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .collect();
    let expected_fields: BTreeSet<(String, String)> = [
        ("version", "string"),
        ("pid", "number"),
        ("startedAt", "number"),
        ("dbPath", "string"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect();
    assert_eq!(fields, expected_fields);
}

/// 声明里 `type` 字段（可带引号）取到的全部字符串字面量。
fn type_literals(src: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = src;
    while let Some(i) = rest.find("type") {
        let after = rest[i + 4..].trim_start_matches('"').trim_start();
        // 只认作为字段名出现的 `type`：后面紧跟冒号与字符串字面量
        if let Some(value) = after.strip_prefix(':').map(str::trim_start)
            && let Some(lit) = value.strip_prefix('"')
            && let Some(end) = lit.find('"')
        {
            out.insert(lit[..end].to_owned());
        }
        rest = &rest[i + 4..];
    }
    out
}

/// R13.S2 导出 Shell Control 消息类型。
#[test]
fn r13_s2_export_shell_messages() {
    let out = tempfile::tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_export-ts"))
        .arg(out.path())
        .status()
        .expect("export-ts 应当能启动");
    assert_eq!(status.code(), Some(0));

    let expected = |xs: &[&str]| xs.iter().map(|s| (*s).to_owned()).collect::<BTreeSet<_>>();
    let command = read(out.path(), "ShellCommand.ts");
    assert_eq!(
        type_literals(&command),
        expected(&["shell.stateChanged", "shell.navigate", "shell.error"]),
        "ShellCommand.ts 内容：{command}"
    );
    let event = read(out.path(), "ShellEvent.ts");
    assert_eq!(
        type_literals(&event),
        expected(&[
            "shell.event.openRequested",
            "shell.event.tabUpdated",
            "shell.event.tabActivated",
            "shell.event.tabClosed",
        ]),
        "ShellEvent.ts 内容：{event}"
    );
}
