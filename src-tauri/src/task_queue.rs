use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::sync::mpsc;

use crate::commands::download::write_atomically;
use crate::engine::{self, LocalBatchManifest, MarkdownRequest};
use crate::models::now_string;
use crate::uploader;
use crate::AppState;

/// 单个任务内文档级转换并发数
const CONVERT_DOCUMENT_CONCURRENCY: usize = 3;

/// 应用退出打断任务时写入任务的消息
const INTERRUPTED_TASK_MESSAGE: &str = "任务已中断：应用退出";

/// 应用退出打断任务时写入任务与文档的错误
const INTERRUPTED_TASK_ERROR: &str = "应用退出导致任务中断";

/// 仓库型图床的上传按仓库分支串行，实际并发为 1
fn is_serial_repository(picbed_type: &str) -> bool {
    matches!(picbed_type, "github" | "gitee")
}

pub fn start_workers(app: AppHandle, mut receiver: mpsc::Receiver<i64>) {
    tauri::async_runtime::spawn(async move {
        interrupt_running_tasks(&app);
        cleanup_stale_local_task_dirs(&app);
        requeue_queued_tasks(&app).await;
        while let Some(task_id) = receiver.recv().await {
            run_convert_task(&app, task_id).await;
        }
    });
}

/// 把仍在执行的任务标记为中断，避免重启后进度弹窗一直轮询运行态
pub fn interrupt_running_tasks(app: &AppHandle) {
    let tasks = with_db(app, |conn| {
        let mut stmt = match conn.prepare("SELECT id FROM conversion_tasks WHERE status = 'running'") {
            Ok(stmt) => stmt,
            Err(_) => return Vec::new(),
        };
        stmt.query_map([], |row| row.get::<_, i64>(0))
            .map(|rows| rows.filter_map(|row| row.ok()).collect())
            .unwrap_or_default()
    })
    .unwrap_or_default();
    for task_id in tasks {
        let cancelled = app.state::<AppState>().is_task_cancelled(Some(task_id));
        let _ = with_db(app, |conn| {
            if cancelled {
                mark_task_cancelled(conn, task_id)
            } else {
                mark_task_interrupted(conn, task_id)
            }
        });
        if cancelled {
            app.state::<AppState>().clear_cancel(task_id);
        }
    }
}

fn mark_task_interrupted(conn: &rusqlite::Connection, task_id: i64) -> rusqlite::Result<()> {
    let now = now_string();
    conn.execute(
        "UPDATE conversion_task_documents SET status = 'interrupted', error = ?2, updated_at = ?3 WHERE task_id = ?1 AND status IN ('queued', 'running')",
        rusqlite::params![task_id, INTERRUPTED_TASK_ERROR, now],
    )?;
    let success = count_documents(conn, task_id, "success");
    let partial = count_documents(conn, task_id, "partial");
    let failed = count_documents(conn, task_id, "failed");
    conn.execute(
        "UPDATE conversion_tasks SET status = 'interrupted', success = ?2, partial = ?3, failed = ?4, message = ?5, error = ?6, ended_at = ?7, updated_at = ?7 WHERE id = ?1 AND status = 'running'",
        rusqlite::params![task_id, success, partial, failed, INTERRUPTED_TASK_MESSAGE, INTERRUPTED_TASK_ERROR, now],
    )?;
    Ok(())
}

fn count_documents(conn: &rusqlite::Connection, task_id: i64, status: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM conversion_task_documents WHERE task_id = ?1 AND status = ?2",
        rusqlite::params![task_id, status],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

fn cleanup_stale_local_task_dirs(app: &AppHandle) {
    let base = match app.path().app_data_dir() {
        Ok(dir) => dir.join("local-tasks"),
        Err(_) => return,
    };
    let entries = match std::fs::read_dir(&base) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    let active = with_db(app, |conn| {
        let mut stmt = match conn.prepare("SELECT id FROM conversion_tasks WHERE status IN ('queued', 'running', 'interrupted')") {
            Ok(stmt) => stmt,
            Err(_) => return Vec::new(),
        };
        stmt.query_map([], |row| row.get::<_, i64>(0))
            .map(|rows| rows.filter_map(|row| row.ok()).collect())
            .unwrap_or_default()
    })
    .unwrap_or_default();
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Ok(task_id) = file_name.to_string_lossy().parse::<i64>() else { continue };
        if active.contains(&task_id) {
            continue;
        }
        let _ = std::fs::remove_dir_all(entry.path());
    }
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

fn document_ids_for_task(app: &AppHandle, task_id: i64) -> Vec<i64> {
    with_db(app, |conn| {
        let mut stmt = match conn.prepare("SELECT id FROM conversion_task_documents WHERE task_id = ?1 ORDER BY id ASC") {
            Ok(stmt) => stmt,
            Err(_) => return Vec::new(),
        };
        stmt.query_map([task_id], |row| row.get::<_, i64>(0))
            .map(|rows| rows.filter_map(|row| row.ok()).collect())
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

struct TaskDocumentState {
    completed: HashSet<i64>,
    success: i64,
    partial: i64,
    failed: i64,
}

fn prepare_task_documents(app: &AppHandle, task_id: i64) -> TaskDocumentState {
    with_db(app, |conn| {
        let rows = {
            let mut stmt = match conn.prepare("SELECT id, status FROM conversion_task_documents WHERE task_id = ?1 ORDER BY id ASC") {
                Ok(stmt) => stmt,
                Err(_) => return TaskDocumentState { completed: HashSet::new(), success: 0, partial: 0, failed: 0 },
            };
            stmt.query_map([task_id], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))
                .map(|rows| rows.filter_map(|row| row.ok()).collect::<Vec<_>>())
                .unwrap_or_default()
        };
        let mut state = TaskDocumentState { completed: HashSet::new(), success: 0, partial: 0, failed: 0 };
        for (document_id, status) in rows {
            match status.as_str() {
                "success" => {
                    state.success += 1;
                    state.completed.insert(document_id);
                }
                "partial" => {
                    state.partial += 1;
                    state.completed.insert(document_id);
                }
                "failed" => {
                    state.failed += 1;
                    state.completed.insert(document_id);
                }
                _ => {}
            }
        }
        let _ = conn.execute(
            "UPDATE conversion_task_documents SET status = 'queued', processed_images = 0, error = '', updated_at = ?2 WHERE task_id = ?1 AND status IN ('queued', 'running', 'interrupted')",
            rusqlite::params![task_id, now_string()],
        );
        state
    })
    .unwrap_or(TaskDocumentState { completed: HashSet::new(), success: 0, partial: 0, failed: 0 })
}

fn document_name_key(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        "untitled.md".to_string()
    } else {
        trimmed.to_lowercase()
    }
}

/// 文档已完成并写入转换记录但状态未落库时，按记录补算终态
fn finalize_documents_with_records(conn: &rusqlite::Connection, task_id: i64, now: &str) -> rusqlite::Result<()> {
    let documents: Vec<(i64, String, i64)> = {
        let mut stmt = conn.prepare(
            "SELECT id, filename, total_images FROM conversion_task_documents WHERE task_id = ?1 AND status IN ('queued', 'running', 'interrupted')",
        )?;
        let rows = stmt.query_map([task_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?))
        })?;
        let items = rows.filter_map(|row| row.ok()).collect();
        items
    };
    for (document_id, filename, total_images) in documents {
        let key = document_name_key(&filename);
        let record = conn
            .query_row(
                "SELECT id, status, error_message FROM conversion_records WHERE task_id = ?1 AND lower(trim(original_filename)) = ?2 AND status IN ('success', 'partial', 'failed') ORDER BY id DESC LIMIT 1",
                rusqlite::params![task_id, key],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)),
            )
            .ok();
        let Some((record_id, status, error_message)) = record else { continue };
        let processed = if status == "success" {
            total_images
        } else {
            conn.query_row(
                "SELECT COUNT(*) FROM conversion_record_details WHERE record_id = ?1 AND status IN ('success', 'skipped')",
                [record_id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            .min(total_images)
        };
        conn.execute(
            "UPDATE conversion_task_documents SET status = ?2, processed_images = ?3, error = ?4, updated_at = ?5 WHERE id = ?1",
            rusqlite::params![document_id, status, processed, error_message, now],
        )?;
    }
    Ok(())
}

/// 把中断任务恢复为排队状态，仅重试未完成文档
pub fn prepare_task_resume(conn: &rusqlite::Connection, task_id: i64) -> rusqlite::Result<()> {
    let now = now_string();
    let task_type = conn
        .query_row("SELECT task_type FROM conversion_tasks WHERE id = ?1", [task_id], |row| row.get::<_, String>(0))
        .unwrap_or_default();
    if task_type == "download" {
        let document_ids: Vec<i64> = {
            let mut stmt = conn.prepare("SELECT id FROM conversion_task_documents WHERE task_id = ?1 ORDER BY id ASC")?;
            let ids = stmt
                .query_map([task_id], |row| row.get(0))?
                .filter_map(|row| row.ok())
                .collect();
            ids
        };
        let mut rows_by_index: HashMap<i64, Vec<String>> = HashMap::new();
        {
            let mut stmt = conn.prepare("SELECT document_index, status FROM download_task_images WHERE task_id = ?1")?;
            let rows = stmt.query_map([task_id], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))?;
            for (index, status) in rows.flatten() {
                rows_by_index.entry(index).or_default().push(status);
            }
        }
        for (index, document_id) in document_ids.iter().enumerate() {
            let statuses = rows_by_index.get(&(index as i64)).cloned().unwrap_or_default();
            let has_incomplete = statuses
                .iter()
                .any(|status| matches!(status.as_str(), "pending" | "running" | "cancelled"));
            if has_incomplete {
                conn.execute(
                    "UPDATE conversion_task_documents SET status = 'queued', processed_images = 0, error = '', updated_at = ?2 WHERE id = ?1",
                    rusqlite::params![document_id, now],
                )?;
                conn.execute(
                    "UPDATE download_task_images SET status = 'pending', error = '', updated_at = ?3 WHERE task_id = ?1 AND document_index = ?2 AND status NOT IN ('success', 'skipped')",
                    rusqlite::params![task_id, index as i64, now],
                )?;
                continue;
            }
            let current_status: String = conn
                .query_row("SELECT status FROM conversion_task_documents WHERE id = ?1", [document_id], |row| row.get(0))
                .unwrap_or_default();
            if matches!(current_status.as_str(), "success" | "partial" | "failed") {
                continue;
            }
            let total = statuses.len() as i64;
            let processed = statuses
                .iter()
                .filter(|status| matches!(status.as_str(), "success" | "skipped"))
                .count() as i64;
            let failed = statuses.iter().filter(|status| status.as_str() == "failed").count() as i64;
            let status = if failed == 0 { "success" } else if failed == total { "failed" } else { "partial" };
            let error = if failed > 0 { format!("{} 张图片下载失败", failed) } else { String::new() };
            conn.execute(
                "UPDATE conversion_task_documents SET status = ?2, processed_images = ?3, error = ?4, updated_at = ?5 WHERE id = ?1",
                rusqlite::params![document_id, status, processed, error, now],
            )?;
        }
    } else {
        finalize_documents_with_records(conn, task_id, &now)?;
        conn.execute(
            "UPDATE conversion_task_documents SET status = 'queued', processed_images = 0, error = '', updated_at = ?2 WHERE task_id = ?1 AND status IN ('queued', 'running', 'interrupted')",
            rusqlite::params![task_id, now],
        )?;
    }
    let success = count_documents(conn, task_id, "success");
    let partial = count_documents(conn, task_id, "partial");
    let failed = count_documents(conn, task_id, "failed");
    conn.execute(
        "UPDATE conversion_tasks SET status = 'queued', success = ?2, partial = ?3, failed = ?4, message = '任务已恢复，等待继续执行', error = '', ended_at = NULL, updated_at = ?5 WHERE id = ?1",
        rusqlite::params![task_id, success, partial, failed, now],
    )?;
    Ok(())
}

/// 校验中断任务是否具备继续条件；本地上传需要临时文件仍存在
pub fn check_task_resumable(conn: &rusqlite::Connection, task_id: i64) -> Result<(), String> {
    let (status, task_type, payload): (String, String, String) = conn
        .query_row(
            "SELECT status, task_type, payload FROM conversion_tasks WHERE id = ?1",
            [task_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| "转换任务不存在".to_string())?;
    if status != "interrupted" {
        return Err("任务当前不可继续".to_string());
    }
    if task_type == engine::LOCAL_CONVERT_TASK_TYPE {
        let parsed: LocalTaskPayload =
            serde_json::from_str(&payload).map_err(|_| "本地上传任务载荷解析失败".to_string())?;
        if parsed.manifest.documents.is_empty() {
            return Err("本地上传任务载荷不正确".to_string());
        }
        let files: Vec<&StoredFile> = parsed.files.values().flatten().collect();
        if files.is_empty() || !files.iter().all(|file| std::path::Path::new(&file.path).is_file()) {
            return Err("本地上传的临时文件已清理，无法继续".to_string());
        }
    } else if task_type == "download" {
        let parsed: engine::DownloadTaskPayload =
            serde_json::from_str(&payload).map_err(|_| "下载任务载荷解析失败".to_string())?;
        if parsed.documents.is_empty() {
            return Err("下载任务载荷不正确".to_string());
        }
        if !std::path::Path::new(parsed.target_dir.trim()).is_dir() {
            return Err("下载目录不存在，无法继续".to_string());
        }
    } else if task_type == "convert" {
        let files: Vec<MarkdownRequest> =
            serde_json::from_str(&payload).map_err(|_| "转换任务载荷解析失败".to_string())?;
        if files.is_empty() {
            return Err("转换任务载荷解析失败".to_string());
        }
    } else {
        return Err("不支持的转换任务类型".to_string());
    }
    Ok(())
}

/// 读取任务当前状态，供取消流程判断
pub fn task_status(conn: &rusqlite::Connection, task_id: i64) -> Option<String> {
    conn.query_row("SELECT status FROM conversion_tasks WHERE id = ?1", [task_id], |row| row.get(0))
        .ok()
}

fn mark_task_cancelled(conn: &rusqlite::Connection, task_id: i64) -> rusqlite::Result<()> {
    let now = now_string();
    let task_type = conn
        .query_row("SELECT task_type FROM conversion_tasks WHERE id = ?1", [task_id], |row| row.get::<_, String>(0))
        .unwrap_or_default();
    if task_type != "download" {
        finalize_documents_with_records(conn, task_id, &now)?;
    }
    conn.execute(
        "UPDATE conversion_task_documents SET status = 'cancelled', error = '', updated_at = ?2 WHERE task_id = ?1 AND status IN ('queued', 'running', 'interrupted')",
        rusqlite::params![task_id, now],
    )?;
    conn.execute(
        "UPDATE download_task_images SET status = 'cancelled', updated_at = ?2 WHERE task_id = ?1 AND status IN ('pending', 'running')",
        rusqlite::params![task_id, now],
    )?;
    let success = count_documents(conn, task_id, "success");
    let partial = count_documents(conn, task_id, "partial");
    let failed = count_documents(conn, task_id, "failed");
    let message = format!("任务已取消，成功 {} 个，部分 {} 个，失败 {} 个", success, partial, failed);
    conn.execute(
        "UPDATE conversion_tasks SET status = 'cancelled', success = ?2, partial = ?3, failed = ?4, message = ?5, error = '', ended_at = ?6, updated_at = ?6 WHERE id = ?1",
        rusqlite::params![task_id, success, partial, failed, message, now],
    )?;
    Ok(())
}

/// 取消排队中或已中断的任务；运行中任务由 worker 观察取消标记后收尾
pub fn cancel_pending_task(app: &AppHandle, conn: &rusqlite::Connection, task_id: i64) -> Result<bool, String> {
    let affected = conn
        .execute(
            "UPDATE conversion_tasks SET status = 'cancelled', updated_at = ?2 WHERE id = ?1 AND status IN ('queued', 'interrupted')",
            rusqlite::params![task_id, now_string()],
        )
        .map_err(|_| "取消任务失败".to_string())?;
    if affected == 0 {
        return Ok(false);
    }
    mark_task_cancelled(conn, task_id).map_err(|_| "取消任务失败".to_string())?;
    let task_type = conn
        .query_row("SELECT task_type FROM conversion_tasks WHERE id = ?1", [task_id], |row| row.get::<_, String>(0))
        .ok();
    if task_type.as_deref() == Some(engine::LOCAL_CONVERT_TASK_TYPE) {
        let _ = std::fs::remove_dir_all(local_task_dir(app, task_id));
    }
    Ok(true)
}

fn mark_document_running(app: &AppHandle, document_id: Option<i64>) {
    let Some(document_id) = document_id else { return };
    let _ = with_db(app, |conn| {
        conn.execute(
            "UPDATE conversion_task_documents SET status = 'running', updated_at = ?2 WHERE id = ?1",
            rusqlite::params![document_id, now_string()],
        )
    });
}

fn finish_task_document(app: &AppHandle, document_id: Option<i64>, status: &str, error: Option<&str>) {
    let Some(document_id) = document_id else { return };
    let message = error.unwrap_or("");
    let _ = with_db(app, |conn| {
        if status == "success" {
            conn.execute(
                "UPDATE conversion_task_documents SET status = ?2, processed_images = total_images, error = ?3, updated_at = ?4 WHERE id = ?1",
                rusqlite::params![document_id, status, message, now_string()],
            )
        } else {
            conn.execute(
                "UPDATE conversion_task_documents SET status = ?2, error = ?3, updated_at = ?4 WHERE id = ?1",
                rusqlite::params![document_id, status, message, now_string()],
            )
        }
    });
}

async fn run_convert_task(app: &AppHandle, task_id: i64) {
    let Some(task) = with_db(app, |conn| load_task(conn, task_id)) else { return };
    let Some(task) = task else { return };
    if task.status != "queued" {
        return;
    }
    let claim_message = match task.task_type.as_str() {
        "download" => "下载任务执行中",
        engine::LOCAL_CONVERT_TASK_TYPE => "本地上传任务执行中",
        _ => "转换任务执行中",
    };
    let claimed = with_db(app, |conn| {
        conn.execute(
            "UPDATE conversion_tasks SET status = 'running', message = ?2, started_at = ?3, updated_at = ?4 WHERE id = ?1 AND status = 'queued'",
            rusqlite::params![task_id, claim_message, now_string(), now_string()],
        )
        .map(|rows| rows > 0)
        .unwrap_or(false)
    });
    if !claimed.unwrap_or(false) {
        return;
    }
    let prepared = prepare_task_documents(app, task_id);
    let completed_documents = prepared.completed;
    let mut success = prepared.success;
    let mut partial = prepared.partial;
    let mut failed = prepared.failed;
    let mut done = (success + partial + failed) as usize;

    if task.task_type == "download" {
        run_download_task(app, task_id, completed_documents, success, partial, failed).await;
        return;
    }
    if task.task_type == engine::LOCAL_CONVERT_TASK_TYPE {
        run_local_convert_task(app, task_id, completed_documents, success, partial, failed).await;
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
    let target_picbed_type = files
        .first()
        .and_then(|file| {
            with_db(app, |conn| {
                engine::find_config_by_id(conn, file.target_config_id).map(|config| config.picbed_type)
            })
        })
        .flatten()
        .unwrap_or_default();
    let start_message = if total > 1 && !is_serial_repository(&target_picbed_type) {
        format!("共 {} 个文档，正在按并发度 {} 转换", total, CONVERT_DOCUMENT_CONCURRENCY)
    } else {
        format!("共 {} 个文档，正在转换", total)
    };
    let _ = with_db(app, |conn| {
        conn.execute(
            "UPDATE conversion_tasks SET message = ?2, updated_at = ?3 WHERE id = ?1",
            rusqlite::params![task_id, start_message, now_string()],
        )
    });

    let upload_cache: engine::RemoteUploadCache = Arc::new(std::sync::Mutex::new(HashMap::new()));
    let semaphore = Arc::new(tokio::sync::Semaphore::new(CONVERT_DOCUMENT_CONCURRENCY));
    let document_ids = document_ids_for_task(app, task_id);
    let mut set = tokio::task::JoinSet::new();
    for (index, file) in files.into_iter().enumerate() {
        if document_ids.get(index).is_some_and(|id| completed_documents.contains(id)) {
            continue;
        }
        if app.state::<AppState>().is_task_cancelled(Some(task_id)) {
            break;
        }
        let permit = semaphore.clone().acquire_owned().await.expect("semaphore open");
        if app.state::<AppState>().is_task_cancelled(Some(task_id)) {
            drop(permit);
            break;
        }
        let app = app.clone();
        let upload_cache = upload_cache.clone();
        let document_id = document_ids.get(index).copied();
        set.spawn(async move {
            let state = app.state::<AppState>();
            mark_document_running(&app, document_id);
            let result = engine::convert_one(&state, &file, &upload_cache, Some(task_id), document_id).await;
            drop(permit);
            let filename = document_filename(&file.filename);
            (document_id, result.map_err(|err| (filename, err)))
        });
    }

    let mut cancelled_docs = 0usize;
    let mut first_error = String::new();
    while let Some(joined) = set.join_next().await {
        let (document_id, result) = joined.unwrap_or_else(|error| (None, Err((String::new(), error.to_string()))));
        match result {
            Ok(value) => {
                if value.get("status").and_then(|status| status.as_str()) == Some("partial") {
                    partial += 1;
                    finish_task_document(app, document_id, "partial", None);
                } else {
                    success += 1;
                    finish_task_document(app, document_id, "success", None);
                }
            }
            Err((_, err)) if err == engine::TASK_CANCELLED => {
                cancelled_docs += 1;
            }
            Err((filename, err)) => {
                failed += 1;
                finish_task_document(app, document_id, "failed", Some(&err));
                let err = if filename.is_empty() { err } else { format!("{}：{}", filename, err) };
                if first_error.is_empty() {
                    first_error = err;
                }
            }
        }
        done += 1;
        let message = format!("已完成 {} / {} 个文档", done, total);
        let _ = with_db(app, |conn| {
            conn.execute(
                "UPDATE conversion_tasks SET success = ?2, partial = ?3, failed = ?4, message = ?5, updated_at = ?6 WHERE id = ?1",
                rusqlite::params![task_id, success, partial, failed, message, now_string()],
            )
        });
    }

    if app.state::<AppState>().is_task_cancelled(Some(task_id)) || cancelled_docs > 0 {
        let _ = with_db(app, |conn| mark_task_cancelled(conn, task_id));
        app.state::<AppState>().clear_cancel(task_id);
        return;
    }

    let status = if failed > 0 { "failed" } else { "success" };
    let message = format!("转换完成，成功 {} 个，部分 {} 个，失败 {} 个", success, partial, failed);
    let _ = with_db(app, |conn| {
        conn.execute(
            "UPDATE conversion_tasks SET status = ?2, success = ?3, partial = ?4, failed = ?5, message = ?6, error = ?7, ended_at = ?8, updated_at = ?9 WHERE id = ?1",
            rusqlite::params![task_id, status, success, partial, failed, message, first_error, now_string(), now_string()],
        )
    });
    app.state::<AppState>().clear_cancel(task_id);
}

async fn run_local_convert_task(
    app: &AppHandle,
    task_id: i64,
    completed_documents: HashSet<i64>,
    mut success: i64,
    mut partial: i64,
    mut failed: i64,
) {
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
    let start_message = if total_documents > 1 && !is_serial_repository(&target.picbed_type) {
        format!("共 {} 个文档，正在按并发度 {} 上传替换", total_documents, CONVERT_DOCUMENT_CONCURRENCY)
    } else {
        format!("共 {} 个文档，正在上传替换", total_documents)
    };
    let _ = with_db(app, |conn| {
        conn.execute(
            "UPDATE conversion_tasks SET message = ?2, updated_at = ?3 WHERE id = ?1",
            rusqlite::params![task_id, start_message, now_string()],
        )
    });

    let target = Arc::new(target);
    let target_config = Arc::new(target_config);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(CONVERT_DOCUMENT_CONCURRENCY));
    let document_ids = document_ids_for_task(app, task_id);
    let mut set = tokio::task::JoinSet::new();
    for (index, document) in payload_manifest.documents.into_iter().enumerate() {
        if document_ids.get(index).is_some_and(|id| completed_documents.contains(id)) {
            continue;
        }
        if app.state::<AppState>().is_task_cancelled(Some(task_id)) {
            break;
        }
        let permit = semaphore.clone().acquire_owned().await.expect("semaphore open");
        if app.state::<AppState>().is_task_cancelled(Some(task_id)) {
            drop(permit);
            break;
        }
        let app = app.clone();
        let target = target.clone();
        let target_config = target_config.clone();
        let load_image = load_image.clone();
        let upload_cache = upload_cache.clone();
        let document_id = document_ids.get(index).copied();
        set.spawn(async move {
            let state = app.state::<AppState>();
            mark_document_running(&app, document_id);
            let result = engine::convert_local_one(
                &state,
                &document,
                &target,
                &target_config,
                load_image,
                &upload_cache,
                Some(task_id),
                document_id,
            )
            .await;
            drop(permit);
            let filename = document_filename(&document.filename);
            (document_id, result.map_err(|err| (filename, err)))
        });
    }

    let mut done = (success + partial + failed) as usize;
    let mut cancelled_docs = 0usize;
    let mut first_error = String::new();
    while let Some(joined) = set.join_next().await {
        let (document_id, result) = joined.unwrap_or_else(|error| (None, Err((String::new(), error.to_string()))));
        match result {
            Ok(value) => {
                if value.get("status").and_then(|status| status.as_str()) == Some("partial") {
                    partial += 1;
                    finish_task_document(app, document_id, "partial", None);
                } else {
                    success += 1;
                    finish_task_document(app, document_id, "success", None);
                }
            }
            Err((_, err)) if err == engine::TASK_CANCELLED => {
                cancelled_docs += 1;
            }
            Err((filename, err)) => {
                failed += 1;
                finish_task_document(app, document_id, "failed", Some(&err));
                let err = if filename.is_empty() { err } else { format!("{}：{}", filename, err) };
                if first_error.is_empty() {
                    first_error = err;
                }
            }
        }
        done += 1;
        let message = format!("已完成 {} / {} 个文档", done, total_documents);
        let _ = with_db(app, |conn| {
            conn.execute(
                "UPDATE conversion_tasks SET success = ?2, partial = ?3, failed = ?4, message = ?5, updated_at = ?6 WHERE id = ?1",
                rusqlite::params![task_id, success, partial, failed, message, now_string()],
            )
        });
    }

    if app.state::<AppState>().is_task_cancelled(Some(task_id)) || cancelled_docs > 0 {
        let _ = with_db(app, |conn| mark_task_cancelled(conn, task_id));
        app.state::<AppState>().clear_cancel(task_id);
        return;
    }

    let status = if failed > 0 { "failed" } else { "success" };
    let message = format!("本地图片上传完成，成功 {} 个，部分 {} 个，失败 {} 个", success, partial, failed);
    let _ = with_db(app, |conn| {
        conn.execute(
            "UPDATE conversion_tasks SET status = ?2, success = ?3, partial = ?4, failed = ?5, message = ?6, error = ?7, ended_at = ?8, updated_at = ?9 WHERE id = ?1",
            rusqlite::params![task_id, status, success, partial, failed, message, first_error, now_string(), now_string()],
        )
    });
    app.state::<AppState>().clear_cancel(task_id);
}

#[derive(Debug)]
struct DownloadImageRow {
    id: i64,
    url: String,
    status: String,
}

async fn run_download_task(
    app: &AppHandle,
    task_id: i64,
    completed_documents: HashSet<i64>,
    mut success: i64,
    mut partial: i64,
    mut failed: i64,
) {
    let payload = with_db(app, |conn| {
        let task = load_task(conn, task_id)?;
        serde_json::from_str::<engine::DownloadTaskPayload>(&task.payload).ok()
    })
    .flatten();
    let Some(payload) = payload else {
        fail_task(app, task_id, "下载任务载荷解析失败", "payload missing");
        return;
    };
    let target = PathBuf::from(payload.target_dir.trim());
    if !target.is_dir() {
        fail_task(app, task_id, "下载目录不存在，请重新选择", "");
        return;
    }
    let document_ids = document_ids_for_task(app, task_id);
    let total_documents = payload.documents.len();
    let mut done = (success + partial + failed) as usize;
    let mut skipped_images = 0i64;
    for (index, _document) in payload.documents.iter().enumerate() {
        let document_id = document_ids.get(index).copied();
        if document_id.is_some_and(|id| completed_documents.contains(&id)) {
            continue;
        }
        if app.state::<AppState>().is_task_cancelled(Some(task_id)) {
            let _ = with_db(app, |conn| mark_task_cancelled(conn, task_id));
            app.state::<AppState>().clear_cancel(task_id);
            return;
        }
        mark_document_running(app, document_id);
        let rows = load_download_image_rows(app, task_id, index);
        let mut ok = 0i64;
        let mut image_failed = 0i64;
        for row in rows {
            if app.state::<AppState>().is_task_cancelled(Some(task_id)) {
                let _ = with_db(app, |conn| mark_task_cancelled(conn, task_id));
                app.state::<AppState>().clear_cancel(task_id);
                return;
            }
            if row.status == "success" {
                ok += 1;
                advance_document_progress(app, document_id);
                continue;
            }
            if row.status == "skipped" {
                ok += 1;
                skipped_images += 1;
                advance_document_progress(app, document_id);
                continue;
            }
            match uploader::download_image(&row.url).await {
                Ok(image) => {
                    let file_name = image.filename.trim();
                    let file_name = if file_name.is_empty() { "image.png" } else { file_name };
                    let path = target.join(file_name);
                    if path.exists() {
                        skipped_images += 1;
                        update_download_image_skipped(app, row.id, file_name, &path.to_string_lossy());
                        advance_document_progress(app, document_id);
                        ok += 1;
                        continue;
                    }
                    match write_atomically(&path, &image.data) {
                        Ok(()) => {
                            ok += 1;
                            update_download_image_success(app, row.id, file_name, &path.to_string_lossy());
                            advance_document_progress(app, document_id);
                        }
                        Err(error) => {
                            image_failed += 1;
                            update_download_image_failed(app, row.id, &format!("写入文件失败：{error}"));
                        }
                    }
                }
                Err(error) => {
                    image_failed += 1;
                    update_download_image_failed(app, row.id, &error);
                }
            }
        }
        let status = if image_failed == 0 { "success" } else if ok == 0 { "failed" } else { "partial" };
        let message = if image_failed > 0 { format!("{} 张图片下载失败", image_failed) } else { String::new() };
        finish_download_document(app, document_id, status, ok, &message);
        match status {
            "partial" => partial += 1,
            "failed" => failed += 1,
            _ => success += 1,
        }
        done += 1;
        let progress_message = format!("已完成 {} / {} 个文档", done, total_documents);
        let _ = with_db(app, |conn| {
            conn.execute(
                "UPDATE conversion_tasks SET success = ?2, partial = ?3, failed = ?4, message = ?5, updated_at = ?6 WHERE id = ?1",
                rusqlite::params![task_id, success, partial, failed, progress_message, now_string()],
            )
        });
    }
    let status = if failed > 0 { "failed" } else { "success" };
    let message = if skipped_images > 0 {
        format!(
            "下载完成，成功 {} 个，部分 {} 个，失败 {} 个，跳过 {} 张已存在图片",
            success, partial, failed, skipped_images
        )
    } else {
        format!("下载完成，成功 {} 个，部分 {} 个，失败 {} 个", success, partial, failed)
    };
    let _ = with_db(app, |conn| {
        conn.execute(
            "UPDATE conversion_tasks SET status = ?2, success = ?3, partial = ?4, failed = ?5, message = ?6, error = '', ended_at = ?7, updated_at = ?8 WHERE id = ?1",
            rusqlite::params![task_id, status, success, partial, failed, message, now_string(), now_string()],
        )
    });
    app.state::<AppState>().clear_cancel(task_id);
}

fn load_download_image_rows(app: &AppHandle, task_id: i64, document_index: usize) -> Vec<DownloadImageRow> {
    with_db(app, |conn| {
        let mut stmt = match conn.prepare(
            "SELECT id, url, status FROM download_task_images WHERE task_id = ?1 AND document_index = ?2 ORDER BY id ASC",
        ) {
            Ok(stmt) => stmt,
            Err(_) => return Vec::new(),
        };
        stmt.query_map(rusqlite::params![task_id, document_index as i64], |row| {
            Ok(DownloadImageRow {
                id: row.get(0)?,
                url: row.get(1)?,
                status: row.get(2)?,
            })
        })
        .map(|rows| rows.filter_map(|row| row.ok()).collect())
        .unwrap_or_default()
    })
    .unwrap_or_default()
}

fn advance_document_progress(app: &AppHandle, document_id: Option<i64>) {
    let Some(document_id) = document_id else { return };
    let _ = with_db(app, |conn| {
        conn.execute(
            "UPDATE conversion_task_documents SET processed_images = MIN(processed_images + 1, total_images), updated_at = ?2 WHERE id = ?1",
            rusqlite::params![document_id, now_string()],
        )
    });
}

fn update_download_image_success(app: &AppHandle, image_id: i64, file_name: &str, saved_path: &str) {
    let _ = with_db(app, |conn| {
        conn.execute(
            "UPDATE download_task_images SET status = 'success', file_name = ?2, saved_path = ?3, error = '', updated_at = ?4 WHERE id = ?1",
            rusqlite::params![image_id, file_name, saved_path, now_string()],
        )
    });
}

fn update_download_image_skipped(app: &AppHandle, image_id: i64, file_name: &str, saved_path: &str) {
    let _ = with_db(app, |conn| {
        conn.execute(
            "UPDATE download_task_images SET status = 'skipped', file_name = ?2, saved_path = ?3, error = '', updated_at = ?4 WHERE id = ?1",
            rusqlite::params![image_id, file_name, saved_path, now_string()],
        )
    });
}

fn update_download_image_failed(app: &AppHandle, image_id: i64, error: &str) {
    let _ = with_db(app, |conn| {
        conn.execute(
            "UPDATE download_task_images SET status = 'failed', error = ?2, updated_at = ?3 WHERE id = ?1",
            rusqlite::params![image_id, error, now_string()],
        )
    });
}

fn finish_download_document(app: &AppHandle, document_id: Option<i64>, status: &str, processed: i64, error: &str) {
    let Some(document_id) = document_id else { return };
    let _ = with_db(app, |conn| {
        conn.execute(
            "UPDATE conversion_task_documents SET status = ?2, processed_images = ?3, error = ?4, updated_at = ?5 WHERE id = ?1",
            rusqlite::params![document_id, status, processed, error, now_string()],
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

#[cfg(test)]
mod tests {
    use super::{is_serial_repository, mark_task_cancelled, mark_task_interrupted, prepare_task_resume};
    use rusqlite::Connection;

    fn test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE conversion_tasks (
                id INTEGER PRIMARY KEY,
                task_type TEXT NOT NULL DEFAULT 'convert',
                status TEXT NOT NULL,
                total INTEGER NOT NULL,
                success INTEGER NOT NULL,
                partial INTEGER NOT NULL DEFAULT 0,
                failed INTEGER NOT NULL,
                message TEXT NOT NULL,
                error TEXT NOT NULL,
                ended_at TEXT,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE conversion_task_documents (
                id INTEGER PRIMARY KEY,
                task_id INTEGER NOT NULL,
                filename TEXT NOT NULL DEFAULT '',
                total_images INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL,
                processed_images INTEGER NOT NULL DEFAULT 0,
                error TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE download_task_images (
                id INTEGER PRIMARY KEY,
                task_id INTEGER NOT NULL,
                document_index INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL,
                error TEXT NOT NULL DEFAULT '',
                updated_at TEXT NOT NULL
            );
            CREATE TABLE conversion_records (
                id INTEGER PRIMARY KEY,
                task_id INTEGER,
                original_filename TEXT NOT NULL,
                status TEXT NOT NULL,
                error_message TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE conversion_record_details (
                id INTEGER PRIMARY KEY,
                record_id INTEGER NOT NULL,
                status TEXT NOT NULL
            );",
        )
        .unwrap();
        conn
    }

    fn insert_task(conn: &Connection, id: i64, status: &str, total: i64, success: i64, failed: i64) {
        conn.execute(
            "INSERT INTO conversion_tasks (id, status, total, success, failed, message, error, ended_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, '', '', NULL, 'start')",
            rusqlite::params![id, status, total, success, failed],
        )
        .unwrap();
    }

    fn insert_document(conn: &Connection, task_id: i64, status: &str, processed_images: i64) {
        conn.execute(
            "INSERT INTO conversion_task_documents (task_id, status, processed_images, error, updated_at) VALUES (?1, ?2, ?3, '', 'start')",
            rusqlite::params![task_id, status, processed_images],
        )
        .unwrap();
    }

    #[test]
    fn serializes_repository_hosts_only() {
        assert!(is_serial_repository("github"));
        assert!(is_serial_repository("gitee"));
        assert!(!is_serial_repository("easyimage"));
        assert!(!is_serial_repository("other"));
    }

    #[test]
    fn interrupts_running_task_and_unfinished_documents() {
        let conn = test_connection();
        insert_task(&conn, 1, "running", 4, 1, 0);
        insert_document(&conn, 1, "success", 15);
        insert_document(&conn, 1, "partial", 7);
        insert_document(&conn, 1, "running", 3);
        insert_document(&conn, 1, "queued", 0);

        mark_task_interrupted(&conn, 1).unwrap();

        let (status, success, partial, failed, message, error, ended_at): (String, i64, i64, i64, String, String, Option<String>) = conn
            .query_row(
                "SELECT status, success, partial, failed, message, error, ended_at FROM conversion_tasks WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)),
            )
            .unwrap();
        assert_eq!(status, "interrupted");
        assert_eq!(success, 1);
        assert_eq!(partial, 1);
        assert_eq!(failed, 0);
        assert_eq!(message, "任务已中断：应用退出");
        assert_eq!(error, "应用退出导致任务中断");
        assert!(ended_at.is_some());

        let statuses: Vec<(String, String, i64)> = conn
            .prepare("SELECT status, error, processed_images FROM conversion_task_documents WHERE task_id = 1 ORDER BY id")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .filter_map(|row| row.ok())
            .collect();
        assert_eq!(statuses[0], ("success".to_string(), String::new(), 15));
        assert_eq!(statuses[1], ("partial".to_string(), String::new(), 7));
        assert_eq!(statuses[2], ("interrupted".to_string(), "应用退出导致任务中断".to_string(), 3));
        assert_eq!(statuses[3], ("interrupted".to_string(), "应用退出导致任务中断".to_string(), 0));
    }

    #[test]
    fn ignores_tasks_not_running() {
        let conn = test_connection();
        insert_task(&conn, 1, "queued", 2, 0, 0);
        insert_task(&conn, 2, "success", 2, 2, 0);

        mark_task_interrupted(&conn, 1).unwrap();
        mark_task_interrupted(&conn, 2).unwrap();

        let statuses: Vec<String> = conn
            .prepare("SELECT status FROM conversion_tasks ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|row| row.ok())
            .collect();
        assert_eq!(statuses, vec!["queued".to_string(), "success".to_string()]);
    }

    #[test]
    fn resumes_only_unfinished_documents() {
        let conn = test_connection();
        insert_task(&conn, 1, "interrupted", 4, 1, 0);
        insert_document(&conn, 1, "success", 15);
        insert_document(&conn, 1, "partial", 7);
        insert_document(&conn, 1, "interrupted", 5);
        insert_document(&conn, 1, "queued", 0);

        prepare_task_resume(&conn, 1).unwrap();

        let (status, success, partial, failed, ended_at): (String, i64, i64, i64, Option<String>) = conn
            .query_row(
                "SELECT status, success, partial, failed, ended_at FROM conversion_tasks WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .unwrap();
        assert_eq!(status, "queued");
        assert_eq!(success, 1);
        assert_eq!(partial, 1);
        assert_eq!(failed, 0);
        assert!(ended_at.is_none());

        let documents: Vec<(String, i64)> = conn
            .prepare("SELECT status, processed_images FROM conversion_task_documents WHERE task_id = 1 ORDER BY id")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .filter_map(|row| row.ok())
            .collect();
        assert_eq!(documents[0], ("success".to_string(), 15));
        assert_eq!(documents[1], ("partial".to_string(), 7));
        assert_eq!(documents[2], ("queued".to_string(), 0));
        assert_eq!(documents[3], ("queued".to_string(), 0));
    }

    #[test]
    fn cancels_unfinished_documents_and_keeps_finished_counts() {
        let conn = test_connection();
        insert_task(&conn, 1, "queued", 4, 1, 0);
        insert_document(&conn, 1, "success", 15);
        insert_document(&conn, 1, "partial", 7);
        insert_document(&conn, 1, "queued", 0);
        insert_document(&conn, 1, "running", 2);

        mark_task_cancelled(&conn, 1).unwrap();

        let (status, success, partial, failed): (String, i64, i64, i64) = conn
            .query_row(
                "SELECT status, success, partial, failed FROM conversion_tasks WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(status, "cancelled");
        assert_eq!(success, 1);
        assert_eq!(partial, 1);
        assert_eq!(failed, 0);

        let statuses: Vec<String> = conn
            .prepare("SELECT status FROM conversion_task_documents WHERE task_id = 1 ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|row| row.ok())
            .collect();
        assert_eq!(
            statuses,
            vec!["success".to_string(), "partial".to_string(), "cancelled".to_string(), "cancelled".to_string()]
        );
    }

    fn insert_download_image(conn: &Connection, task_id: i64, document_index: i64, status: &str) {
        conn.execute(
            "INSERT INTO download_task_images (task_id, document_index, status, error, updated_at) VALUES (?1, ?2, ?3, '', 'start')",
            rusqlite::params![task_id, document_index, status],
        )
        .unwrap();
    }

    #[test]
    fn download_resume_keeps_fully_attempted_documents() {
        let conn = test_connection();
        insert_task(&conn, 1, "interrupted", 3, 0, 0);
        conn.execute("UPDATE conversion_tasks SET task_type = 'download' WHERE id = 1", []).unwrap();
        conn.execute(
            "INSERT INTO conversion_task_documents (id, task_id, status, processed_images, error, updated_at) VALUES (1, 1, 'partial', 8, '2 张图片下载失败', 'start')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO conversion_task_documents (id, task_id, status, processed_images, error, updated_at) VALUES (2, 1, 'running', 0, '', 'start')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO conversion_task_documents (id, task_id, status, processed_images, error, updated_at) VALUES (3, 1, 'running', 5, '', 'start')",
            [],
        )
        .unwrap();
        insert_download_image(&conn, 1, 0, "success");
        insert_download_image(&conn, 1, 0, "failed");
        insert_download_image(&conn, 1, 1, "success");
        insert_download_image(&conn, 1, 2, "success");
        insert_download_image(&conn, 1, 2, "failed");
        insert_download_image(&conn, 1, 2, "pending");

        prepare_task_resume(&conn, 1).unwrap();

        let (status, success, partial, failed): (String, i64, i64, i64) = conn
            .query_row(
                "SELECT status, success, partial, failed FROM conversion_tasks WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(status, "queued");
        assert_eq!(success, 1);
        assert_eq!(partial, 1);
        assert_eq!(failed, 0);

        let documents: Vec<(i64, String, i64)> = conn
            .prepare("SELECT id, status, processed_images FROM conversion_task_documents ORDER BY id")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .filter_map(|row| row.ok())
            .collect();
        assert_eq!(documents[0], (1, "partial".to_string(), 8));
        assert_eq!(documents[1], (2, "success".to_string(), 1));
        assert_eq!(documents[2], (3, "queued".to_string(), 0));

        let rows: Vec<(i64, String)> = conn
            .prepare("SELECT document_index, status FROM download_task_images ORDER BY id")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .filter_map(|row| row.ok())
            .collect();
        assert_eq!(rows[0], (0, "success".to_string()));
        assert_eq!(rows[1], (0, "failed".to_string()));
        assert_eq!(rows[2], (1, "success".to_string()));
        assert_eq!(rows[3], (2, "success".to_string()));
        assert_eq!(rows[4], (2, "pending".to_string()));
        assert_eq!(rows[5], (2, "pending".to_string()));
    }

    #[test]
    fn convert_resume_finalizes_documents_with_records() {
        let conn = test_connection();
        insert_task(&conn, 1, "interrupted", 2, 0, 0);
        conn.execute(
            "INSERT INTO conversion_task_documents (id, task_id, filename, total_images, status, processed_images, error, updated_at) VALUES (1, 1, 'a.md', 5, 'running', 0, '', 'start')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO conversion_task_documents (id, task_id, filename, total_images, status, processed_images, error, updated_at) VALUES (2, 1, 'b.md', 3, 'queued', 0, '', 'start')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO conversion_records (id, task_id, original_filename, status, error_message) VALUES (10, 1, 'A.md', 'partial', '2 张图片处理失败')",
            [],
        )
        .unwrap();
        for status in ["success", "success", "skipped", "failed", "failed"] {
            conn.execute("INSERT INTO conversion_record_details (record_id, status) VALUES (10, ?1)", [status])
                .unwrap();
        }

        prepare_task_resume(&conn, 1).unwrap();

        let documents: Vec<(i64, String, i64)> = conn
            .prepare("SELECT id, status, processed_images FROM conversion_task_documents ORDER BY id")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .filter_map(|row| row.ok())
            .collect();
        assert_eq!(documents[0], (1, "partial".to_string(), 3));
        assert_eq!(documents[1], (2, "queued".to_string(), 0));

        let (status, success, partial, failed): (String, i64, i64, i64) = conn
            .query_row(
                "SELECT status, success, partial, failed FROM conversion_tasks WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(status, "queued");
        assert_eq!(success, 0);
        assert_eq!(partial, 1);
        assert_eq!(failed, 0);
    }

    #[test]
    fn convert_cancel_keeps_documents_with_records() {
        let conn = test_connection();
        insert_task(&conn, 1, "queued", 2, 0, 0);
        conn.execute(
            "INSERT INTO conversion_task_documents (id, task_id, filename, total_images, status, processed_images, error, updated_at) VALUES (1, 1, 'a.md', 2, 'running', 0, '', 'start')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO conversion_task_documents (id, task_id, filename, total_images, status, processed_images, error, updated_at) VALUES (2, 1, 'b.md', 1, 'running', 0, '', 'start')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO conversion_records (id, task_id, original_filename, status, error_message) VALUES (20, 1, 'a.md', 'success', '')",
            [],
        )
        .unwrap();

        mark_task_cancelled(&conn, 1).unwrap();

        let documents: Vec<(i64, String, i64)> = conn
            .prepare("SELECT id, status, processed_images FROM conversion_task_documents ORDER BY id")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .filter_map(|row| row.ok())
            .collect();
        assert_eq!(documents[0], (1, "success".to_string(), 2));
        assert_eq!(documents[1], (2, "cancelled".to_string(), 0));

        let (status, success, failed): (String, i64, i64) = conn
            .query_row(
                "SELECT status, success, failed FROM conversion_tasks WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(status, "cancelled");
        assert_eq!(success, 1);
        assert_eq!(failed, 0);
    }
}
