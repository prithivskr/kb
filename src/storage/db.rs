use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::Connection;

const APP_NAME: &str = "kanban";
const DB_FILENAME: &str = "kanban-v2.db";

pub fn default_db_path() -> Result<PathBuf> {
    if let Some(base) = dirs::data_local_dir() {
        return Ok(base.join(APP_NAME).join(DB_FILENAME));
    }

    let home = dirs::home_dir().context("could not resolve home directory")?;
    Ok(home
        .join(".local")
        .join("share")
        .join(APP_NAME)
        .join(DB_FILENAME))
}

pub fn ensure_db_parent_exists(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create database directory at {}",
                parent.display()
            )
        })?;
    }
    Ok(())
}

pub fn open_connection(path: &Path) -> Result<Connection> {
    ensure_db_parent_exists(path)?;
    let conn = Connection::open(path)
        .with_context(|| format!("failed to open sqlite database at {}", path.display()))?;

    conn.execute_batch(
        "PRAGMA foreign_keys = ON;\nPRAGMA journal_mode = WAL;\nPRAGMA synchronous = NORMAL;\nPRAGMA busy_timeout = 5000;",
    )
    .context("failed to apply sqlite pragmas")?;

    Ok(conn)
}

pub fn open_default_connection() -> Result<Connection> {
    let path = default_db_path()?;
    open_connection(&path)
}

#[cfg(test)]
mod tests {
    use super::open_connection;

    #[test]
    fn opens_db_and_applies_foreign_key_pragma() {
        let root = std::env::temp_dir().join(format!("kb-storage-{}", uuid::Uuid::new_v4()));
        let path = root.join("kanban.db");

        let conn = open_connection(&path).expect("connection should open");
        let foreign_keys: i64 = conn
            .query_row("PRAGMA foreign_keys;", [], |row| row.get(0))
            .expect("pragma query should succeed");
        assert_eq!(foreign_keys, 1);

        std::fs::remove_dir_all(root).expect("temporary db directory should be removable");
    }
}
