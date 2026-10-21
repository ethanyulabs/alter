//! SQLite 存储：打开数据目录下的 `alter.db` 并应用迁移。

mod migrate;

use std::path::{Path, PathBuf};

use rusqlite::Connection;

/// 打开或迁移数据库时的错误。
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("无法创建数据目录：{0}")]
    Io(#[from] std::io::Error),
    #[error("SQLite 错误：{0}")]
    Sqlite(#[from] rusqlite::Error),
}

/// alterd 持有的唯一写连接。
pub struct Store {
    conn: Connection,
}

/// 数据目录下数据库文件的路径。
pub fn db_path(home: &Path) -> PathBuf {
    home.join("alter.db")
}

impl Store {
    /// 打开 `home` 下的 `alter.db`，不存在则创建，并应用全部未应用的迁移。
    pub fn open(home: &Path) -> Result<Store, StoreError> {
        std::fs::create_dir_all(home)?;
        let mut conn = Connection::open(db_path(home))?;
        // 两者都必须在迁移事务之外设置：WAL 不能在事务内切换，外键开关是每个连接各自的
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", true)?;
        migrate::apply(&mut conn)?;
        Ok(Store { conn })
    }

    /// 底层连接，供存储层之上的读写使用。
    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}
