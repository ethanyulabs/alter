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

/// N5 R12.S1（承接 N4 R12.S1、N3 R9.S1、N2 R23.S1、N1 R16.S1）导出错误码与健康文件类型。
#[test]
fn n5_r12_s1_export_error_code_and_health_file() {
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

/// RC N1 R9.S2（承接 add-core-features N5 R12.S2）导出 Shell Control 消息类型：8 种命令、20 种事件，
/// TabView 带 pinned、taskSpaceId、label、splitGroup，ImportedCookie 字段恰好为 Electron `cookies.set` 的那 9 个。
#[test]
fn rc_n1_r9_s2_export_shell_messages() {
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
        expected(&[
            "shell.stateChanged",
            "shell.navigate",
            "shell.error",
            "shell.discard",
            "shell.searchResults",
            "shell.importSources",
            "shell.importProgress",
            "shell.importCookies",
        ]),
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
            "shell.event.spaceCreated",
            "shell.event.spaceUpdated",
            "shell.event.spaceActivated",
            "shell.event.spaceDeleted",
            "shell.event.tabPinned",
            "shell.event.tabMoved",
            "shell.event.cdpReady",
            "shell.event.ownershipReturned",
            "shell.event.userTookControl",
            "shell.event.searchRequested",
            "shell.event.splitRequested",
            "shell.event.splitDissolved",
            "shell.event.tabRestored",
            "shell.event.importSourcesRequested",
            "shell.event.importRequested",
            "shell.event.cookiesImported",
        ]),
        "ShellEvent.ts 内容：{event}"
    );
    let tab = read(out.path(), "TabView.ts");
    for field in ["pinned: boolean", "taskSpaceId", "label", "splitGroup"] {
        assert!(tab.contains(field), "TabView.ts 缺 {field}：{tab}");
    }
    let cookie = read(out.path(), "ImportedCookie.ts");
    assert_eq!(
        object_fields(&cookie),
        expected(&[
            "url",
            "name",
            "value",
            "domain",
            "path",
            "secure",
            "httpOnly",
            "sameSite",
            "expirationDate",
        ]),
        "ImportedCookie.ts 内容：{cookie}"
    );
    let same_site = read(out.path(), "SameSite.ts");
    let literals: BTreeSet<String> = same_site
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect();
    assert_eq!(
        literals,
        expected(&["unspecified", "no_restriction", "lax", "strict"]),
        "SameSite.ts 内容：{same_site}"
    );
}

/// 对象类型声明里的字段名集合。
fn object_fields(src: &str) -> BTreeSet<String> {
    // 跳过开头的 `import type { … }`，从类型定义的 `= {` 开始
    let body = src
        .split_once("= {")
        .and_then(|(_, b)| b.rsplit_once('}'))
        .map(|(b, _)| b)
        .unwrap_or_else(|| panic!("不是对象类型：{src}"));
    body.split([',', ';'])
        .filter_map(|f| f.split_once(':'))
        .map(|(k, _)| k.trim().trim_end_matches('?').to_owned())
        .filter(|k| !k.is_empty())
        .collect()
}

/// N5 R12.S3（承接 N4 R12.S3、N3 R9.S3、N2 R23.S3）导出 Gateway 类型：GatewayFile 字段恰好为 port、token、cdpPort、version，
/// Receipt 含 ok、durationMs、popups，OwnershipChanged 字段恰好为 id、ownership、reason。
#[test]
fn n5_r12_s3_export_gateway_types() {
    let out = tempfile::tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_export-ts"))
        .arg(out.path())
        .status()
        .expect("export-ts 应当能启动");
    assert_eq!(status.code(), Some(0));

    let file = read(out.path(), "GatewayFile.ts");
    let want: BTreeSet<String> = ["port", "token", "cdpPort", "version"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    assert_eq!(object_fields(&file), want, "GatewayFile.ts 内容：{file}");

    let receipt = object_fields(&read(out.path(), "Receipt.ts"));
    for field in ["ok", "durationMs", "popups"] {
        assert!(receipt.contains(field), "Receipt 缺 {field}：{receipt:?}");
    }

    let changed = read(out.path(), "OwnershipChanged.ts");
    let want: BTreeSet<String> = ["id", "ownership", "reason"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    assert_eq!(
        object_fields(&changed),
        want,
        "OwnershipChanged.ts 内容：{changed}"
    );

    let counts = read(out.path(), "ImportCounts.ts");
    let want: BTreeSet<String> = ["bookmarks", "history", "cookies", "skipped"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    assert_eq!(
        object_fields(&counts),
        want,
        "ImportCounts.ts 内容：{counts}"
    );
}

/// N1 预设颜色另写成 `spaceColors.ts` 常量，供 Renderer 色板与 alterd 校验共用一份。
#[test]
fn n1_export_space_colors() {
    let out = tempfile::tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_export-ts"))
        .arg(out.path())
        .status()
        .expect("export-ts 应当能启动");
    assert_eq!(status.code(), Some(0));

    let src = read(out.path(), "spaceColors.ts");
    assert!(
        src.contains("export const SPACE_COLORS = [") && src.contains("] as const;"),
        "spaceColors.ts 内容：{src}"
    );
    let colors: Vec<&str> = src.split('"').skip(1).step_by(2).collect();
    assert_eq!(
        colors,
        [
            "#6366f1", "#ef4444", "#f97316", "#eab308", "#22c55e", "#06b6d4", "#3b82f6", "#ec4899"
        ]
    );
}

/// RC N2 R15.S1 导出更新提示类型：UpdateAvailable 字段恰好为 version、url，ShellState 含 updateAvailable。
#[test]
fn rc_n2_r15_s1_export_update_available() {
    let out = tempfile::tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_export-ts"))
        .arg(out.path())
        .status()
        .expect("export-ts 应当能启动");
    assert_eq!(status.code(), Some(0));
    let expected = |xs: &[&str]| xs.iter().map(|s| (*s).to_owned()).collect::<BTreeSet<_>>();
    let update = read(out.path(), "UpdateAvailable.ts");
    assert_eq!(
        object_fields(&update),
        expected(&["version", "url"]),
        "UpdateAvailable.ts 内容：{update}"
    );
    let state = read(out.path(), "ShellState.ts");
    assert!(
        object_fields(&state).contains("updateAvailable"),
        "ShellState.ts 内容：{state}"
    );
}
