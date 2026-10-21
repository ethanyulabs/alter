//! 地址输入规范化：把用户在地址框里输入的内容变成要打开的 URL。

/// 非 URL 输入的搜索地址前缀（可配置搜索引擎不在当前需求范围）。
const SEARCH_PREFIX: &str = "https://www.google.com/search?q=";

/// 去掉首尾空白后转换；空输入返回 `None`。
pub fn normalize_input(input: &str) -> Option<String> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    // blob: 来自网页弹窗（已解析的 URL）；data: 不放行，避免顶层打开 data: 页面钓鱼
    if has_scheme(s) || s.starts_with("about:") || s.starts_with("blob:") {
        return Some(s.to_owned());
    }
    if !s.chars().any(char::is_whitespace) {
        let host = host_of(s);
        // 本机地址先判断：IPv4 也满足「含点号」的域名规则
        if host == "localhost" || is_ipv4(host) {
            return Some(format!("http://{s}"));
        }
        if is_domain(host) {
            return Some(format!("https://{s}"));
        }
    }
    Some(format!("{SEARCH_PREFIX}{}", percent_encode(s)))
}

/// RFC 3986 的非保留字符原样保留，其余按 UTF-8 字节编码为 `%XX`（空格为 `%20`）。
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// 以 `<scheme>://` 开头（scheme 按 RFC 3986：字母开头，后接字母、数字、`+`、`-`、`.`）。
fn has_scheme(s: &str) -> bool {
    let Some((scheme, _)) = s.split_once("://") else {
        return false;
    };
    let mut chars = scheme.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// 第一个 `/` 之前、去掉 `:端口` 后的部分（R5：只有「主机名[:端口][/路径]」形式才算地址）。
fn host_of(s: &str) -> &str {
    let authority = s.split('/').next().unwrap_or("");
    match authority.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) => host,
        _ => authority,
    }
}

/// 四段 0–255 的十进制数。
fn is_ipv4(host: &str) -> bool {
    let parts: Vec<&str> = host.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.len() <= 3
                && p.chars().all(|c| c.is_ascii_digit())
                && p.parse::<u16>().is_ok_and(|n| n <= 255)
        })
}

/// 含点号、只由字母、数字、`-`、`.` 组成，且不以点号开头或结尾。
fn is_domain(host: &str) -> bool {
    host.contains('.')
        && !host.starts_with('.')
        && !host.ends_with('.')
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.'))
}
