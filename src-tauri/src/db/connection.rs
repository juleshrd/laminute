use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::error::AppResult;

use super::migrations::run_migrations;

pub struct AppState {
    pub db: Mutex<Connection>,
}

#[cfg(test)]
pub fn open_in_memory() -> AppResult<Connection> {
    let mut conn = Connection::open_in_memory()?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    run_migrations(&mut conn)?;
    Ok(conn)
}

pub fn open_and_migrate(path: &Path) -> AppResult<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    run_migrations(&mut conn)?;
    Ok(conn)
}
