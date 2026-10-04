//! 最新 Release 查询：官方接口优先，异常时经发布页 302 跳转解析版本号

use serde::{Deserialize, Serialize};

use super::{RELEASE_API_URL, RELEASE_LATEST_URL, UPDATE_HTTP_TIMEOUT_SECS, UPDATE_USER_AGENT};

/// 最新 Release 查询结果（tag 含 v 前缀；assets 为 None 表示清单缺失，由前端按命名规律直拼地址；
/// notes/html_url/published_at 来自官方接口，发布页 302 回退通道拿不到时为空串）
#[derive(Serialize)]
pub struct UpdateLatest {
    pub tag: String,
    pub assets: Option<Vec<String>>,
    pub notes: String,
    pub html_url: String,
    pub published_at: String,
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: Option<String>,
    html_url: Option<String>,
    body: Option<String>,
    published_at: Option<String>,
    assets: Option<Vec<ApiAsset>>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
}

/// 请求失败的完整原因链（reqwest 的 Display 不含底层 source，拼出 DNS / 连接 / TLS 具体错误才能定位问题）
fn err_chain(e: &dyn std::error::Error) -> String {
    let mut chain = e.to_string();
    let mut source = e.source();
    while let Some(err) = source {
        chain.push_str(&format!(": {err}"));
        source = err.source();
    }
    chain
}

/// 带退避重试的请求发送：直连 GitHub 的链路存在间歇性超时，网络类失败自动重试，间隔 1s / 2s
async fn send_with_retry(builder: &reqwest::RequestBuilder, what: &str) -> Result<reqwest::Response, String> {
    let mut last_error = String::new();
    for attempt in 0..3 {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(1000 * attempt as u64)).await;
        }
        let request = builder
            .try_clone()
            .ok_or_else(|| format!("{what}: 请求无法克隆以供重试"))?;
        match request.send().await {
            Ok(response) => return Ok(response),
            Err(error) => last_error = err_chain(&error),
        }
    }
    if last_error.contains("timed out") || last_error.contains("timeout") {
        return Err(format!(
            "{what}: 连接 github.com 超时（已自动重试 3 次）。可稍后重试；若本机需要代理访问 GitHub，请设置 HTTPS_PROXY 环境变量后重启应用"
        ));
    }
    Err(format!("{what}: {last_error}"))
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

/// 经官方接口获取最新 Release，返回版本号、资产名清单与发布信息
async fn fetch_release_api() -> Result<UpdateLatest, String> {
    let client = update_http_client(reqwest::redirect::Policy::default())?;
    let builder = client
        .get(RELEASE_API_URL)
        .header("Accept", "application/vnd.github+json");
    let response = send_with_retry(&builder, "检查应用更新").await?;
    if !response.status().is_success() {
        return Err(format!(
            "GitHub 接口响应异常（{}）",
            response.status().as_u16()
        ));
    }
    let text = response
        .text()
        .await
        .map_err(|e| format!("读取版本信息失败: {}", err_chain(&e)))?;
    let release: ApiRelease = serde_json::from_str(&text).map_err(|_| "版本信息解析失败".to_string())?;
    let tag = release.tag_name.unwrap_or_default();
    if tag.is_empty() {
        return Err("版本信息缺少版本号".to_string());
    }
    let assets = release
        .assets
        .map(|list| list.into_iter().map(|asset| asset.name).collect());
    Ok(UpdateLatest {
        tag,
        assets,
        notes: extract_release_digest(&release.body.unwrap_or_default()),
        html_url: release.html_url.unwrap_or_default(),
        published_at: release.published_at.unwrap_or_default(),
    })
}

/// 提取 release 正文前两个 `###` 章节（构建信息与更新内容），清洗 markdown 标记为可读纯文本；
/// 无章节结构时清洗全文兜底
fn extract_release_digest(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let section_heads: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim_start().starts_with("### "))
        .map(|(index, _)| index)
        .collect();
    let slice = match section_heads.len() {
        0 => &lines[..],
        1 => &lines[section_heads[0]..],
        _ => &lines[section_heads[0]..section_heads[2]],
    };
    clean_markdown(&slice.join("\n"))
}

/// 清洗 markdown 标记：去标题井号、加粗星号与行内代码反引号，保留行结构与换行
fn clean_markdown(text: &str) -> String {
    text.lines()
        .map(|line| {
            let trimmed = line.trim_start();
            let stripped = trimmed.strip_prefix("### ").unwrap_or(trimmed);
            stripped.replace("**", "").replace('`', "")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 经发布页 302 跳转地址解析最新版本号（无资产清单与发布信息）
async fn fetch_release_redirect() -> Result<UpdateLatest, String> {
    let client = update_http_client(reqwest::redirect::Policy::none())?;
    let builder = client.get(RELEASE_LATEST_URL);
    let response = send_with_retry(&builder, "检查应用更新").await?;
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
        notes: String::new(),
        html_url: String::new(),
        published_at: String::new(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_digest_keeps_first_two_sections_and_cleans_markdown() {
        let body = "## 🎉 PicBed Switcher Desktop v1.2.0 发布\n\n### 📋 构建信息\n- **版本**: v1.2.0\n- **Commit**: `e01a53d`\n\n### 📝 更新内容\n- feat: 某功能 (ab12cd3)\n\n### 📦 安装包下载\n- Windows AMD64: `setup.exe`\n";
        let digest = extract_release_digest(body);
        assert!(digest.starts_with("📋 构建信息"));
        assert!(digest.contains("版本: v1.2.0"));
        assert!(digest.contains("Commit: e01a53d"));
        assert!(digest.contains("📝 更新内容"));
        assert!(digest.contains("- feat: 某功能 (ab12cd3)"));
        assert!(!digest.contains("###"));
        assert!(!digest.contains("**"));
        assert!(!digest.contains('`'));
        assert!(!digest.contains("安装包下载"));
        assert!(!digest.contains("🎉"));
    }

    #[test]
    fn release_digest_falls_back_to_full_body_without_sections() {
        let digest = extract_release_digest("- **版本**: v1.0.0");
        assert_eq!(digest, "- 版本: v1.0.0");
    }
}
