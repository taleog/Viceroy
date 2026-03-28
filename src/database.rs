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
            is_pinned INTEGER DEFAULT 0,
            sync_id TEXT,
            source_device_id TEXT,
            source_device_name TEXT,
            updated_at INTEGER,
            deleted_at INTEGER
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
    conn.execute("ALTER TABLE clipboard_history ADD COLUMN sync_id TEXT", [])
        .ok();
    conn.execute(
        "ALTER TABLE clipboard_history ADD COLUMN source_device_id TEXT",
        [],
    )
    .ok();
    conn.execute(
        "ALTER TABLE clipboard_history ADD COLUMN source_device_name TEXT",
        [],
    )
    .ok();
    conn.execute(
        "ALTER TABLE clipboard_history ADD COLUMN updated_at INTEGER",
        [],
    )
    .ok();
    conn.execute(
        "ALTER TABLE clipboard_history ADD COLUMN deleted_at INTEGER",
        [],
    )
    .ok();

    conn.execute(
        "CREATE TABLE IF NOT EXISTS sync_devices (
            device_id TEXT PRIMARY KEY,
            device_name TEXT NOT NULL,
            platform TEXT NOT NULL,
            is_current INTEGER NOT NULL DEFAULT 0,
            first_seen_at INTEGER NOT NULL,
            last_seen_at INTEGER NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS sync_outbox (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            entry_sync_id TEXT NOT NULL,
            operation TEXT NOT NULL,
            payload TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            sent_at INTEGER,
            attempts INTEGER NOT NULL DEFAULT 0,
            last_error TEXT
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS sync_state (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
        [],
    )?;

    // Create index for faster searches
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_clipboard_timestamp ON clipboard_history(timestamp DESC)",
        [],
    )?;
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_clipboard_sync_id ON clipboard_history(sync_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_clipboard_deleted_at ON clipboard_history(deleted_at)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sync_outbox_pending ON sync_outbox(sent_at, created_at)",
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
