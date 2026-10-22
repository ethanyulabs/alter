// fixture profile：在临时目录里生成 Chromium 格式的 profile（Bookmarks JSON、History SQLite），
// 与 Rust 侧 crates/alterd/tests/common/fixture_profile.rs 的「标准 fixture profile」一致
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";

/** 1601-01-01 到 1970-01-01 的毫秒数。 */
const CHROME_EPOCH_OFFSET_MS = 11_644_473_600_000;

export const DAY_MS = 24 * 60 * 60 * 1000;

/** UTC 毫秒 → Chrome 时间（1601 年起的微秒；超出 2^53，用 BigInt）。 */
export function chromeTime(ms: number): bigint {
  return (BigInt(ms) + BigInt(CHROME_EPOCH_OFFSET_MS)) * 1000n;
}

export interface BookmarkNode {
  type: "url" | "folder";
  name: string;
  url?: string;
  children?: BookmarkNode[];
}

export function link(name: string, url: string): BookmarkNode {
  return { type: "url", name, url };
}

export function folder(name: string, children: BookmarkNode[]): BookmarkNode {
  return { type: "folder", name, children };
}

/** 写 `Bookmarks`：书签栏与其他书签下的节点。 */
export function writeBookmarks(dir: string, bar: BookmarkNode[], other: BookmarkNode[] = []): void {
  mkdirSync(dir, { recursive: true });
  const doc = {
    checksum: "",
    roots: {
      bookmark_bar: { type: "folder", name: "书签栏", children: bar },
      other: { type: "folder", name: "其他书签", children: other },
      synced: { type: "folder", name: "移动设备书签", children: [] },
    },
    version: 1,
  };
  writeFileSync(join(dir, "Bookmarks"), JSON.stringify(doc));
}

/** 写 `History`：每个 URL 一行 `urls`，每次访问（UTC 毫秒）一行 `visits`。 */
export function writeHistory(dir: string, entries: { url: string; title: string; visits: number[] }[]): void {
  mkdirSync(dir, { recursive: true });
  const db = new DatabaseSync(join(dir, "History"));
  try {
    db.exec(`CREATE TABLE urls (id INTEGER PRIMARY KEY AUTOINCREMENT, url LONGVARCHAR, title LONGVARCHAR,
               visit_count INTEGER DEFAULT 0 NOT NULL, typed_count INTEGER DEFAULT 0 NOT NULL,
               last_visit_time INTEGER NOT NULL, hidden INTEGER DEFAULT 0 NOT NULL);
             CREATE TABLE visits (id INTEGER PRIMARY KEY AUTOINCREMENT, url INTEGER NOT NULL,
               visit_time INTEGER NOT NULL, from_visit INTEGER, transition INTEGER DEFAULT 0 NOT NULL);`);
    const insertUrl = db.prepare(
      "INSERT INTO urls (url, title, visit_count, last_visit_time) VALUES (?, ?, ?, ?)",
    );
    const insertVisit = db.prepare("INSERT INTO visits (url, visit_time) VALUES (?, ?)");
    for (const e of entries) {
      const last = Math.max(0, ...e.visits);
      const { lastInsertRowid } = insertUrl.run(e.url, e.title, e.visits.length, chromeTime(last));
      for (const v of e.visits) insertVisit.run(lastInsertRowid, chromeTime(v));
    }
  } finally {
    db.close();
  }
}

/**
 * 标准 fixture profile 的书签与历史：书签栏下 `Rust` 与 `Dev`（其中 `MDN`）；
 * `https://a.example/` 10 天前访问 2 次，`https://b.example/` 100 天前访问 1 次。
 */
export function standardProfile(dir: string): void {
  writeBookmarks(dir, [
    link("Rust", "https://rust-lang.org/"),
    folder("Dev", [link("MDN", "https://developer.mozilla.org/")]),
  ]);
  const tenDaysAgo = Date.now() - 10 * DAY_MS;
  writeHistory(dir, [
    { url: "https://a.example/", title: "A", visits: [tenDaysAgo, tenDaysAgo + 60_000] },
    { url: "https://b.example/", title: "B", visits: [Date.now() - 100 * DAY_MS] },
  ]);
}
