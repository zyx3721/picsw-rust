use rusqlite::Connection;
use std::path::Path;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS picbed_configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    picbed_type TEXT NOT NULL,
    config_name TEXT NOT NULL UNIQUE,
    encrypted_config TEXT NOT NULL,
    is_default INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_picbed_configs_default ON picbed_configs(is_default) WHERE is_default = 1;

CREATE TABLE IF NOT EXISTS conversion_tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_type TEXT NOT NULL,
    status TEXT NOT NULL,
    total INTEGER NOT NULL DEFAULT 0,
    success INTEGER NOT NULL DEFAULT 0,
    failed INTEGER NOT NULL DEFAULT 0,
    message TEXT NOT NULL DEFAULT '',
    payload TEXT NOT NULL DEFAULT '',
    error TEXT NOT NULL DEFAULT '',
    started_at TEXT,
    ended_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_conversion_tasks_status ON conversion_tasks(status);

CREATE TABLE IF NOT EXISTS conversion_records (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    original_filename TEXT NOT NULL,
    source_picbed TEXT NOT NULL,
    target_picbed TEXT NOT NULL,
    status TEXT NOT NULL,
    error_message TEXT NOT NULL DEFAULT '',
    image_count INTEGER NOT NULL DEFAULT 0,
    task_id INTEGER REFERENCES conversion_tasks(id) ON DELETE SET NULL,
    converted_content TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_conversion_records_created_at ON conversion_records(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_conversion_records_task_id ON conversion_records(task_id);

CREATE TABLE IF NOT EXISTS conversion_record_details (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    record_id INTEGER NOT NULL REFERENCES conversion_records(id) ON DELETE CASCADE,
    original_url TEXT NOT NULL,
    target_url TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL,
    error TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_conversion_record_details_record_id ON conversion_record_details(record_id);
"#;

pub fn init_db(data_dir: &Path) -> Result<Connection, String> {
    std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    let conn = Connection::open(data_dir.join("picbed.db")).map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")
        .map_err(|e| e.to_string())?;
    conn.execute_batch(SCHEMA).map_err(|e| e.to_string())?;
    Ok(conn)
}
