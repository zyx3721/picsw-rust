//! GitHub Device Flow（RFC 8628）认证桥
//!
//! `github.com/login/*` 在 WebView 内 fetch 存在 CORS 限制，须经 Rust reqwest 代理；
//! client_id 支持运行时解析（环境变量 / 程序目录 .env / 应用数据目录 .env），并回退构建期注入值
//! （仅需 OAuth App 的 client_id，无需 client secret）。

use std::sync::OnceLock;

use serde::Serialize;
use tauri::Manager;

const GITHUB_DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const GITHUB_ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const DEFAULT_SCOPE: &str = "gist read:user";
const CLIENT_ID_ENV_KEY: &str = "VITE_SYNC_GITHUB_CLIENT_ID";

fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("构建 HTTP 客户端失败")
    })
}

/// 请求失败的完整原因链（reqwest 的 Display 不含底层 source）
fn err_chain(error: &dyn std::error::Error) -> String {
    let mut chain = error.to_string();
    let mut source = error.source();
    while let Some(err) = source {
        chain.push_str(&format!(": {err}"));
        source = err.source();
    }
    chain
}

/// 带重试的请求发送：直连 github.com 的链路存在间歇性超时（尤其国内网络），
/// 网络类失败自动重试，间隔 1s / 2s 递增。最后一次仍失败时，超时类错误附上处理指引。
async fn send_with_retry(
    builder: reqwest::RequestBuilder,
    attempts: u32,
    what: &str,
) -> Result<reqwest::Response, String> {
    let mut last_err = String::new();
    for attempt in 0..attempts {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(1000 * attempt as u64)).await;
        }
        let request = builder
            .try_clone()
            .ok_or_else(|| format!("{what}: 请求体无法克隆以供重试"))?;
        match request.send().await {
            Ok(res) => return Ok(res),
            Err(e) => {
                last_err = err_chain(&e);
            }
        }
    }
    if last_err.contains("timed out") || last_err.contains("timeout") {
        return Err(format!(
            "{what}: 连接 github.com 超时（网络不稳定或被间歇性阻断，已自动重试 {attempts} 次）。\
             可稍后重试；若本机需要代理访问 GitHub，请设置 HTTPS_PROXY 环境变量后重启应用"
        ));
    }
    Err(format!("{what}: {last_err}"))
}

#[derive(Debug, Serialize)]
pub struct DeviceFlowStart {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    /// 过期时间（本地毫秒时间戳）
    pub expires_at: i64,
    /// 建议轮询间隔（秒）
    pub interval: u64,
}

/// 启动 Device Flow：请求 device code，用户到 verification_uri 输入 user_code
#[tauri::command]
pub async fn github_device_flow_start(
    client_id: String,
    scope: Option<String>,
) -> Result<DeviceFlowStart, String> {
    if client_id.trim().is_empty() {
        return Err(format!("缺少 GitHub OAuth App 的 client_id（{CLIENT_ID_ENV_KEY}）"));
    }

    let response = send_with_retry(
        http_client()
            .post(GITHUB_DEVICE_CODE_URL)
            .header("Accept", "application/json")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!(
                "client_id={}&scope={}",
                urlencode(&client_id),
                urlencode(scope.as_deref().unwrap_or(DEFAULT_SCOPE))
            )),
        3,
        "请求 GitHub device code",
    )
    .await?;

    let status = response.status();
    let text = response.text().await.map_err(|e| format!("读取响应失败: {e}"))?;
    if !status.is_success() {
        return Err(format!("GitHub device flow 失败: {status} - {text}"));
    }

    #[derive(serde::Deserialize)]
    struct Raw {
        device_code: String,
        user_code: String,
        verification_uri: String,
        expires_in: Option<u64>,
        interval: Option<u64>,
    }
    let raw: Raw = serde_json::from_str(&text).map_err(|_| {
        format!("GitHub device flow 响应不是合法 JSON: {}", &text[..text.len().min(200)])
    })?;

    Ok(DeviceFlowStart {
        device_code: raw.device_code,
        user_code: raw.user_code,
        verification_uri: raw.verification_uri,
        expires_at: now_millis() + (raw.expires_in.unwrap_or(0) * 1000) as i64,
        interval: raw.interval.unwrap_or(5).max(5),
    })
}

/// 轮询令牌端点一次。GitHub 对 pending/slow_down 返回 HTTP 200 + error 字段，
/// 原样透传响应 JSON，由前端按 RFC 8628 处理。
#[tauri::command]
pub async fn github_device_flow_poll(
    client_id: String,
    device_code: String,
) -> Result<serde_json::Value, String> {
    if client_id.trim().is_empty() || device_code.trim().is_empty() {
        return Err("缺少 client_id 或 device_code".to_string());
    }

    let response = send_with_retry(
        http_client()
            .post(GITHUB_ACCESS_TOKEN_URL)
            .header("Accept", "application/json")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!(
                "client_id={}&device_code={}&grant_type={}",
                urlencode(&client_id),
                urlencode(&device_code),
                urlencode("urn:ietf:params:oauth:grant-type:device_code"),
            )),
        2,
        "请求 GitHub token",
    )
    .await?;

    let status = response.status();
    let text = response.text().await.map_err(|e| format!("读取响应失败: {e}"))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|_| format!("GitHub token 轮询响应不是合法 JSON: {status} - {}", &text[..text.len().min(200)]))?;

    if value.get("access_token").is_some() || value.get("error").is_some() {
        return Ok(value);
    }
    if !status.is_success() {
        return Err(format!("GitHub token 轮询失败: {status} - {text}"));
    }
    Ok(value)
}

/// 运行时解析 GitHub OAuth App client_id，供前端判断设备码授权是否可用：
/// 优先级为进程环境变量 → 程序所在目录 .env → 应用数据目录 .env → 构建期注入值
#[tauri::command]
pub fn sync_github_client_id(app: tauri::AppHandle) -> Option<String> {
    if let Ok(value) = std::env::var(CLIENT_ID_ENV_KEY) {
        let value = value.trim().to_string();
        if !value.is_empty() {
            return Some(value);
        }
    }

    let mut env_paths: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            env_paths.push(dir.join(".env"));
        }
    }
    if let Ok(dir) = app.path().app_config_dir() {
        env_paths.push(dir.join(".env"));
    }
    for path in env_paths {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Some(value) = parse_env_value(&content, CLIENT_ID_ENV_KEY) {
                return Some(value);
            }
        }
    }

    option_env!("VITE_SYNC_GITHUB_CLIENT_ID")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// 解析 .env 文本中目标键的值：跳过空行与 # 注释行，剔除行尾注释与成对引号
fn parse_env_value(content: &str, key: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let stripped = trimmed.strip_prefix("export ").map(str::trim_start).unwrap_or(trimmed);
        let Some((name, value)) = stripped.split_once('=') else {
            continue;
        };
        if name.trim() != key {
            continue;
        }
        let value = value.trim();
        let value = match value.find(" #") {
            Some(index) => value[..index].trim_end(),
            None => value,
        };
        let unquoted = value
            .strip_prefix('"').and_then(|v| v.strip_suffix('"'))
            .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
            .unwrap_or(value);
        if !unquoted.is_empty() {
            return Some(unquoted.to_string());
        }
    }
    None
}

/// application/x-www-form-urlencoded 值编码（字母数字与 -*._ 之外全部转义）
fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'*' | b'.' | b'_' => {
                out.push(*byte as char)
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
