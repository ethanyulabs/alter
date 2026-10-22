//! ImportService：把 Chrome / Arc profile 的书签与历史导入某个 Space（Cookie 的导入不在本需求，`items` 含它时拒绝）。
//! 源文件先拷贝到 `ALTER_HOME/import-tmp/<importId>/` 再读（源浏览器开着时库可能被锁）；
//! 读文件与解析放进 `spawn_blocking`，写库经 TabService 的循环，每批 500 条一个事务。
//! Gateway 与 Shell 两个入口共用 `Imports::start`，进度经调用方给的通道送回。

mod bookmarks;
mod history;
pub(crate) mod sources;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use alter_core::protocol::{
    ErrorCode, ImportCounts, ImportDone, ImportItem, ImportProgress, ImportSource, ImportState,
    ImportStatus, ShellCommand,
};
use alter_core::store::Store;
use tokio::sync::{mpsc, oneshot};

use crate::lifecycle::now_millis;
use crate::tabs::{self, OpError};

/// 每个写库事务的条数。
const BATCH: usize = 500;

/// 一次导入的请求。
pub struct StartRequest {
    pub source: ImportSource,
    /// None 时用来源浏览器数据根目录下的 `Default`
    pub profile_path: Option<String>,
    pub space_id: String,
    /// None 时导入书签与历史
    pub items: Option<Vec<ImportItem>>,
}

/// 送回发起方的进度。
pub enum Event {
    /// 完成了一类数据
    Progress(ImportProgress),
    /// 全部结束
    Done(ImportDone),
}

pub struct Imports {
    tabs: mpsc::UnboundedSender<tabs::Request>,
    home: PathBuf,
    /// 本次运行里各导入的状态。`skipped` 只记在这里：imports 表没有这一列，重启后按 0 报
    states: Mutex<HashMap<i64, ImportState>>,
}

impl Imports {
    pub fn new(tabs: mpsc::UnboundedSender<tabs::Request>, home: PathBuf) -> Arc<Imports> {
        Arc::new(Imports {
            tabs,
            home,
            states: Mutex::new(HashMap::new()),
        })
    }

    /// 检查来源与目标 Space，记一行 `running` 的导入，在后台开始导入并返回它的 id。
    pub async fn start(
        self: &Arc<Self>,
        req: StartRequest,
        events: mpsc::UnboundedSender<Event>,
    ) -> Result<i64, OpError> {
        let dir = sources::profile_dir(req.source, req.profile_path.as_deref())?;
        let items = in_order(req.items.as_deref());
        if items.is_empty() {
            return Err(OpError::new(ErrorCode::InvalidArgument, "items 不能为空"));
        }
        if items.contains(&ImportItem::Cookies) {
            return Err(OpError::new(
                ErrorCode::InvalidArgument,
                "还不支持导入 Cookie",
            ));
        }
        let source = source_name(req.source);
        let space = req.space_id.clone();
        let path = dir.to_string_lossy().into_owned();
        let id = self
            .db(move |s| {
                if !s.space_exists(&space).map_err(tabs::db_error)? {
                    return Err(OpError::new(
                        ErrorCode::SpaceNotFound,
                        format!("没有这个 Space：{space}"),
                    ));
                }
                s.create_import(&space, source, &path, now_millis())
                    .map_err(tabs::db_error)
            })
            .await??;
        self.set_state(id, ImportStatus::Running, ImportCounts::default(), None);
        tracing::info!(import_id = id, source, "开始导入");
        tokio::spawn(
            self.clone()
                .run(id, dir, req.space_id, source, items, events),
        );
        Ok(id)
    }

    /// 导入的状态与计数：本次运行里的从内存读，更早的从 imports 表读。
    pub async fn status(&self, id: i64) -> Result<ImportState, OpError> {
        if let Some(state) = self.lock().get(&id) {
            return Ok(state.clone());
        }
        let row = self
            .db(move |s| s.import_row(id))
            .await?
            .map_err(tabs::db_error)?
            .ok_or_else(|| {
                OpError::new(ErrorCode::InvalidArgument, format!("没有这个导入：{id}"))
            })?;
        let status = match row.status.as_str() {
            "running" => ImportStatus::Running,
            "done" => ImportStatus::Done,
            "partial" => ImportStatus::Partial,
            _ => ImportStatus::Failed,
        };
        Ok(ImportState {
            status,
            counts: ImportCounts {
                bookmarks: row.bookmarks as u32,
                history: row.history as u32,
                cookies: row.cookies as u32,
                skipped: 0,
            },
            warning: (status == ImportStatus::Partial)
                .then(|| row.error.clone())
                .flatten(),
            error: row.error,
        })
    }

    async fn run(
        self: Arc<Self>,
        id: i64,
        dir: PathBuf,
        space: String,
        source: &'static str,
        items: Vec<ImportItem>,
        events: mpsc::UnboundedSender<Event>,
    ) {
        let tmp = self.home.join("import-tmp").join(id.to_string());
        let mut counts = ImportCounts::default();
        let mut error = None;
        for item in items {
            let result = match item {
                ImportItem::Bookmarks => self.bookmarks(&dir, &tmp, &space, source).await,
                ImportItem::History => self.history(&dir, &tmp, &space, source).await,
                ImportItem::Cookies => Err("还不支持导入 Cookie".to_owned()),
            };
            match result {
                Ok((n, skipped)) => {
                    match item {
                        ImportItem::Bookmarks => counts.bookmarks = n,
                        ImportItem::History => counts.history = n,
                        ImportItem::Cookies => counts.cookies = n,
                    }
                    counts.skipped += skipped;
                    tracing::info!(
                        import_id = id,
                        item = item_name(item),
                        count = n,
                        skipped,
                        "导入完成一类"
                    );
                }
                Err(e) => {
                    tracing::warn!(import_id = id, error = %e, "导入失败");
                    error = Some(e);
                    break;
                }
            }
            self.set_state(id, ImportStatus::Running, counts, None);
            let _ = events.send(Event::Progress(ImportProgress {
                import_id: id,
                item,
                counts,
            }));
        }
        let removed = {
            let tmp = tmp.clone();
            tokio::task::spawn_blocking(move || std::fs::remove_dir_all(tmp)).await
        };
        match removed {
            // 一类都没开始读时目录不存在
            Ok(Ok(())) => {}
            Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => {}
            Ok(Err(e)) => {
                tracing::warn!(path = %tmp.display(), error = %e, "删除导入的临时副本失败")
            }
            Err(e) => tracing::warn!(path = %tmp.display(), error = %e, "删除导入的临时副本失败"),
        }

        let mut status = if error.is_some() {
            ImportStatus::Failed
        } else {
            ImportStatus::Done
        };
        let written = {
            let error = error.clone();
            let name = status_name(status);
            let c = [
                counts.bookmarks as i64,
                counts.history as i64,
                counts.cookies as i64,
            ];
            self.db(move |s| s.finish_import(id, name, c, error.as_deref(), now_millis()))
                .await
        };
        // 结果没写进去就不能报成功：写库出错，或导入行已不在（目标 Space 在最后一步之后被删除，级联删掉了它）
        let lost = match written {
            Ok(Ok(true)) => None,
            Ok(Ok(false)) => Some("导入记录已不存在（目标 Space 可能已被删除）".to_owned()),
            Ok(Err(e)) => Some(format!("写导入结果失败：{e}")),
            Err(e) => Some(format!("写导入结果失败：{}", e.message)),
        };
        if let Some(reason) = lost {
            tracing::error!(import_id = id, error = %reason, "导入结果没有写进数据库");
            status = ImportStatus::Failed;
            error = Some(reason);
        }
        self.set_state(id, status, counts, error);
        tracing::info!(import_id = id, status = status_name(status), "导入结束");
        let _ = events.send(Event::Done(ImportDone {
            import_id: id,
            status,
            counts,
            warning: None,
        }));
    }

    /// 导入书签：返回 (条目数, 跳过数)；profile 里没有 `Bookmarks` 时为 0。
    async fn bookmarks(
        &self,
        dir: &Path,
        tmp: &Path,
        space: &str,
        source: &'static str,
    ) -> Result<(u32, u32), String> {
        let (src, tmp) = (dir.join("Bookmarks"), tmp.to_path_buf());
        let parsed = tokio::task::spawn_blocking(move || {
            if !src.is_file() {
                return Ok(None);
            }
            let copy = copy_in(&src, &tmp)?;
            let text =
                std::fs::read_to_string(&copy).map_err(|e| format!("读取 Bookmarks 失败：{e}"))?;
            bookmarks::parse(&text).map(Some)
        })
        .await
        .map_err(|e| e.to_string())??;
        let Some((items, skipped)) = parsed else {
            return Ok((0, 0));
        };
        let items = Arc::new(items);
        let mut ids = Vec::with_capacity(items.len());
        for offset in (0..items.len()).step_by(BATCH) {
            let end = (offset + BATCH).min(items.len());
            let (items, space) = (items.clone(), space.to_owned());
            let mut batch_ids = std::mem::take(&mut ids);
            ids = self
                .db(move |s| {
                    s.import_bookmarks(
                        &space,
                        source,
                        &items[offset..end],
                        offset,
                        &mut batch_ids,
                        &mut || ulid::Ulid::generate().to_string(),
                    )
                    .map(|()| batch_ids)
                })
                .await
                .map_err(|e| e.message)?
                .map_err(|e| format!("写入书签失败：{e}"))?;
        }
        Ok((items.len() as u32, skipped))
    }

    /// 导入最近 90 天的历史：返回 (访问数, 跳过数)；profile 里没有 `History` 时为 0。
    async fn history(
        &self,
        dir: &Path,
        tmp: &Path,
        space: &str,
        source: &'static str,
    ) -> Result<(u32, u32), String> {
        let (src, tmp) = (dir.join("History"), tmp.to_path_buf());
        let since = now_millis() - history::WINDOW_MS;
        let read = tokio::task::spawn_blocking(move || {
            if !src.is_file() {
                return Ok(None);
            }
            let copy = snapshot(&src, &tmp)?;
            history::read(&copy, since).map(Some)
        })
        .await
        .map_err(|e| e.to_string())??;
        let Some((visits, skipped)) = read else {
            return Ok((0, 0));
        };
        let visits = Arc::new(visits);
        for offset in (0..visits.len()).step_by(BATCH) {
            let end = (offset + BATCH).min(visits.len());
            let (visits, space) = (visits.clone(), space.to_owned());
            self.db(move |s| s.import_history(&space, source, &visits[offset..end]))
                .await
                .map_err(|e| e.message)?
                .map_err(|e| format!("写入历史失败：{e}"))?;
        }
        Ok((visits.len() as u32, skipped))
    }

    /// 在 TabService 的循环里执行一段读写（它是数据库唯一写者），等结果。
    async fn db<T: Send + 'static>(
        &self,
        job: impl FnOnce(&Store) -> T + Send + 'static,
    ) -> Result<T, OpError> {
        let (tx, rx) = oneshot::channel();
        let boxed: tabs::GatewayJob = Box::new(move |t| {
            let _ = tx.send(job(t.store()));
            false
        });
        let stopped = || OpError::new(ErrorCode::Internal, "TabService 已停止");
        self.tabs
            .send(tabs::Request::Gateway(boxed))
            .map_err(|_| stopped())?;
        rx.await.map_err(|_| stopped())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<i64, ImportState>> {
        self.states.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn set_state(
        &self,
        id: i64,
        status: ImportStatus,
        counts: ImportCounts,
        error: Option<String>,
    ) {
        self.lock().insert(
            id,
            ImportState {
                status,
                counts,
                warning: None,
                error,
            },
        );
    }
}

/// 启动时调用：删除上次运行留在 `ALTER_HOME/import-tmp` 里的源数据副本（导入被中断时没来得及删）。
pub fn remove_leftover_copies(home: &Path) {
    let dir = home.join("import-tmp");
    match std::fs::remove_dir_all(&dir) {
        Ok(()) => tracing::info!(path = %dir.display(), "已删除上次留下的导入临时副本"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            tracing::warn!(path = %dir.display(), error = %e, "删除上次留下的导入临时副本失败")
        }
    }
}

/// 按固定顺序（书签、历史、Cookie）排列并去重；None 为书签与历史（Cookie 的导入不在本需求）。
fn in_order(items: Option<&[ImportItem]>) -> Vec<ImportItem> {
    let all = [
        ImportItem::Bookmarks,
        ImportItem::History,
        ImportItem::Cookies,
    ];
    match items {
        None => vec![ImportItem::Bookmarks, ImportItem::History],
        Some(given) => all.into_iter().filter(|i| given.contains(i)).collect(),
    }
}

fn item_name(item: ImportItem) -> &'static str {
    match item {
        ImportItem::Bookmarks => "bookmarks",
        ImportItem::History => "history",
        ImportItem::Cookies => "cookies",
    }
}

fn source_name(source: ImportSource) -> &'static str {
    match source {
        ImportSource::Chrome => "chrome",
        ImportSource::Arc => "arc",
    }
}

fn status_name(status: ImportStatus) -> &'static str {
    match status {
        ImportStatus::Running => "running",
        ImportStatus::Done => "done",
        ImportStatus::Partial => "partial",
        ImportStatus::Failed => "failed",
    }
}

/// 把源 SQLite 库做成 `tmp` 下的一份一致副本，返回副本的路径。先经 SQLite 的 backup API 读取
/// （包括还没 checkpoint 的 WAL，读的是同一个事务时刻），得到一份不带 `-wal` 的独立库；源库打不开或被锁
/// （源浏览器持有独占锁）时退回文件拷贝。
fn snapshot(src: &Path, tmp: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(tmp).map_err(|e| format!("建临时目录失败：{e}"))?;
    let copy = tmp.join(src.file_name().unwrap_or_default());
    match backup(src, &copy) {
        Ok(()) => Ok(copy),
        Err(e) => {
            tracing::warn!(path = %src.display(), error = %e, "源库无法经 SQLite 备份，改为拷贝文件");
            let _ = std::fs::remove_file(&copy);
            copy_in(src, tmp)
        }
    }
}

/// 只读打开 `src`，用 backup API 写出 `dest`；被锁时最多等 1 秒。
fn backup(src: &Path, dest: &Path) -> rusqlite::Result<()> {
    use rusqlite::{MAIN_DB, OpenFlags};
    let conn = rusqlite::Connection::open_with_flags(
        src,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(std::time::Duration::from_secs(1))?;
    conn.backup(MAIN_DB, dest, None)
}

/// 把 SQLite 主文件连同 `-wal` / `-journal` 拷进 `tmp`，返回副本的路径。
fn copy_in(src: &Path, tmp: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(tmp).map_err(|e| format!("建临时目录失败：{e}"))?;
    let name = src.file_name().unwrap_or_default();
    let copy = tmp.join(name);
    std::fs::copy(src, &copy).map_err(|e| format!("拷贝 {} 失败：{e}", src.display()))?;
    for suffix in ["-wal", "-journal"] {
        let mut side = src.as_os_str().to_owned();
        side.push(suffix);
        let side = PathBuf::from(side);
        if side.is_file() {
            let mut dest = copy.as_os_str().to_owned();
            dest.push(suffix);
            std::fs::copy(&side, PathBuf::from(dest))
                .map_err(|e| format!("拷贝 {} 失败：{e}", side.display()))?;
        }
    }
    Ok(copy)
}

/// Shell 入口（导入向导）：按 profile 名称在数据根目录下找到 profile 并开始导入，
/// 把进度转成 `shell.importProgress` 写进发起连接的发件箱。
pub async fn run_for_shell(
    imports: Arc<Imports>,
    request_id: u32,
    source: ImportSource,
    profile: String,
    space_id: String,
    items: Vec<ImportItem>,
    outbox: tabs::Outbox,
) {
    let send = |cmd: ShellCommand| match serde_json::to_string(&cmd) {
        // 连接已断开时没有人要这条消息
        Ok(line) => {
            let _ = outbox.send(line.into());
        }
        Err(e) => tracing::error!(error = %e, "序列化导入进度失败"),
    };
    let failed = |code: ErrorCode, message: &str| {
        let code = serde_json::to_value(code)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        ShellCommand::ImportProgress {
            request_id,
            import_id: 0,
            status: ImportStatus::Failed,
            item: None,
            counts: ImportCounts::default(),
            warning: None,
            error: Some(format!("{code}: {message}")),
        }
    };
    // 只接受列表里那样的子目录名，不接受路径
    if profile.is_empty() || profile.contains('/') || profile == "." || profile == ".." {
        send(failed(
            ErrorCode::InvalidArgument,
            &format!("profile 名称不合法：{profile}"),
        ));
        return;
    }
    let path = sources::root(source).join(&profile);
    let (tx, mut rx) = mpsc::unbounded_channel();
    let req = StartRequest {
        source,
        profile_path: Some(path.to_string_lossy().into_owned()),
        space_id,
        items: Some(items),
    };
    let import_id = match imports.start(req, tx).await {
        Ok(id) => id,
        Err(e) => {
            send(failed(e.code, &e.message));
            return;
        }
    };
    while let Some(ev) = rx.recv().await {
        send(match ev {
            Event::Progress(p) => ShellCommand::ImportProgress {
                request_id,
                import_id,
                status: ImportStatus::Running,
                item: Some(p.item),
                counts: p.counts,
                warning: None,
                error: None,
            },
            Event::Done(d) => ShellCommand::ImportProgress {
                request_id,
                import_id,
                status: d.status,
                item: None,
                counts: d.counts,
                warning: d.warning,
                error: None,
            },
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 建一个源库：`n` 行访问写在表 `t` 里。
    fn source_db(path: &Path, wal: bool, n: i64) -> rusqlite::Connection {
        let conn = rusqlite::Connection::open(path).unwrap();
        if wal {
            conn.pragma_update(None, "journal_mode", "WAL").unwrap();
            // 不自动 checkpoint：写入一直留在 -wal 里
            conn.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
        }
        conn.execute_batch("CREATE TABLE t (x INTEGER)").unwrap();
        for i in 0..n {
            conn.execute("INSERT INTO t VALUES (?1)", [i]).unwrap();
        }
        conn
    }

    fn rows(path: &Path) -> i64 {
        rusqlite::Connection::open(path)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))
            .unwrap()
    }

    /// 源库在 WAL 模式、写入还没 checkpoint 且写连接开着：经 SQLite 备份出一份独立的副本（没有 -wal），
    /// 读得到全部已提交的行。
    #[test]
    fn snapshot_reads_uncheckpointed_wal_into_standalone_copy() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("History");
        let _writer = source_db(&src, true, 50);
        let tmp = dir.path().join("tmp");

        let copy = snapshot(&src, &tmp).unwrap();
        let mut wal = copy.as_os_str().to_owned();
        wal.push("-wal");
        assert!(
            !Path::new(&wal).exists(),
            "副本应当是一份独立的库，不带 -wal"
        );
        assert_eq!(rows(&copy), 50);
    }

    /// 源浏览器持有独占锁时，经 SQLite 读不了：退回文件拷贝，已提交的行仍然读得到。
    #[test]
    fn snapshot_falls_back_to_file_copy_when_locked() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("History");
        let writer = source_db(&src, false, 20);
        writer
            .pragma_update(None, "locking_mode", "EXCLUSIVE")
            .unwrap();
        // 独占锁在第一次写之后取得并一直持有
        writer.execute("INSERT INTO t VALUES (99)", []).unwrap();
        let tmp = dir.path().join("tmp");

        let copy = snapshot(&src, &tmp).unwrap();
        assert_eq!(rows(&copy), 21);
    }
}
