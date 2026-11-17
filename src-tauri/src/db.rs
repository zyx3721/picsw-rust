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
    partial INTEGER NOT NULL DEFAULT 0,
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

CREATE TABLE IF NOT EXISTS download_task_images (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id INTEGER NOT NULL REFERENCES conversion_tasks(id) ON DELETE CASCADE,
    document_index INTEGER NOT NULL,
    url TEXT NOT NULL,
    file_name TEXT NOT NULL DEFAULT '',
    saved_path TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'pending',
    error TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_download_task_images_task_id ON download_task_images(task_id);

CREATE TABLE IF NOT EXISTS conversion_task_documents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id INTEGER NOT NULL REFERENCES conversion_tasks(id) ON DELETE CASCADE,
    filename TEXT NOT NULL,
    total_images INTEGER NOT NULL DEFAULT 0,
    processed_images INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'queued',
    error TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_conversion_task_documents_task_id ON conversion_task_documents(task_id);

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
    ensure_column(&conn, "conversion_tasks", "partial", "partial INTEGER NOT NULL DEFAULT 0")?;
    Ok(conn)
}

fn ensure_column(conn: &Connection, table: &str, column: &str, definition: &str) -> Result<(), String> {
    let existing = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|e| e.to_string())?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .filter_map(|row| row.ok())
        .any(|name| name == column);
    if existing {
        return Ok(());
    }
    conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {definition}"))
        .map_err(|e| e.to_string())
}
