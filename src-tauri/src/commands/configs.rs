use serde::Deserialize;
use serde_json::json;
use serde_json::Map;
use serde_json::Value;
use std::collections::HashMap;
use tauri::State;

use crate::crypto;
use crate::engine::{decrypt_config_map, find_config_by_id};
use crate::models::{now_string, PicBedConfig};
use crate::types_def::{display_field_label, find_picbed_type, picbed_type_defs};
use crate::uploader;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct PicbedConfigRequest {
    #[serde(default)]
    pub picbed_type: String,
    #[serde(default)]
    pub config_name: String,
    #[serde(default)]
    pub config: Option<HashMap<String, String>>,
    #[serde(default)]
    pub is_default: bool,
}

fn lock_db(state: &AppState) -> Result<std::sync::MutexGuard<'_, rusqlite::Connection>, String> {
    state.db.lock().map_err(|_| "数据库连接异常".to_string())
}

#[tauri::command]
pub fn get_picbed_types() -> Value {
    json!({ "types": picbed_type_defs() })
}

#[tauri::command]
pub fn list_configs(state: State<AppState>) -> Result<Value, String> {
    let conn = lock_db(&state)?;
    normalize_default_config(&conn)?;
    let mut stmt = conn
        .prepare("SELECT id, picbed_type, config_name, encrypted_config, is_default, created_at, updated_at FROM picbed_configs ORDER BY is_default DESC, updated_at DESC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, bool>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut items: Vec<Value> = Vec::new();
    for row in rows {
        let (id, picbed_type, config_name, encrypted_config, is_default, created_at, updated_at) = row.map_err(|e| e.to_string())?;
        let config_map = decrypt_config_map(&encrypted_config).ok();
        let editable = editable_config(&picbed_type, config_map);
        items.push(config_response(
            PicBedConfig {
                id,
                picbed_type,
                config_name,
                is_default,
                created_at,
                updated_at,
                config: None,
            },
            Some(editable),
        ));
    }
    Ok(json!({ "configs": items }))
}

#[tauri::command]
pub fn create_config(state: State<AppState>, req: PicbedConfigRequest) -> Result<Value, String> {
    let conn = lock_db(&state)?;
    validate_picbed_config(&req)?;
    let encrypted = encrypt_config(&req)?;
    let config_name = req.config_name.trim().to_string();
    if config_name_exists(&conn, &config_name, 0)? {
        return Err("配置名称不能重复".to_string());
    }
    if req.is_default {
        clear_default(&conn, 0)?;
    }
    let now = now_string();
    let inserted = conn
        .execute(
            "INSERT INTO picbed_configs (picbed_type, config_name, encrypted_config, is_default, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![req.picbed_type, config_name, encrypted, req.is_default as i64, now.clone(), now],
        )
        .map_err(|_| "配置名称不能重复".to_string())?;
    if inserted == 0 {
        return Err("配置名称不能重复".to_string());
    }
    let id = conn.last_insert_rowid();
    Ok(json!({ "config": config_response(PicBedConfig {
        id,
        picbed_type: req.picbed_type,
        config_name,
        is_default: req.is_default,
        created_at: String::new(),
        updated_at: String::new(),
        config: None,
    }, Some(json_masked())) }))
}

#[tauri::command]
pub fn update_config(state: State<AppState>, id: i64, req: PicbedConfigRequest) -> Result<Value, String> {
    let conn = lock_db(&state)?;
    let existing = find_config_by_id(&conn, id).ok_or("图床配置不存在")?;
    validate_picbed_config(&req)?;
    let encrypted = encrypt_config(&req)?;
    let config_name = req.config_name.trim().to_string();
    if config_name_exists(&conn, &config_name, existing.id)? {
        return Err("配置名称不能重复".to_string());
    }
    if req.is_default {
        clear_default(&conn, existing.id)?;
    }
    let updated = conn
        .execute(
            "UPDATE picbed_configs SET picbed_type = ?2, config_name = ?3, encrypted_config = ?4, is_default = ?5, updated_at = ?6 WHERE id = ?1",
            rusqlite::params![existing.id, req.picbed_type, config_name, encrypted, req.is_default as i64, now_string()],
        )
        .map_err(|_| "保存图床配置失败".to_string())?;
    if updated == 0 {
        return Err("保存图床配置失败".to_string());
    }
    Ok(json!({ "config": config_response(PicBedConfig {
        id: existing.id,
        picbed_type: req.picbed_type,
        config_name,
        is_default: req.is_default,
        created_at: existing.created_at,
        updated_at: String::new(),
        config: None,
    }, Some(json_masked())) }))
}

#[tauri::command]
pub fn delete_config(state: State<AppState>, id: i64) -> Result<Value, String> {
    let conn = lock_db(&state)?;
    find_config_by_id(&conn, id).ok_or("图床配置不存在")?;
    conn.execute("DELETE FROM picbed_configs WHERE id = ?1", [id])
        .map_err(|_| "删除图床配置失败".to_string())?;
    Ok(json!({ "message": "图床配置已删除" }))
}

#[tauri::command]
pub fn set_default_config(state: State<AppState>, id: i64) -> Result<Value, String> {
    let conn = lock_db(&state)?;
    let existing = find_config_by_id(&conn, id).ok_or("图床配置不存在")?;
    clear_default(&conn, existing.id)?;
    conn.execute(
        "UPDATE picbed_configs SET is_default = 1, updated_at = ?2 WHERE id = ?1",
        rusqlite::params![existing.id, now_string()],
    )
    .map_err(|_| "设置默认配置失败".to_string())?;
    let config_map = decrypt_config_map(&existing.encrypted_config).ok();
    Ok(json!({ "config": config_response(
        PicBedConfig {
            id: existing.id,
            picbed_type: existing.picbed_type.clone(),
            config_name: existing.config_name,
            is_default: true,
            created_at: existing.created_at,
            updated_at: existing.updated_at,
            config: None,
        },
        Some(editable_config(&existing.picbed_type, config_map)),
    ) }))
}

#[tauri::command]
pub async fn test_config_draft(req: PicbedConfigRequest) -> Result<Value, String> {
    validate_picbed_config(&req)?;
    let config_map = normalize_config(&req);
    uploader::test_config(&req.picbed_type, &config_map)
        .await
        .map_err(|err| format!("配置测试失败：{}", err))?;
    Ok(json!({ "message": "配置测试通过" }))
}

#[tauri::command]
pub async fn test_config_saved(state: State<'_, AppState>, id: i64) -> Result<Value, String> {
    let (picbed_type, config_map) = {
        let conn = lock_db(&state)?;
        let existing = find_config_by_id(&conn, id).ok_or("图床配置不存在")?;
        let config_map = decrypt_config_map(&existing.encrypted_config)?;
        (existing.picbed_type, config_map)
    };
    uploader::test_config(&picbed_type, &config_map)
        .await
        .map_err(|err| format!("配置测试失败：{}", err))?;
    Ok(json!({ "message": "配置测试通过" }))
}

fn json_masked() -> Value {
    let mut map = Map::new();
    map.insert("masked".to_string(), Value::Bool(true));
    Value::Object(map)
}

fn config_response(item: PicBedConfig, config: Option<Value>) -> Value {
    json!({
        "id": item.id,
        "picbed_type": item.picbed_type,
        "config_name": item.config_name,
        "is_default": item.is_default,
        "created_at": item.created_at,
        "updated_at": item.updated_at,
        "config": config.unwrap_or_else(json_masked),
    })
}

fn editable_config(picbed_type: &str, config: Option<Map<String, Value>>) -> Value {
    let mut output = Map::new();
    output.insert("masked".to_string(), Value::Bool(true));
    let Some(mut config) = config else {
        return Value::Object(output);
    };
    let Some(def) = find_picbed_type(picbed_type) else {
        return Value::Object(output);
    };
    if picbed_type == "aliyun" {
        let region_empty = config
            .get("region")
            .and_then(|value| value.as_str())
            .map(|value| value.trim().is_empty())
            .unwrap_or(true);
        if region_empty {
            let endpoint = config.get("endpoint").and_then(|value| value.as_str()).unwrap_or("").trim().to_string();
            config.insert("region".to_string(), Value::String(endpoint));
        }
    }
    for field in def.fields {
        let value = config
            .get(&field.key)
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if !value.is_empty() {
            output.insert(field.key, Value::String(value));
        }
    }
    Value::Object(output)
}

fn validate_picbed_config(req: &PicbedConfigRequest) -> Result<(), String> {
    let def = find_picbed_type(&req.picbed_type).ok_or("不支持的图床类型")?;
    if req.config_name.trim().is_empty() {
        return Err("请填写配置名称".to_string());
    }
    let Some(config) = &req.config else {
        return Err("请填写图床配置".to_string());
    };
    for field in def.fields {
        if field.required && config.get(&field.key).map(|value| value.trim()).unwrap_or("").is_empty() {
            return Err(format!("请填写{}", display_field_label(&field)));
        }
    }
    Ok(())
}

fn normalize_config(req: &PicbedConfigRequest) -> Map<String, Value> {
    let mut output = Map::new();
    if let Some(config) = &req.config {
        for (key, value) in config {
            output.insert(key.clone(), Value::String(value.trim().to_string()));
        }
    }
    if req.picbed_type == "aliyun" {
        let region_empty = output
            .get("region")
            .and_then(|value| value.as_str())
            .map(|value| value.trim().is_empty())
            .unwrap_or(true);
        if region_empty {
            let endpoint = output.get("endpoint").and_then(|value| value.as_str()).unwrap_or("").to_string();
            output.insert("region".to_string(), Value::String(endpoint));
        }
        output.remove("endpoint");
    }
    output
}

fn encrypt_config(req: &PicbedConfigRequest) -> Result<String, String> {
    let normalized = normalize_config(req);
    let raw = serde_json::to_string(&normalized).map_err(|e| e.to_string())?;
    crypto::encrypt_string(&raw).map_err(|_| "配置加密失败".to_string())
}

fn config_name_exists(conn: &rusqlite::Connection, config_name: &str, exclude_id: i64) -> Result<bool, String> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM picbed_configs WHERE config_name = ?1 AND id <> ?2",
            rusqlite::params![config_name, exclude_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(count > 0)
}

fn clear_default(conn: &rusqlite::Connection, exclude_id: i64) -> Result<(), String> {
    conn.execute("UPDATE picbed_configs SET is_default = 0 WHERE is_default = 1 AND id <> ?1", [exclude_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn normalize_default_config(conn: &rusqlite::Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare("SELECT id FROM picbed_configs WHERE is_default = 1 ORDER BY updated_at DESC, id DESC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get::<_, i64>(0))
        .map_err(|e| e.to_string())?;
    let ids: Vec<i64> = rows.filter_map(|row| row.ok()).collect();
    if ids.len() <= 1 {
        return Ok(());
    }
    for id in &ids[1..] {
        conn.execute("UPDATE picbed_configs SET is_default = 0 WHERE id = ?1", [id])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
