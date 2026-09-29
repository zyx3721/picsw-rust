use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PicBedConfig {
    pub id: i64,
    pub picbed_type: String,
    pub config_name: String,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConversionRecord {
    pub id: i64,
    pub original_filename: String,
    pub source_picbed: String,
    pub target_picbed: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    pub image_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub converted_content: Option<String>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<ConversionRecordDetail>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConversionRecordDetail {
    pub id: i64,
    pub record_id: i64,
    pub original_url: String,
    pub target_url: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub created_at: String,
}

pub fn now_string() -> String {
    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, false)
}
