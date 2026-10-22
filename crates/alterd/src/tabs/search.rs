//! 命令栏的搜索：标签（当前 Space 优先）、Space、当前 Space 的书签、历史，合计至多 8 项，最后附「打开或搜索」。

use alter_core::protocol::{SearchItem, SearchKind};

use super::TabService;

/// 标签、Space、书签、历史合计的上限（不含最后的「打开或搜索」）。
const LIMIT: usize = 8;

impl TabService {
    pub(super) fn search(&self, query: &str) -> anyhow::Result<Vec<SearchItem>> {
        let q = query.trim();
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let needle = q.to_lowercase();
        let hit = |s: &str| s.to_lowercase().contains(&needle);
        let active_space = self.store.active_space_id()?;
        let open_tabs = self.store.list_open_tabs()?;

        // 标签：当前 Space 优先，其余按 Space 顺序与 position（list_open_tabs 已是这个顺序）
        let mut tabs: Vec<_> = open_tabs
            .iter()
            .filter(|t| hit(&t.title) || hit(&t.url))
            .collect();
        tabs.sort_by_key(|t| Some(&t.space_id) != active_space.as_ref());
        let mut items: Vec<SearchItem> = tabs
            .into_iter()
            .map(|t| SearchItem {
                kind: SearchKind::Tab,
                title: if t.title.is_empty() {
                    t.url.clone()
                } else {
                    t.title.clone()
                },
                url: Some(t.url.clone()),
                tab_id: Some(t.id.clone()),
                space_id: Some(t.space_id.clone()),
                input: None,
            })
            .collect();
        for s in self.store.list_spaces()? {
            if hit(&s.name) {
                items.push(SearchItem {
                    kind: SearchKind::Space,
                    title: s.name,
                    url: None,
                    tab_id: None,
                    space_id: Some(s.id),
                    input: None,
                });
            }
        }
        if items.len() < LIMIT
            && let Some(space) = &active_space
        {
            for (url, title) in self.store.search_bookmarks(space, q, LIMIT - items.len())? {
                items.push(SearchItem {
                    kind: SearchKind::Bookmark,
                    title: if title.is_empty() { url.clone() } else { title },
                    url: Some(url),
                    tab_id: None,
                    space_id: None,
                    input: None,
                });
            }
        }
        if items.len() < LIMIT {
            let opened: Vec<String> = open_tabs.iter().map(|t| t.url.clone()).collect();
            for (url, title) in self.store.search_history(q, &opened, LIMIT - items.len())? {
                items.push(SearchItem {
                    kind: SearchKind::History,
                    title: if title.is_empty() { url.clone() } else { title },
                    url: Some(url),
                    tab_id: None,
                    space_id: None,
                    input: None,
                });
            }
        }
        items.truncate(LIMIT);
        items.push(SearchItem {
            kind: SearchKind::Open,
            title: q.to_owned(),
            url: None,
            tab_id: None,
            space_id: None,
            input: Some(q.to_owned()),
        });
        Ok(items)
    }
}
