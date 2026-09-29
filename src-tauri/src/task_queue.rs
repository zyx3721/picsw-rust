use serde::Deserialize;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::sync::mpsc;

use crate::engine::{self, LocalBatchManifest, MarkdownRequest};
use crate::models::now_string;
use crate::uploader;
use crate::AppState;

/// 单个任务内文档级转换并发数
const CONVERT_DOCUMENT_CONCURRENCY: usize = 3;

pub fn start_workers(app: AppHandle, mut receiver: mpsc::Receiver<i64>) {
    tauri::async_runtime::spawn(async move {
        requeue_queued_tasks(&app).await;
        while let Some(task_id) = receiver.recv().await {
            run_convert_task(&app, task_id).await;
        }
    });
}

pub async fn enqueue(state: &AppState, task_id: i64) -> Result<(), String> {
    state
        .queue
        .send(task_id)
        .await
        .map_err(|_| "本地转换队列已关闭".to_string())
}

async fn requeue_queued_tasks(app: &AppHandle) {
    let Some(ids) = with_db(app, |conn| {
        let mut stmt = match conn.prepare("SELECT id FROM conversion_tasks WHERE status = 'queued' ORDER BY created_at ASC") {
            Ok(stmt) => stmt,
            Err(_) => return Vec::new(),
        };
        let rows = stmt.query_map([], |row| row.get::<_, i64>(0));
        match rows {
            Ok(rows) => rows.filter_map(|row| row.ok()).collect(),
            Err(_) => Vec::new(),
        }
    }) else {
        return;
    };
    if let Some(state) = app.try_state::<AppState>() {
        for id in ids {
            let _ = enqueue(&state, id).await;
        }
    }
}

fn with_db<T>(app: &AppHandle, f: impl FnOnce(&rusqlite::Connection) -> T) -> Option<T> {
    let state = app.try_state::<AppState>()?;
    let conn = state.db.lock().ok()?;
    Some(f(&conn))
}

async fn run_convert_task(app: &AppHandle, task_id: i64) {
    let Some(task) = with_db(app, |conn| load_task(conn, task_id)) else { return };
    let Some(task) = task else { return };
    if task.status != "queued" {
        return;
    }
    let claimed = with_db(app, |conn| {
        conn.execute(
            "UPDATE conversion_tasks SET status = 'running', message = '转换任务执行中', started_at = ?2, updated_at = ?3 WHERE id = ?1 AND status = 'queued'",
            rusqlite::params![task_id, now_string(), now_string()],
        )
        .map(|rows| rows > 0)
        .unwrap_or(false)
    });
    if !claimed.unwrap_or(false) {
        return;
    }

    if task.task_type == engine::LOCAL_CONVERT_TASK_TYPE {
        run_local_convert_task(app, task_id).await;
        return;
    }
    if task.task_type != "convert" {
        fail_task(app, task_id, "不支持的转换任务类型", &format!("task_type: {}", task.task_type));
        return;
    }
    let files: Vec<MarkdownRequest> = match serde_json::from_str::<Vec<MarkdownRequest>>(&task.payload) {
        Ok(files) => files,
        Err(err) => {
            fail_task(app, task_id, "转换任务载荷解析失败", &err.to_string());
            return;
        }
    };

    let total = files.len();
    let _ = with_db(app, |conn| {
        conn.execute(
            "UPDATE conversion_tasks SET message = ?2, updated_at = ?3 WHERE id = ?1",
            rusqlite::params![
                task_id,
                format!("共 {} 个文档，正在按并发度 {} 转换", total, CONVERT_DOCUMENT_CONCURRENCY),
                now_string()
            ],
        )
    });

    let semaphore = Arc::new(tokio::sync::Semaphore::new(CONVERT_DOCUMENT_CONCURRENCY));
    let mut set = tokio::task::JoinSet::new();
    for file in files {
        let permit = semaphore.clone().acquire_owned().await.expect("semaphore open");
        let app = app.clone();
        set.spawn(async move {
            let state = app.state::<AppState>();
            let result = engine::convert_one(&state, &file, Some(task_id)).await;
            drop(permit);
            match result {
                Ok(_) => Ok(()),
                Err(err) => Err(format!("{}：{}", document_filename(&file.filename), err)),
            }
        });
    }

    let mut success = 0i64;
    let mut failed = 0i64;
    let mut done = 0usize;
    let mut first_error = String::new();
    while let Some(joined) = set.join_next().await {
        match joined.unwrap_or_else(|error| Err(error.to_string())) {
            Ok(()) => success += 1,
            Err(err) => {
                failed += 1;
                if first_error.is_empty() {
                    first_error = err;
                }
            }
        }
        done += 1;
        let message = format!("已完成 {} / {} 个文档", done, total);
        let _ = with_db(app, |conn| {
            conn.execute(
                "UPDATE conversion_tasks SET success = ?2, failed = ?3, message = ?4, updated_at = ?5 WHERE id = ?1",
                rusqlite::params![task_id, success, failed, message, now_string()],
            )
        });
    }

    let status = if failed > 0 { "failed" } else { "success" };
    let message = format!("转换完成，成功 {} 个，失败 {} 个", success, failed);
    let _ = with_db(app, |conn| {
        conn.execute(
            "UPDATE conversion_tasks SET status = ?2, success = ?3, failed = ?4, message = ?5, error = ?6, ended_at = ?7, updated_at = ?8 WHERE id = ?1",
            rusqlite::params![task_id, status, success, failed, message, first_error, now_string(), now_string()],
        )
    });
}

async fn run_local_convert_task(app: &AppHandle, task_id: i64) {
    let cleanup_dir = local_task_dir(app, task_id);
    struct CleanupGuard {
        dir: std::path::PathBuf,
    }
    impl Drop for CleanupGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
    let _cleanup = CleanupGuard { dir: cleanup_dir };

    let payload_result = with_db(app, |conn| {
        let task = load_task(conn, task_id)?;
        let payload: LocalTaskPayload = serde_json::from_str(&task.payload).ok()?;
        Some((payload.manifest, payload.files))
    })
    .flatten();
    let Some((payload_manifest, payload_files)) = payload_result else {
        fail_task(app, task_id, "本地上传任务载荷解析失败", "payload missing");
        return;
    };
    if let Err(err) = validate_local_manifest(&payload_manifest) {
        fail_task(app, task_id, "本地上传任务载荷不正确", &err);
        return;
    }
    let target = with_db(app, |conn| engine::find_config_by_id(conn, payload_manifest.target_config_id)).flatten();
    let Some(target) = target else {
        fail_task(app, task_id, "目标图床配置不存在", "");
        return;
    };
    let target_config = with_db(app, |_conn| engine::decrypt_config_map(&target.encrypted_config).ok()).flatten();
    let Some(target_config) = target_config else {
        fail_task(app, task_id, "读取目标图床配置失败", "");
        return;
    };

    let stored_files = payload_files;
    let load_image: engine::LocalImageLoader = Arc::new(move |file_key: &str| {
        let entries = stored_files.get(file_key).ok_or_else(|| format!("本地图片文件 {} 不存在", file_key))?;
        let entry = entries.first().ok_or_else(|| format!("本地图片文件 {} 不存在", file_key))?;
        let data = std::fs::read(&entry.path).map_err(|e| e.to_string())?;
        uploader::new_image_file(&entry.filename, data)
    });

    let upload_cache: engine::LocalUploadCache = Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));
    let total_documents = payload_manifest.documents.len();
    let _ = with_db(app, |conn| {
        conn.execute(
            "UPDATE conversion_tasks SET message = ?2, updated_at = ?3 WHERE id = ?1",
            rusqlite::params![
                task_id,
                format!("共 {} 个文档，正在按并发度 {} 上传替换", total_documents, CONVERT_DOCUMENT_CONCURRENCY),
                now_string()
            ],
        )
    });

    let target = Arc::new(target);
    let target_config = Arc::new(target_config);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(CONVERT_DOCUMENT_CONCURRENCY));
    let mut set = tokio::task::JoinSet::new();
    for document in payload_manifest.documents {
        let permit = semaphore.clone().acquire_owned().await.expect("semaphore open");
        let app = app.clone();
        let target = target.clone();
        let target_config = target_config.clone();
        let load_image = load_image.clone();
        let upload_cache = upload_cache.clone();
        set.spawn(async move {
            let state = app.state::<AppState>();
            let result =
                engine::convert_local_one(&state, &document, &target, &target_config, load_image, &upload_cache, Some(task_id)).await;
            drop(permit);
            match result {
                Ok(_) => Ok(()),
                Err(err) => Err(format!("{}：{}", document_filename(&document.filename), err)),
            }
        });
    }

    let mut success = 0i64;
    let mut failed = 0i64;
    let mut done = 0usize;
    let mut first_error = String::new();
    while let Some(joined) = set.join_next().await {
        match joined.unwrap_or_else(|error| Err(error.to_string())) {
            Ok(()) => success += 1,
            Err(err) => {
                failed += 1;
                if first_error.is_empty() {
                    first_error = err;
                }
            }
        }
        done += 1;
        let message = format!("已完成 {} / {} 个文档", done, total_documents);
        let _ = with_db(app, |conn| {
            conn.execute(
                "UPDATE conversion_tasks SET success = ?2, failed = ?3, message = ?4, updated_at = ?5 WHERE id = ?1",
                rusqlite::params![task_id, success, failed, message, now_string()],
            )
        });
    }

    let status = if failed > 0 { "failed" } else { "success" };
    let message = format!("本地图片上传完成，成功 {} 个，失败 {} 个", success, failed);
    let _ = with_db(app, |conn| {
        conn.execute(
            "UPDATE conversion_tasks SET status = ?2, success = ?3, failed = ?4, message = ?5, error = ?6, ended_at = ?7, updated_at = ?8 WHERE id = ?1",
            rusqlite::params![task_id, status, success, failed, message, first_error, now_string(), now_string()],
        )
    });
}

fn document_filename(filename: &str) -> String {
    let trimmed = filename.trim();
    if trimmed.is_empty() {
        "untitled.md".to_string()
    } else {
        trimmed.to_string()
    }
}

fn validate_local_manifest(manifest: &LocalBatchManifest) -> Result<(), String> {
    if manifest.target_config_id == 0 {
        return Err("请先选择目标图床配置".to_string());
    }
    if manifest.documents.is_empty() {
        return Err("请至少上传一个 Markdown 文档".to_string());
    }
    if manifest.documents.len() > 20 {
        return Err("单次最多处理 20 个 Markdown 文档".to_string());
    }
    Ok(())
}

fn fail_task(app: &AppHandle, task_id: i64, message: &str, error: &str) {
    let _ = with_db(app, |conn| {
        conn.execute(
            "UPDATE conversion_tasks SET status = 'failed', message = ?2, error = ?3, ended_at = ?4, updated_at = ?5 WHERE id = ?1",
            rusqlite::params![task_id, message, error, now_string(), now_string()],
        )
    });
}

#[derive(Debug, Deserialize)]
struct StoredFile {
    #[serde(default)]
    filename: String,
    #[serde(default)]
    path: String,
}

#[derive(Debug, Deserialize)]
struct LocalTaskPayload {
    #[serde(default)]
    manifest: LocalBatchManifest,
    #[serde(default)]
    files: std::collections::HashMap<String, Vec<StoredFile>>,
}

struct LoadedTask {
    status: String,
    task_type: String,
    payload: String,
}

fn load_task(conn: &rusqlite::Connection, task_id: i64) -> Option<LoadedTask> {
    conn.query_row(
        "SELECT status, task_type, payload FROM conversion_tasks WHERE id = ?1",
        [task_id],
        |row| {
            Ok(LoadedTask {
                status: row.get(0)?,
                task_type: row.get(1)?,
                payload: row.get(2)?,
            })
        },
    )
    .ok()
}

pub fn local_task_dir(app: &AppHandle, task_id: i64) -> std::path::PathBuf {
    let base = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("picsw-rust"));
    base.join("local-tasks").join(task_id.to_string())
}

pub fn safe_local_task_name(value: &str) -> String {
    let normalized = value.trim().replace('\\', "/");
    let base = normalized.rsplit('/').next().unwrap_or("");
    let replaced: String = base
        .chars()
        .map(|c| match c {
            '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            other => other,
        })
        .collect();
    let trimmed = replaced.trim_matches(|c| c == '.' || c == '-');
    if trimmed.is_empty() {
        "file".to_string()
    } else {
        trimmed.to_string()
    }
}
