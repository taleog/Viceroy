use crate::database;
use anyhow::Result;

pub fn total_history_count() -> Result<usize> {
    let conn = database::get_connection()?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM clipboard_history WHERE deleted_at IS NULL",
        [],
        |row| row.get(0),
    )?;
    Ok(count.max(0) as usize)
}
