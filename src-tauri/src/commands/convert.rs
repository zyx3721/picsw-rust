use serde::Deserialize;
use serde_json::json;
use serde_json::Value;
use tauri::State;

use crate::engine::{self, load_record, load_record_with_details, LocalBatchManifest, MarkdownRequest};
use crate::models::{now_string, ConversionRecord};
use crate::task_queue::{self, safe_local_task_name};
use crate::AppState;

const MAX_LOCAL_BATCH_BYTES: usize = 256 << 20;

#[derive(Debug, Deserialize)]
pub struct CreateConvertTaskRequest {
    #[serde(default)]
    pub files: Vec<MarkdownRequest>,
    #[serde(default)]
    pub target_config_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct LocalImageUpload {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub data: String,
}

#[derive(Debug, Deserialize)]
pub struct DeleteRecordsRequest {
    #[serde(default)]
    pub ids: Vec<i64>,
}

fn lock_db(state: &AppState) -> Result<std::sync::MutexGuard<'_, rusqlite::Connection>, String> {
    state.db.lock().map_err(|_| "数据库连接异常".to_string())
}

#[tauri::command]
pub fn analyze_markdown(req: MarkdownRequest) -> Value {
    let images = crate::markdown::extract_markdown_images(&req.content);
    let mut counts = std::collections::HashMap::new();
    for image in &images {
        *counts.entry(image.picbed.clone()).or_insert(0) += 1;
    }
    json!({ "images": images, "counts": counts, "total": images.len() })
}

#[tauri::command]
pub async fn create_convert_task(state: State<'_, AppState>, req: CreateConvertTaskRequest) -> Result<Value, String> {
    let mut files = req.files;
    if files.is_empty() {
        return Err("请至少上传一个 Markdown 文件".to_string());
    }
    if files.len() > 20 {
        return Err("单次最多转换 20 个 Markdown 文件".to_string());
    }
    for file in files.iter_mut() {
        if file.target_config_id == 0 {
            file.target_config_id = req.target_config_id;
        }
        if file.target_config_id == 0 {
            return Err("请先选择目标图床配置".to_string());
        }
    }
    let payload = serde_json::to_string(&files).map_err(|_| "转换任务载荷生成失败".to_string())?;
    let task_id = {
        let conn = lock_db(&state)?;
        conn.execute(
            "INSERT INTO conversion_tasks (task_type, status, total, success, failed, message, payload, created_at, updated_at) VALUES ('convert', 'queued', ?1, 0, 0, '转换任务已加入队列', ?2, ?3, ?4)",
            rusqlite::params![files.len() as i64, payload, now_string(), now_string()],
        )
        .map_err(|_| "创建转换任务失败".to_string())?;
        conn.last_insert_rowid()
    };
    if let Err(err) = task_queue::enqueue(&state, task_id).await {
        let _ = lock_db(&state).map(|conn| {
            conn.execute(
                "UPDATE conversion_tasks SET status = 'failed', message = '转换任务入队失败', error = ?2, ended_at = ?3, updated_at = ?4 WHERE id = ?1",
                rusqlite::params![task_id, err, now_string(), now_string()],
            )
        });
        return Err("转换任务入队失败".to_string());
    }
    let task = {
        let conn = lock_db(&state)?;
        load_task_json(&conn, task_id).ok_or("创建转换任务失败")?
    };
    Ok(json!({ "task": task, "results": [] }))
}

#[tauri::command]
pub async fn create_local_convert_task(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    manifest: LocalBatchManifest,
    images: Vec<LocalImageUpload>,
) -> Result<Value, String> {
    if manifest.target_config_id == 0 {
        return Err("请先选择目标图床配置".to_string());
    }
    if manifest.documents.is_empty() {
        return Err("请至少上传一个 Markdown 文档".to_string());
    }
    if manifest.documents.len() > 20 {
        return Err("单次最多处理 20 个 Markdown 文档".to_string());
    }
    let total_bytes: usize = images.iter().map(|image| image.data.len()).sum();
    if total_bytes > MAX_LOCAL_BATCH_BYTES {
        return Err("本地图片总大小不能超过 256MB".to_string());
    }
    {
        let conn = lock_db(&state)?;
        engine::find_config_by_id(&conn, manifest.target_config_id).ok_or("目标图床配置不存在")?;
    }

    let task_id = {
        let conn = lock_db(&state)?;
        conn.execute(
            "INSERT INTO conversion_tasks (task_type, status, total, success, failed, message, payload, created_at, updated_at) VALUES ('local_upload', 'queued', ?1, 0, 0, '本地上传任务已加入队列', '', ?2, ?3)",
            rusqlite::params![manifest.documents.len() as i64, now_string(), now_string()],
        )
        .map_err(|_| "创建本地上传任务失败".to_string())?;
        conn.last_insert_rowid()
    };

    let task_dir = task_queue::local_task_dir(&app, task_id);
    if let Err(err) = store_local_task_files(&task_dir, &images) {
        let _ = std::fs::remove_dir_all(&task_dir);
        fail_task(&state, task_id, "保存本地上传文件失败", &err);
        return Err("保存本地上传文件失败".to_string());
    }

    let mut files_payload: std::collections::BTreeMap<String, Vec<Value>> = std::collections::BTreeMap::new();
    for image in &images {
        let key = image.key.trim().to_string();
        if key.is_empty() {
            continue;
        }
        let safe_name = safe_local_task_name(&image.name);
        let path = task_dir.join(format!("{}-{}", safe_local_task_name(&key), safe_name));
        files_payload
            .entry(key)
            .or_default()
            .push(json!({ "filename": image.name, "path": path.to_string_lossy() }));
    }
    let payload = serde_json::to_string(&json!({ "manifest": manifest, "files": files_payload }))
        .map_err(|_| "本地上传任务载荷生成失败".to_string())?;
    {
        let conn = lock_db(&state)?;
        conn.execute("UPDATE conversion_tasks SET payload = ?2, updated_at = ?3 WHERE id = ?1", rusqlite::params![task_id, payload, now_string()])
            .map_err(|_| "保存本地上传任务载荷失败".to_string())?;
    }

    if let Err(err) = task_queue::enqueue(&state, task_id).await {
        let _ = std::fs::remove_dir_all(&task_dir);
        fail_task(&state, task_id, "本地上传任务入队失败", &err);
        return Err("本地上传任务入队失败".to_string());
    }
    let task = {
        let conn = lock_db(&state)?;
        load_task_json(&conn, task_id).ok_or("创建本地上传任务失败")?
    };
    Ok(json!({ "task": task, "results": [] }))
}

#[tauri::command]
pub fn list_convert_tasks(state: State<AppState>) -> Result<Value, String> {
    let conn = lock_db(&state)?;
    let mut stmt = conn
        .prepare("SELECT id FROM conversion_tasks ORDER BY created_at DESC LIMIT 50")
        .map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| row.get::<_, i64>(0)).map_err(|e| e.to_string())?;
    let ids: Vec<i64> = rows.filter_map(|row| row.ok()).collect();
    let mut tasks: Vec<Value> = Vec::new();
    for id in ids {
        if let Some(task) = load_task_json(&conn, id) {
            tasks.push(task);
        }
    }
    Ok(json!({ "tasks": tasks }))
}

#[tauri::command]
pub fn get_convert_task(state: State<AppState>, id: i64) -> Result<Value, String> {
    let conn = lock_db(&state)?;
    if id <= 0 {
        return Err("任务 ID 不正确".to_string());
    }
    let task = load_task_json(&conn, id).ok_or("转换任务不存在")?;
    let mut stmt = conn
        .prepare("SELECT id FROM conversion_records WHERE task_id = ?1 ORDER BY id ASC")
        .map_err(|e| e.to_string())?;
    let rows = stmt.query_map([id], |row| row.get::<_, i64>(0)).map_err(|e| e.to_string())?;
    let ids: Vec<i64> = rows.filter_map(|row| row.ok()).collect();
    let records: Vec<ConversionRecord> = ids
        .iter()
        .filter_map(|record_id| load_record_with_details(&conn, *record_id))
        .collect();
    Ok(json!({ "task": task, "records": records }))
}

#[tauri::command]
pub fn list_records(state: State<AppState>) -> Result<Value, String> {
    let conn = lock_db(&state)?;
    let mut stmt = conn
        .prepare("SELECT id FROM conversion_records ORDER BY created_at DESC LIMIT 50")
        .map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| row.get::<_, i64>(0)).map_err(|e| e.to_string())?;
    let ids: Vec<i64> = rows.filter_map(|row| row.ok()).collect();
    let records: Vec<ConversionRecord> = ids.iter().filter_map(|id| load_record(&conn, *id)).collect();
    Ok(json!({ "records": records }))
}

#[tauri::command]
pub fn get_record(state: State<AppState>, id: i64) -> Result<Value, String> {
    if id <= 0 {
        return Err("记录 ID 不正确".to_string());
    }
    let conn = lock_db(&state)?;
    let record = load_record_with_details(&conn, id).ok_or("转换记录不存在")?;
    Ok(json!({ "record": record }))
}

#[tauri::command]
pub fn delete_records(state: State<AppState>, req: DeleteRecordsRequest) -> Result<Value, String> {
    let mut seen = std::collections::HashSet::new();
    let ids: Vec<i64> = req
        .ids
        .into_iter()
        .filter(|id| *id > 0 && seen.insert(*id))
        .collect();
    if ids.is_empty() {
        return Err("请选择要删除的历史记录".to_string());
    }
    if ids.len() > 50 {
        return Err("单次最多删除 50 条历史记录".to_string());
    }
    let conn = lock_db(&state)?;
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    let params: Vec<&dyn rusqlite::ToSql> = ids.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
    let deleted = conn
        .execute(
            &format!("DELETE FROM conversion_records WHERE id IN ({})", placeholders),
            params.as_slice(),
        )
        .map_err(|_| "删除转换记录失败".to_string())?;
    if deleted == 0 {
        return Err("请选择要删除的历史记录".to_string());
    }
    Ok(json!({ "message": format!("已删除 {} 条转换记录", deleted) }))
}

fn fail_task(state: &State<'_, AppState>, task_id: i64, message: &str, error: &str) {
    let _ = lock_db(state).map(|conn| {
        conn.execute(
            "UPDATE conversion_tasks SET status = 'failed', message = ?2, error = ?3, ended_at = ?4, updated_at = ?5 WHERE id = ?1",
            rusqlite::params![task_id, message, error, now_string(), now_string()],
        )
    });
}

fn load_task_json(conn: &rusqlite::Connection, task_id: i64) -> Option<Value> {
    conn.query_row(
        "SELECT id, task_type, status, total, success, failed, message, error, started_at, ended_at, created_at, updated_at FROM conversion_tasks WHERE id = ?1",
        [task_id],
        |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "task_type": row.get::<_, String>(1)?,
                "status": row.get::<_, String>(2)?,
                "total": row.get::<_, i64>(3)?,
                "success": row.get::<_, i64>(4)?,
                "failed": row.get::<_, i64>(5)?,
                "message": row.get::<_, String>(6)?,
                "error": row.get::<_, Option<String>>(7)?,
                "started_at": row.get::<_, Option<String>>(8)?,
                "ended_at": row.get::<_, Option<String>>(9)?,
                "created_at": row.get::<_, String>(10)?,
                "updated_at": row.get::<_, String>(11)?,
            }))
        },
    )
    .ok()
}

fn store_local_task_files(task_dir: &std::path::Path, images: &[LocalImageUpload]) -> Result<(), String> {
    std::fs::create_dir_all(task_dir).map_err(|e| e.to_string())?;
    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::Engine;
    for image in images {
        let key = image.key.trim();
        if key.is_empty() {
            continue;
        }
        let data = BASE64
            .decode(image.data.as_bytes())
            .map_err(|_| format!("本地图片 {} 解码失败", image.name))?;
        let filename = format!("{}-{}", safe_local_task_name(key), safe_local_task_name(&image.name));
        let path = task_dir.join(filename);
        std::fs::write(&path, data).map_err(|e| e.to_string())?;
    }
    Ok(())
}
