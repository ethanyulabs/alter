//! 读 `Bookmarks`（JSON）：书签栏与其他书签下的文件夹和链接，按先序展开。

use alter_core::store::ImportedBookmark;
use serde_json::Value;

/// 解析 `Bookmarks` 的内容：返回按先序排列的条目与无法解析而跳过的条目数。
/// 书签栏与其他书签这两个根本身不导入，它们的子项成为根级条目（书签栏的在前）。
pub(super) fn parse(text: &str) -> Result<(Vec<ImportedBookmark>, u32), String> {
    let doc: Value =
        serde_json::from_str(text).map_err(|e| format!("Bookmarks 不是合法的 JSON：{e}"))?;
    let mut out = Vec::new();
    let mut skipped = 0;
    for root in ["bookmark_bar", "other"] {
        if let Some(children) = doc["roots"][root]["children"].as_array() {
            walk(children, None, &mut out, &mut skipped);
        }
    }
    Ok((out, skipped))
}

fn walk(
    nodes: &[Value],
    parent: Option<usize>,
    out: &mut Vec<ImportedBookmark>,
    skipped: &mut u32,
) {
    for node in nodes {
        let title = node["name"].as_str().unwrap_or_default().to_owned();
        match node["type"].as_str() {
            Some("url") => match node["url"].as_str() {
                Some(url) if !url.is_empty() => out.push(ImportedBookmark {
                    title,
                    url: Some(url.to_owned()),
                    parent,
                }),
                _ => *skipped += 1,
            },
            Some("folder") => {
                let index = out.len();
                out.push(ImportedBookmark {
                    title,
                    url: None,
                    parent,
                });
                if let Some(children) = node["children"].as_array() {
                    walk(children, Some(index), out, skipped);
                }
            }
            _ => *skipped += 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 文件夹先于其子项，子项指向它；缺 url 的链接与未知类型计入跳过。
    #[test]
    fn parses_tree_in_preorder_and_skips_broken_nodes() {
        let text = r#"{"roots":{
            "bookmark_bar":{"type":"folder","children":[
                {"type":"url","name":"Rust","url":"https://rust-lang.org/"},
                {"type":"folder","name":"Dev","children":[
                    {"type":"url","name":"MDN","url":"https://developer.mozilla.org/"},
                    {"type":"url","name":"坏的"}
                ]}
            ]},
            "other":{"type":"folder","children":[{"type":"separator"}]}
        }}"#;
        let (items, skipped) = parse(text).unwrap();
        let view: Vec<_> = items
            .iter()
            .map(|b| (b.title.as_str(), b.url.is_some(), b.parent))
            .collect();
        assert_eq!(
            view,
            vec![
                ("Rust", true, None),
                ("Dev", false, None),
                ("MDN", true, Some(1))
            ]
        );
        assert_eq!(skipped, 2);
    }
}
