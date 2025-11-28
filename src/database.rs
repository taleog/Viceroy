use rusqlite::{Connection, Result};
use std::path::PathBuf;

pub fn init() -> Result<()> {
    let db_path = get_db_path();
    let conn = Connection::open(&db_path)?;

    // Create clipboard history table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS clipboard_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            content TEXT NOT NULL,
            content_type TEXT NOT NULL,
            app_name TEXT,
            timestamp INTEGER NOT NULL,
            custom_name TEXT,
            image_width INTEGER,
            image_height INTEGER,
            is_favorite INTEGER DEFAULT 0,
            is_pinned INTEGER DEFAULT 0
        )",
        [],
    )?;

    // Add is_pinned column if it doesn't exist (for existing databases)
    conn.execute(
        "ALTER TABLE clipboard_history ADD COLUMN is_pinned INTEGER DEFAULT 0",
        [],
    )
    .ok(); // Ignore error if column already exists

    // Add image dimension columns for clipboard images
    conn.execute(
        "ALTER TABLE clipboard_history ADD COLUMN image_width INTEGER",
        [],
    )
    .ok();
    conn.execute(
        "ALTER TABLE clipboard_history ADD COLUMN image_height INTEGER",
        [],
    )
    .ok(); // Ignore error if column already exists

    // Create index for faster searches
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_clipboard_timestamp ON clipboard_history(timestamp DESC)",
        [],
    )?;

    Ok(())
}

pub fn get_db_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("viceroy");
    std::fs::create_dir_all(&path).ok();
    path.push("clipboard.db");
    path
}

pub fn get_connection() -> Result<Connection> {
    Connection::open(get_db_path())
}
