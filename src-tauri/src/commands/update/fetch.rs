//! 最新 Release 查询：官方接口优先，异常时经发布页 302 跳转解析版本号

use serde::{Deserialize, Serialize};

use super::{RELEASE_API_URL, RELEASE_LATEST_URL, UPDATE_HTTP_TIMEOUT_SECS, UPDATE_USER_AGENT};

/// 最新 Release 查询结果（tag 含 v 前缀；assets 为 None 表示清单缺失，由前端按命名规律直拼地址）
#[derive(Serialize)]
pub struct UpdateLatest {
    pub tag: String,
    pub assets: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: Option<String>,
    assets: Option<Vec<ApiAsset>>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
}

/// 查询 GitHub 最新 Release 的版本号与资产名清单，官方接口异常时回退发布页跳转解析
#[tauri::command]
pub async fn get_update_latest() -> Result<UpdateLatest, String> {
    let api_error = match fetch_release_api().await {
        Ok(latest) => return Ok(latest),
        Err(error) => error,
    };
    match fetch_release_redirect().await {
        Ok(latest) => Ok(latest),
        Err(error) => Err(format!("{api_error}；{error}")),
    }
}

/// 经官方接口获取最新 Release，返回版本号与资产名清单
async fn fetch_release_api() -> Result<UpdateLatest, String> {
    let client = update_http_client(reqwest::redirect::Policy::default())?;
    let response = client
        .get(RELEASE_API_URL)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("连接 GitHub 接口失败: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "GitHub 接口响应异常（{}）",
            response.status().as_u16()
        ));
    }
    let text = response
        .text()
        .await
        .map_err(|e| format!("读取版本信息失败: {e}"))?;
    let release: ApiRelease = serde_json::from_str(&text).map_err(|_| "版本信息解析失败".to_string())?;
    let tag = release.tag_name.unwrap_or_default();
    if tag.is_empty() {
        return Err("版本信息缺少版本号".to_string());
    }
    let assets = release
        .assets
        .map(|list| list.into_iter().map(|asset| asset.name).collect());
    Ok(UpdateLatest { tag, assets })
}

/// 经发布页 302 跳转地址解析最新版本号（无资产清单）
async fn fetch_release_redirect() -> Result<UpdateLatest, String> {
    let client = update_http_client(reqwest::redirect::Policy::none())?;
    let response = client
        .get(RELEASE_LATEST_URL)
        .send()
        .await
        .map_err(|e| format!("连接发布页失败: {e}"))?;
    let location = response
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| "发布页未返回跳转地址".to_string())?;
    let tag = location.rsplit('/').next().unwrap_or_default();
    if tag.is_empty() || !(tag.starts_with('v') || tag.starts_with(|c: char| c.is_ascii_digit())) {
        return Err("未从发布页解析到版本号".to_string());
    }
    Ok(UpdateLatest {
        tag: tag.to_string(),
        assets: None,
    })
}

/// 创建更新检查专用 HTTP 客户端（统一 UA 与超时）
fn update_http_client(redirect: reqwest::redirect::Policy) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(UPDATE_USER_AGENT)
        .timeout(std::time::Duration::from_secs(UPDATE_HTTP_TIMEOUT_SECS))
        .redirect(redirect)
        .build()
        .map_err(|e| format!("创建网络客户端失败: {e}"))
}
