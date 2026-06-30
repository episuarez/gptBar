//! Usage history storage using SQLite
//!
//! Persists usage snapshots per provider for sparkline graphs and trend analysis.

use std::path::PathBuf;

use crate::config::AppConfig;
use crate::providers::UsageSnapshot;

/// A single history entry stored in the database
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HistoryEntry {
    pub provider_id: String,
    pub timestamp: String, // ISO 8601
    pub primary_percent: Option<f64>,
    pub secondary_percent: Option<f64>,
    pub tertiary_percent: Option<f64>,
}

/// Returns the path to the history SQLite database
fn db_path() -> Option<PathBuf> {
    AppConfig::config_dir_pub().map(|d| d.join("history.db"))
}

/// Opens (or creates) the history database and ensures the schema exists
fn open_db() -> Result<rusqlite::Connection, String> {
    let path = db_path().ok_or("Could not determine history database path")?;

    // Create directory if needed
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create history dir: {}", e))?;
    }

    let conn = rusqlite::Connection::open(&path)
        .map_err(|e| format!("Failed to open history DB: {}", e))?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS usage_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            provider_id TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            primary_percent REAL,
            secondary_percent REAL,
            tertiary_percent REAL
        );
        CREATE INDEX IF NOT EXISTS idx_history_provider_ts
            ON usage_history (provider_id, timestamp);",
    )
    .map_err(|e| format!("Failed to create history table: {}", e))?;

    Ok(conn)
}

/// Saves a usage snapshot to history
pub fn save_snapshot(provider_id: &str, snapshot: &UsageSnapshot) -> Result<(), String> {
    let conn = open_db()?;

    conn.execute(
        "INSERT INTO usage_history (provider_id, timestamp, primary_percent, secondary_percent, tertiary_percent)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            provider_id,
            snapshot.updated_at.to_rfc3339(),
            snapshot.primary.as_ref().map(|w| w.used_percent),
            snapshot.secondary.as_ref().map(|w| w.used_percent),
            snapshot.tertiary.as_ref().map(|w| w.used_percent),
        ],
    )
    .map_err(|e| format!("Failed to insert history entry: {}", e))?;

    // Keep only the last 200 entries per provider to avoid unbounded growth
    conn.execute(
        "DELETE FROM usage_history WHERE id IN (
            SELECT id FROM usage_history
            WHERE provider_id = ?1
            ORDER BY timestamp DESC
            LIMIT -1 OFFSET 200
        )",
        rusqlite::params![provider_id],
    )
    .map_err(|e| format!("Failed to prune history: {}", e))?;

    Ok(())
}

/// Returns the last N history entries for a provider (most recent first)
pub fn get_history(provider_id: &str, limit: usize) -> Result<Vec<HistoryEntry>, String> {
    let conn = open_db()?;

    let mut stmt = conn
        .prepare(
            "SELECT provider_id, timestamp, primary_percent, secondary_percent, tertiary_percent
             FROM usage_history
             WHERE provider_id = ?1
             ORDER BY timestamp DESC
             LIMIT ?2",
        )
        .map_err(|e| format!("Failed to prepare history query: {}", e))?;

    let entries = stmt
        .query_map(rusqlite::params![provider_id, limit as i64], |row| {
            Ok(HistoryEntry {
                provider_id: row.get(0)?,
                timestamp: row.get(1)?,
                primary_percent: row.get(2)?,
                secondary_percent: row.get(3)?,
                tertiary_percent: row.get(4)?,
            })
        })
        .map_err(|e| format!("Failed to query history: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to read history rows: {}", e))?;

    Ok(entries)
}

/// Clears all history for a provider
pub fn clear_history(provider_id: &str) -> Result<(), String> {
    let conn = open_db()?;
    conn.execute(
        "DELETE FROM usage_history WHERE provider_id = ?1",
        rusqlite::params![provider_id],
    )
    .map_err(|e| format!("Failed to clear history: {}", e))?;
    Ok(())
}

/// Exports history for all providers as JSON
pub fn export_history_json() -> Result<String, String> {
    let conn = open_db()?;

    let mut stmt = conn
        .prepare(
            "SELECT provider_id, timestamp, primary_percent, secondary_percent, tertiary_percent
             FROM usage_history
             ORDER BY provider_id, timestamp DESC",
        )
        .map_err(|e| format!("Failed to prepare export query: {}", e))?;

    let entries: Vec<HistoryEntry> = stmt
        .query_map([], |row| {
            Ok(HistoryEntry {
                provider_id: row.get(0)?,
                timestamp: row.get(1)?,
                primary_percent: row.get(2)?,
                secondary_percent: row.get(3)?,
                tertiary_percent: row.get(4)?,
            })
        })
        .map_err(|e| format!("Failed to query export: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to read export rows: {}", e))?;

    serde_json::to_string_pretty(&entries).map_err(|e| format!("Failed to serialize: {}", e))
}

/// Exports history for all providers as CSV
pub fn export_history_csv() -> Result<String, String> {
    let conn = open_db()?;

    let mut stmt = conn
        .prepare(
            "SELECT provider_id, timestamp, primary_percent, secondary_percent, tertiary_percent
             FROM usage_history
             ORDER BY provider_id, timestamp DESC",
        )
        .map_err(|e| format!("Failed to prepare CSV query: {}", e))?;

    let mut csv =
        String::from("provider_id,timestamp,primary_percent,secondary_percent,tertiary_percent\n");

    stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<f64>>(2)?,
            row.get::<_, Option<f64>>(3)?,
            row.get::<_, Option<f64>>(4)?,
        ))
    })
    .map_err(|e| format!("Failed to query CSV: {}", e))?
    .try_for_each(|row| {
        let (pid, ts, p, s, t) = row.map_err(|e| format!("Row error: {}", e))?;
        let fmt = |v: Option<f64>| v.map(|x| format!("{:.2}", x)).unwrap_or_default();
        csv.push_str(&format!(
            "{},{},{},{},{}\n",
            pid,
            ts,
            fmt(p),
            fmt(s),
            fmt(t)
        ));
        Ok::<_, String>(())
    })?;

    Ok(csv)
}
