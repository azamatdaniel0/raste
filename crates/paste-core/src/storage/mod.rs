pub mod item_repo;
pub mod pinboard_repo;
pub mod prefs_repo;
pub mod schema;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use directories::ProjectDirs;
use rusqlite::Connection;

use crate::error::Result;

/// Thread-safe, clonable handle to the SQLite two-tier clipboard storage.
#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    /// Opens (or creates) the SQLite database at the given file path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        schema::init_schema(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Opens an isolated in-memory SQLite database (useful for tests).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        schema::init_schema(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Returns the standard cross-platform OS data path for `new-paste.db`.
    /// Respects `NEW_PASTE_DB_PATH` if set (handy for MCP stdio / custom profiles).
    pub fn default_db_path() -> PathBuf {
        if let Ok(custom) = std::env::var("NEW_PASTE_DB_PATH") {
            if !custom.trim().is_empty() {
                return PathBuf::from(custom);
            }
        }
        if let Some(proj) = ProjectDirs::from("io", "artcraft", "new-paste") {
            return proj.data_local_dir().join("new-paste.db");
        }
        PathBuf::from("new-paste.db")
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().expect("database mutex poisoned")
    }
}
