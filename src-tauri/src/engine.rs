use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::crypto;
use crate::markdown::{extract_markdown_images, is_http_image_url, normalize_local_image_source, replace_image_urls, MarkdownImage};
use crate::models::{now_string, ConversionRecord, ConversionRecordDetail};
use crate::uploader::{self, ImageFile};
use crate::AppState;

pub const LOCAL_CONVERT_TASK_TYPE: &str = "local_upload";

/// 用户主动取消任务时引擎返回的固定错误
pub const TASK_CANCELLED: &str = "任务已取消";

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MarkdownRequest {
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub target_config_id: i64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct LocalImageMapping {
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub file_key: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct LocalMarkdownDocument {
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub images: Vec<LocalImageMapping>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct LocalBatchManifest {
    #[serde(default)]
    pub target_config_id: i64,
    #[serde(default)]
    pub documents: Vec<LocalMarkdownDocument>,
}

pub struct ConfigRow {
    pub id: i64,
    pub picbed_type: String,
    pub config_name: String,
    pub encrypted_config: String,
    pub created_at: String,
    pub updated_at: String,
}

fn lock_db(state: &AppState) -> std::sync::MutexGuard<'_, rusqlite::Connection> {
    state.db.lock().expect("database lock poisoned")
}

/// 累加单个文档已处理的图片数，供任务进度弹窗实时展示
fn advance_document_progress(state: &AppState, document_id: Option<i64>) {
    let Some(document_id) = document_id else { return };
    let conn = lock_db(state);
    let _ = conn.execute(
        "UPDATE conversion_task_documents SET processed_images = MIN(processed_images + 1, total_images), updated_at = ?2 WHERE id = ?1",
        rusqlite::params![document_id, now_string()],
    );
}

pub fn find_config_by_id(conn: &rusqlite::Connection, id: i64) -> Option<ConfigRow> {
    conn.query_row(
        "SELECT id, picbed_type, config_name, encrypted_config, created_at, updated_at FROM picbed_configs WHERE id = ?1",
        [id],
        |row| {
            Ok(ConfigRow {
                id: row.get(0)?,
                picbed_type: row.get(1)?,
                config_name: row.get(2)?,
                encrypted_config: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        },
    )
    .ok()
}

pub fn decrypt_config_map(encrypted: &str) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let raw = crypto::decrypt_string(encrypted)?;
    serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&raw).map_err(|_| "读取图床配置失败".to_string())
}

fn document_filename(filename: &str) -> String {
    let trimmed = filename.trim();
    if trimmed.is_empty() {
        return "untitled.md".to_string();
    }
    trimmed.to_string()
}

fn summarize_source_pic_beds(images: &[MarkdownImage]) -> String {
    if images.is_empty() {
        return "unknown".to_string();
    }
    let mut labels: Vec<String> = Vec::new();
    for image in images {
        let mut picbed_type = image.picbed.trim().to_string();
        if picbed_type.is_empty() {
            picbed_type = "unknown".to_string();
        }
        if !labels.contains(&picbed_type) {
            labels.push(picbed_type);
        }
    }
    if labels.len() == 1 {
        return labels.remove(0);
    }
    "mixed".to_string()
}

enum PendingImage {
    Keep(String),
    Use { url: String },
    Failed { message: String },
}

pub async fn convert_one(
    state: &AppState,
    req: &MarkdownRequest,
    upload_cache: &RemoteUploadCache,
    task_id: Option<i64>,
    document_id: Option<i64>,
) -> Result<serde_json::Value, String> {
    if state.is_task_cancelled(task_id) {
        return Err(TASK_CANCELLED.to_string());
    }
    if req.content.trim().is_empty() {
        return Err("Markdown 内容不能为空".to_string());
    }
    let target = {
        let conn = lock_db(state);
        find_config_by_id(&conn, req.target_config_id)
    };
    let Some(target) = target else {
        let err = "目标图床配置不存在".to_string();
        write_failed_record(state, &document_filename(&req.filename), "unknown", "unknown", &err, task_id);
        return Err(err);
    };
    let target_config = match decrypt_config_map(&target.encrypted_config) {
        Ok(config) => config,
        Err(err) => {
            write_failed_record(state, &document_filename(&req.filename), "unknown", &target.picbed_type, &err, task_id);
            return Err(err);
        }
    };
    let images = extract_markdown_images(&req.content);
    if images.is_empty() {
        let err = "未识别到图片地址".to_string();
        write_failed_record(state, &document_filename(&req.filename), "unknown", "unknown", &err, task_id);
        return Err(err);
    }
    let source_picbed = summarize_source_pic_beds(&images);

    let unique_urls = collect_unique_urls(&images);
    let mut resolved: HashMap<String, PendingImage> = HashMap::new();
    for image_url in unique_urls {
        if state.is_task_cancelled(task_id) {
            return Err(TASK_CANCELLED.to_string());
        }
        if !is_http_image_url(&image_url) {
            resolved.insert(image_url.clone(), PendingImage::Keep(image_url));
            advance_document_progress(state, document_id);
            continue;
        }
        match upload_cached(upload_cache, target.id, &image_url, &target.picbed_type, &target_config).await {
            Ok(url) => {
                resolved.insert(image_url.clone(), PendingImage::Use { url });
                advance_document_progress(state, document_id);
            }
            Err(err) => {
                resolved.insert(image_url.clone(), PendingImage::Failed { message: err });
                advance_document_progress(state, document_id);
                break;
            }
        }
    }

    let mut details: Vec<ConversionRecordDetail> = Vec::new();
    let mut skipped_unsupported = 0usize;
    let filename = document_filename(&req.filename);
    let (output, changed) = match replace_image_urls(&req.content, |current_url| match resolved.get(current_url) {
        Some(PendingImage::Keep(original)) => {
            skipped_unsupported += 1;
            details.push(ConversionRecordDetail {
                id: 0,
                record_id: 0,
                original_url: current_url.to_string(),
                target_url: String::new(),
                status: "failed".to_string(),
                error: Some("本地路径无法通过批量转换上传".to_string()),
                created_at: String::new(),
            });
            Ok(original.clone())
        }
        Some(PendingImage::Use { url }) => {
            details.push(success_detail(current_url, url));
            Ok(url.clone())
        }
        Some(PendingImage::Failed { message }) => Err(message.clone()),
        None => Err(format!("下载图片 {} 失败：处理中断", current_url)),
    }) {
        Ok(result) => result,
        Err(err) => {
            write_failed_record(state, &filename, &source_picbed, &target.picbed_type, &err, task_id);
            return Err(err);
        }
    };

    let mut status = "success";
    let mut message = String::new();
    if skipped_unsupported > 0 {
        message = format!("存在 {} 个图片无法转换", skipped_unsupported);
    }
    if changed == 0 {
        status = "failed";
        if message.is_empty() {
            message = "没有图片地址被转换".to_string();
        }
    }
    if state.is_task_cancelled(task_id) {
        return Err(TASK_CANCELLED.to_string());
    }
    let record_value = {
        let conn = lock_db(state);
        let record = insert_record(
            &conn,
            RecordInsert {
                original_filename: filename.clone(),
                source_picbed,
                target_picbed: target.picbed_type.clone(),
                status: status.to_string(),
                error_message: message.clone(),
                image_count: changed as i64,
                task_id,
                converted_content: output.clone(),
            },
        )?;
        save_record_details(&conn, record, &details);
        record_json(&conn, record)?
    };
    if status == "failed" {
        return Err(message);
    }
    Ok(serde_json::json!({
        "filename": filename,
        "content": output,
        "changed": changed,
        "status": status,
        "record": record_value,
    }))
}

pub type LocalImageLoader = Arc<dyn Fn(&str) -> Result<ImageFile, String> + Send + Sync>;
pub type LocalUploadCache = Arc<std::sync::Mutex<HashMap<String, String>>>;
// 任务级远程上传缓存：同一源图片在多个文档中只上传一次
pub type RemoteUploadCache = Arc<std::sync::Mutex<HashMap<String, Arc<tokio::sync::OnceCell<String>>>>>;

pub async fn convert_local_one(
    state: &AppState,
    document: &LocalMarkdownDocument,
    target: &ConfigRow,
    target_config: &serde_json::Map<String, serde_json::Value>,
    load_image: LocalImageLoader,
    upload_cache: &LocalUploadCache,
    task_id: Option<i64>,
    document_id: Option<i64>,
) -> Result<serde_json::Value, String> {
    if state.is_task_cancelled(task_id) {
        return Err(TASK_CANCELLED.to_string());
    }
    if document.content.trim().is_empty() {
        return Err("Markdown 内容不能为空".to_string());
    }
    let images = extract_markdown_images(&document.content);
    if images.is_empty() {
        return Err("未识别到图片地址".to_string());
    }
    let mut mapping_by_source: HashMap<String, String> = HashMap::new();
    let mut mapping_by_normalized: HashMap<String, String> = HashMap::new();
    for image in &document.images {
        let source = image.source.trim().to_string();
        let file_key = image.file_key.trim().to_string();
        if source.is_empty() || file_key.is_empty() || is_http_image_url(&source) {
            continue;
        }
        mapping_by_source.insert(source.clone(), file_key.clone());
        mapping_by_normalized.insert(normalize_local_image_source(&source), file_key);
    }

    let unique_urls = collect_unique_urls(&images);
    let mut resolved: HashMap<String, PendingImage> = HashMap::new();
    for image_url in unique_urls {
        if state.is_task_cancelled(task_id) {
            return Err(TASK_CANCELLED.to_string());
        }
        if is_http_image_url(&image_url) {
            resolved.insert(image_url.clone(), PendingImage::Keep(image_url));
            advance_document_progress(state, document_id);
            continue;
        }
        let file_key = mapping_by_source
            .get(&image_url)
            .cloned()
            .or_else(|| mapping_by_normalized.get(&normalize_local_image_source(&image_url)).cloned());
        let Some(file_key) = file_key else {
            let message = format!("本地图片 {} 未匹配到上传文件", image_url);
            resolved.insert(image_url.clone(), PendingImage::Failed { message });
            advance_document_progress(state, document_id);
            break;
        };
        let cached = upload_cache
            .lock()
            .ok()
            .and_then(|cache| cache.get(&file_key).cloned());
        if let Some(cached) = cached {
            resolved.insert(image_url.clone(), PendingImage::Use { url: cached });
            advance_document_progress(state, document_id);
            continue;
        }
        let image_file = match (load_image)(&file_key) {
            Ok(file) => file,
            Err(err) => {
                resolved.insert(image_url.clone(), PendingImage::Failed { message: err });
                advance_document_progress(state, document_id);
                break;
            }
        };
        match uploader::upload(&target.picbed_type, target_config, &mut image_file.clone()).await {
            Ok(result) => {
                if let Ok(mut cache) = upload_cache.lock() {
                    cache.insert(file_key.clone(), result.url.clone());
                }
                resolved.insert(image_url.clone(), PendingImage::Use { url: result.url });
                advance_document_progress(state, document_id);
            }
            Err(err) => {
                let message = format!("上传图片 {} 失败：{}", image_url, err);
                resolved.insert(image_url.clone(), PendingImage::Failed { message });
                advance_document_progress(state, document_id);
                break;
            }
        }
    }

    let mut details: Vec<ConversionRecordDetail> = Vec::new();
    let filename = document_filename(&document.filename);
    let (output, changed) = match replace_image_urls(&document.content, |current_url| match resolved.get(current_url) {
        Some(PendingImage::Keep(original)) => Ok(original.clone()),
        Some(PendingImage::Use { url }) => {
            details.push(success_detail(current_url, url));
            Ok(url.clone())
        }
        Some(PendingImage::Failed { message }) => Err(message.clone()),
        None => Err(format!("本地图片 {} 未匹配到上传文件", current_url)),
    }) {
        Ok(result) => result,
        Err(err) => {
            write_failed_record(state, &filename, "local", &target.picbed_type, &err, task_id);
            return Err(err);
        }
    };

    let mut status = "success";
    let mut message = String::new();
    if changed == 0 {
        status = "failed";
        message = "没有本地图片地址被转换".to_string();
    }
    if state.is_task_cancelled(task_id) {
        return Err(TASK_CANCELLED.to_string());
    }
    let record_value = {
        let conn = lock_db(state);
        let record = insert_record(
            &conn,
            RecordInsert {
                original_filename: filename.clone(),
                source_picbed: "local".to_string(),
                target_picbed: target.picbed_type.clone(),
                status: status.to_string(),
                error_message: message.clone(),
                image_count: changed as i64,
                task_id,
                converted_content: output.clone(),
            },
        )?;
        save_record_details(&conn, record, &details);
        record_json(&conn, record)?
    };
    if status == "failed" {
        return Err(message);
    }
    Ok(serde_json::json!({
        "filename": filename,
        "content": output,
        "changed": changed,
        "status": status,
        "record": record_value,
    }))
}

fn collect_unique_urls(images: &[MarkdownImage]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for image in images {
        let url = image.url.trim().to_string();
        if !seen.contains(&url) {
            seen.push(url);
        }
    }
    seen
}

async fn download_and_upload(
    image_url: &str,
    picbed_type: &str,
    cfg: &serde_json::Map<String, serde_json::Value>,
) -> Result<String, String> {
    let image = uploader::download_image(image_url)
        .await
        .map_err(|err| format!("下载图片 {} 失败：{}", image_url, err))?;
    uploader::upload(picbed_type, cfg, &mut image.clone())
        .await
        .map(|result| result.url)
        .map_err(|err| format!("上传图片 {} 失败：{}", image_url, err))
}

async fn upload_cached(
    cache: &RemoteUploadCache,
    config_id: i64,
    image_url: &str,
    picbed_type: &str,
    cfg: &serde_json::Map<String, serde_json::Value>,
) -> Result<String, String> {
    let key = format!("{}|{}", config_id, image_url);
    let cell = match cache.lock() {
        Ok(mut entries) => entries.entry(key).or_insert_with(|| Arc::new(tokio::sync::OnceCell::new())).clone(),
        Err(_) => Arc::new(tokio::sync::OnceCell::new()),
    };
    let url = cell
        .get_or_try_init(|| async { download_and_upload(image_url, picbed_type, cfg).await })
        .await?;
    Ok(url.clone())
}

fn success_detail(original_url: &str, target_url: &str) -> ConversionRecordDetail {
    ConversionRecordDetail {
        id: 0,
        record_id: 0,
        original_url: original_url.to_string(),
        target_url: target_url.to_string(),
        status: "success".to_string(),
        error: None,
        created_at: String::new(),
    }
}

fn write_failed_record(
    state: &AppState,
    filename: &str,
    source_picbed: &str,
    target_picbed: &str,
    error_message: &str,
    task_id: Option<i64>,
) {
    let conn = lock_db(state);
    let _ = insert_record(
        &conn,
        RecordInsert {
            original_filename: filename.to_string(),
            source_picbed: source_picbed.to_string(),
            target_picbed: target_picbed.to_string(),
            status: "failed".to_string(),
            error_message: error_message.to_string(),
            image_count: 0,
            task_id,
            converted_content: String::new(),
        },
    );
}

pub struct RecordInsert {
    pub original_filename: String,
    pub source_picbed: String,
    pub target_picbed: String,
    pub status: String,
    pub error_message: String,
    pub image_count: i64,
    pub task_id: Option<i64>,
    pub converted_content: String,
}

pub fn insert_record(conn: &rusqlite::Connection, record: RecordInsert) -> Result<i64, String> {
    conn.execute(
        "INSERT INTO conversion_records (original_filename, source_picbed, target_picbed, status, error_message, image_count, task_id, converted_content, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            record.original_filename,
            record.source_picbed,
            record.target_picbed,
            record.status,
            record.error_message,
            record.image_count,
            record.task_id,
            record.converted_content,
            now_string(),
        ],
    )
    .map_err(|_| "保存转换记录失败".to_string())?;
    Ok(conn.last_insert_rowid())
}

pub fn save_record_details(conn: &rusqlite::Connection, record_id: i64, details: &[ConversionRecordDetail]) {
    if details.is_empty() {
        return;
    }
    let now = now_string();
    for detail in details {
        let _ = conn.execute(
            "INSERT INTO conversion_record_details (record_id, original_url, target_url, status, error, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![record_id, detail.original_url, detail.target_url, detail.status, detail.error.clone().unwrap_or_default(), now],
        );
    }
}

pub fn record_json(conn: &rusqlite::Connection, record_id: i64) -> Result<serde_json::Value, String> {
    conn.query_row(
        "SELECT id, original_filename, source_picbed, target_picbed, status, error_message, image_count, task_id, converted_content, created_at FROM conversion_records WHERE id = ?1",
        [record_id],
        |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "original_filename": row.get::<_, String>(1)?,
                "source_picbed": row.get::<_, String>(2)?,
                "target_picbed": row.get::<_, String>(3)?,
                "status": row.get::<_, String>(4)?,
                "error_message": row.get::<_, Option<String>>(5)?,
                "image_count": row.get::<_, i64>(6)?,
                "task_id": row.get::<_, Option<i64>>(7)?,
                "converted_content": row.get::<_, Option<String>>(8)?,
                "created_at": row.get::<_, String>(9)?,
            }))
        },
    )
    .map_err(|e| e.to_string())
}

pub fn load_record_with_details(conn: &rusqlite::Connection, record_id: i64) -> Option<ConversionRecord> {
    let mut record = load_record(conn, record_id)?;
    record.details = Some(load_record_details(conn, record_id));
    Some(record)
}

pub fn load_record(conn: &rusqlite::Connection, record_id: i64) -> Option<ConversionRecord> {
    conn.query_row(
        "SELECT id, original_filename, source_picbed, target_picbed, status, error_message, image_count, task_id, converted_content, created_at FROM conversion_records WHERE id = ?1",
        [record_id],
        |row| {
            Ok(ConversionRecord {
                id: row.get(0)?,
                original_filename: row.get(1)?,
                source_picbed: row.get(2)?,
                target_picbed: row.get(3)?,
                status: row.get(4)?,
                error_message: row.get(5)?,
                image_count: row.get(6)?,
                task_id: row.get(7)?,
                converted_content: row.get(8)?,
                created_at: row.get(9)?,
                details: None,
            })
        },
    )
    .ok()
}

pub fn load_record_details(conn: &rusqlite::Connection, record_id: i64) -> Vec<ConversionRecordDetail> {
    let mut stmt = match conn.prepare(
        "SELECT id, record_id, original_url, target_url, status, error, created_at FROM conversion_record_details WHERE record_id = ?1 ORDER BY id ASC",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    let rows = stmt.query_map([record_id], |row| {
        Ok(ConversionRecordDetail {
            id: row.get(0)?,
            record_id: row.get(1)?,
            original_url: row.get(2)?,
            target_url: row.get(3)?,
            status: row.get(4)?,
            error: row.get(5)?,
            created_at: row.get(6)?,
        })
    });
    match rows {
        Ok(rows) => rows.filter_map(|row| row.ok()).collect(),
        Err(_) => Vec::new(),
    }
}
