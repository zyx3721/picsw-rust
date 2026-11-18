//! 判断图片地址是否属于目标图床配置的仓库、存储桶或域名

use serde_json::{Map, Value};
use url::Url;

/// 判断图片地址是否属于目标图床配置
pub fn is_owned_by_target(raw_url: &str, picbed_type: &str, cfg: &Map<String, Value>) -> bool {
    let Some(parsed) = ParsedUrl::parse(raw_url) else {
        return false;
    };
    match picbed_type {
        "github" => matches_repository(&parsed, cfg, RepositoryHost::Github),
        "gitee" => matches_repository(&parsed, cfg, RepositoryHost::Gitee),
        "tencent" => matches_bucket_region(&parsed, cfg, ".cos.", ".myqcloud.com"),
        "aliyun" => matches_aliyun(&parsed, cfg),
        "qiniu" => matches_bucket_region(&parsed, cfg, ".s3.", ".qiniucs.com"),
        "baidu_bos" => matches_bucket_region(&parsed, cfg, ".s3.", ".bcebos.com"),
        "huawei_obs" => matches_bucket_region(&parsed, cfg, ".obs.", ".myhuaweicloud.com"),
        "minio" => matches_minio(&parsed, cfg),
        "easyimage" => matches_easyimage(&parsed, cfg),
        _ => false,
    }
}

enum RepositoryHost {
    Github,
    Gitee,
}

struct ParsedUrl {
    host: String,
    port: Option<u16>,
    segments: Vec<String>,
}

impl ParsedUrl {
    fn parse(raw_url: &str) -> Option<Self> {
        let parsed = Url::parse(raw_url.trim()).ok()?;
        let host = parsed.host_str()?.to_ascii_lowercase();
        let port = explicit_port(&parsed);
        let segments = parsed
            .path_segments()
            .map(|segments| segments.map(|segment| segment.to_string()).collect())
            .unwrap_or_default();
        Some(Self { host, port, segments })
    }
}

fn matches_repository(parsed: &ParsedUrl, cfg: &Map<String, Value>, host: RepositoryHost) -> bool {
    let Some((owner, repo)) = split_repository(cfg_str(cfg, "repository")) else {
        return false;
    };
    let branch = normalize_branch(cfg_str(cfg, "branch"));
    if branch.is_empty() {
        return false;
    }
    let start = match host {
        RepositoryHost::Github if parsed.host == "github.com" => {
            if !matches!(parsed.segments.get(2).map(String::as_str), Some("raw" | "blob")) {
                return false;
            }
            3
        }
        RepositoryHost::Github if parsed.host == "raw.githubusercontent.com" => 2,
        RepositoryHost::Gitee if parsed.host == "gitee.com" => {
            if !matches!(parsed.segments.get(2).map(String::as_str), Some("raw" | "blob")) {
                return false;
            }
            3
        }
        _ => return false,
    };
    if parsed.segments.len() <= start {
        return false;
    }
    if !eq_ignore_case(&parsed.segments[0], owner) || !eq_ignore_case(&parsed.segments[1], repo) {
        return false;
    }
    let branch_and_path = normalize_branch_path(&parsed.segments[start..]);
    let branch_prefix = format!("{}/", branch.to_ascii_lowercase());
    branch_and_path.to_ascii_lowercase().starts_with(&branch_prefix)
}

fn split_repository(value: &str) -> Option<(&str, &str)> {
    let trimmed = value.trim().trim_matches('/');
    let mut parts = trimmed.split('/');
    let owner = parts.next()?.trim();
    let repo = parts.next()?.trim();
    if owner.is_empty() || repo.is_empty() || parts.next().is_some() {
        return None;
    }
    Some((owner, repo))
}

fn normalize_branch(value: &str) -> String {
    let trimmed = value.trim().trim_matches('/');
    strip_heads_prefix(trimmed).trim_matches('/').to_string()
}

fn normalize_branch_path(segments: &[String]) -> String {
    let joined = segments.join("/");
    strip_heads_prefix(joined.trim_start_matches('/')).to_string()
}

fn strip_heads_prefix(value: &str) -> &str {
    const PREFIX: &str = "refs/heads/";
    if value
        .get(..PREFIX.len())
        .map(|prefix| prefix.eq_ignore_ascii_case(PREFIX))
        .unwrap_or(false)
    {
        &value[PREFIX.len()..]
    } else {
        value
    }
}

fn matches_bucket_region(parsed: &ParsedUrl, cfg: &Map<String, Value>, separator: &str, suffix: &str) -> bool {
    let bucket = cfg_str(cfg, "bucket").trim();
    let region = cfg_str(cfg, "region").trim();
    if bucket.is_empty() || region.is_empty() {
        return false;
    }
    let Some(rest) = parsed.host.strip_suffix(suffix) else {
        return false;
    };
    let Some((url_bucket, url_region)) = rest.rsplit_once(separator) else {
        return false;
    };
    eq_ignore_case(url_bucket, bucket) && eq_ignore_case(url_region, region)
}

fn matches_aliyun(parsed: &ParsedUrl, cfg: &Map<String, Value>) -> bool {
    let bucket = cfg_str(cfg, "bucket").trim();
    let raw_region = {
        let region = cfg_str(cfg, "region").trim();
        if region.is_empty() {
            cfg_str(cfg, "endpoint").trim()
        } else {
            region
        }
    };
    let region = normalize_aliyun_region(raw_region);
    if bucket.is_empty() || region.is_empty() {
        return false;
    }
    let Some(rest) = parsed.host.strip_suffix(".aliyuncs.com") else {
        return false;
    };
    let Some((url_bucket, url_region)) = rest.rsplit_once(".oss-") else {
        return false;
    };
    eq_ignore_case(url_bucket, bucket) && eq_ignore_case(url_region, &region)
}

fn normalize_aliyun_region(value: &str) -> String {
    let trimmed = value.trim().trim_end_matches('/');
    let trimmed = trimmed.strip_suffix(".aliyuncs.com").unwrap_or(trimmed);
    let trimmed = trimmed.strip_prefix("oss-").unwrap_or(trimmed);
    trimmed.to_ascii_lowercase()
}

fn matches_minio(parsed: &ParsedUrl, cfg: &Map<String, Value>) -> bool {
    let bucket = cfg_str(cfg, "bucket").trim();
    if bucket.is_empty() {
        return false;
    }
    let Some(endpoint) = parse_endpoint(cfg_str(cfg, "endpoint")) else {
        return false;
    };
    if parsed.port != endpoint.port {
        return false;
    }
    if parsed.host == endpoint.host {
        return parsed
            .segments
            .first()
            .map(|segment| eq_ignore_case(segment, bucket))
            .unwrap_or(false);
    }
    let virtual_host = format!("{}.{}", bucket.to_ascii_lowercase(), endpoint.host);
    parsed.host == virtual_host
}

struct Endpoint {
    host: String,
    port: Option<u16>,
}

fn parse_endpoint(value: &str) -> Option<Endpoint> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let candidate = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{}", trimmed)
    };
    let parsed = Url::parse(&candidate).ok()?;
    Some(Endpoint {
        host: parsed.host_str()?.to_ascii_lowercase(),
        port: explicit_port(&parsed),
    })
}

fn matches_easyimage(parsed: &ParsedUrl, cfg: &Map<String, Value>) -> bool {
    let api_url = cfg_str(cfg, "api_url").trim();
    if api_url.is_empty() {
        return false;
    }
    let Ok(api) = Url::parse(api_url) else {
        return false;
    };
    let Some(api_host) = api.host_str() else {
        return false;
    };
    parsed.host == api_host.to_ascii_lowercase()
}

fn explicit_port(url: &Url) -> Option<u16> {
    match url.port() {
        Some(port) if Some(port) == default_port(url.scheme()) => None,
        other => other,
    }
}

fn default_port(scheme: &str) -> Option<u16> {
    match scheme {
        "http" => Some(80),
        "https" => Some(443),
        _ => None,
    }
}

fn cfg_str<'a>(cfg: &'a Map<String, Value>, key: &str) -> &'a str {
    cfg.get(key).and_then(|value| value.as_str()).unwrap_or("")
}

fn eq_ignore_case(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

#[cfg(test)]
mod tests {
    use super::is_owned_by_target;
    use serde_json::{json, Map, Value};

    fn cfg(value: Value) -> Map<String, Value> {
        value.as_object().expect("config object").clone()
    }

    #[test]
    fn github_requires_repository_and_branch() {
        let config = cfg(json!({ "repository": "me/pic", "branch": "main" }));
        assert!(is_owned_by_target("https://github.com/me/pic/raw/main/a.png", "github", &config));
        assert!(is_owned_by_target("https://github.com/me/pic/blob/main/a.png", "github", &config));
        assert!(is_owned_by_target("https://raw.githubusercontent.com/me/pic/main/a.png", "github", &config));
        assert!(is_owned_by_target("https://raw.githubusercontent.com/me/pic/refs/heads/main/a.png", "github", &config));
        assert!(is_owned_by_target("https://github.com/me/pic/raw/refs/heads/main/a.png", "github", &config));
        assert!(is_owned_by_target("https://github.com/ME/PIC/raw/MAIN/a.png", "github", &config));
        assert!(!is_owned_by_target("https://github.com/me/pic/raw/dev/a.png", "github", &config));
        assert!(!is_owned_by_target("https://github.com/me/pic/raw/main2/a.png", "github", &config));
        assert!(!is_owned_by_target("https://github.com/other/pic/raw/main/a.png", "github", &config));
        assert!(!is_owned_by_target("https://github.com.evil.com/me/pic/raw/main/a.png", "github", &config));
        assert!(!is_owned_by_target("https://camo.githubusercontent.com/abc", "github", &config));
    }

    #[test]
    fn github_supports_branch_with_slash() {
        let config = cfg(json!({ "repository": "me/pic", "branch": "feature/x" }));
        assert!(is_owned_by_target("https://github.com/me/pic/raw/feature/x/a.png", "github", &config));
        assert!(is_owned_by_target("https://raw.githubusercontent.com/me/pic/feature/x/a.png", "github", &config));
        assert!(!is_owned_by_target("https://github.com/me/pic/raw/feature/y/a.png", "github", &config));
    }

    #[test]
    fn gitee_requires_repository_and_branch() {
        let config = cfg(json!({ "repository": "me/pic", "branch": "main" }));
        assert!(is_owned_by_target("https://gitee.com/me/pic/raw/main/a.png", "gitee", &config));
        assert!(is_owned_by_target("https://gitee.com/ME/PIC/raw/MAIN/a.png", "gitee", &config));
        assert!(!is_owned_by_target("https://gitee.com/master_hj/pic/raw/master/a.png", "gitee", &config));
        assert!(!is_owned_by_target("https://gitee.com/me/pic/raw/dev/a.png", "gitee", &config));
        assert!(!is_owned_by_target("https://fakegitee.com/me/pic/raw/main/a.png", "gitee", &config));
        let screenshot = cfg(json!({ "repository": "master_hj/pic", "branch": "master" }));
        assert!(is_owned_by_target(
            "https://gitee.com/master_hj/pic/raw/master/img/202211251758722.png",
            "gitee",
            &screenshot
        ));
    }

    #[test]
    fn tencent_requires_bucket_and_region() {
        let config = cfg(json!({ "bucket": "pic-1250000000", "region": "ap-guangzhou" }));
        assert!(is_owned_by_target(
            "https://pic-1250000000.cos.ap-guangzhou.myqcloud.com/a.png",
            "tencent",
            &config
        ));
        assert!(!is_owned_by_target(
            "https://pic-1250000000.cos.ap-shanghai.myqcloud.com/a.png",
            "tencent",
            &config
        ));
        assert!(!is_owned_by_target(
            "https://other-1250000000.cos.ap-guangzhou.myqcloud.com/a.png",
            "tencent",
            &config
        ));
        assert!(!is_owned_by_target(
            "https://pic-1250000000.cos.ap-guangzhou.myqcloud.com.evil.com/a.png",
            "tencent",
            &config
        ));
        let with_domain = cfg(json!({
            "bucket": "pic-1250000000",
            "region": "ap-guangzhou",
            "custom_domain": "https://cdn.example.com"
        }));
        assert!(is_owned_by_target(
            "https://pic-1250000000.cos.ap-guangzhou.myqcloud.com/a.png",
            "tencent",
            &with_domain
        ));
        assert!(!is_owned_by_target("https://cdn.example.com/a.png", "tencent", &with_domain));
    }

    #[test]
    fn aliyun_requires_bucket_and_region() {
        let config = cfg(json!({ "bucket": "pic", "region": "cn-guangzhou" }));
        assert!(is_owned_by_target("https://pic.oss-cn-guangzhou.aliyuncs.com/a.png", "aliyun", &config));
        assert!(!is_owned_by_target("https://pic.oss-cn-shanghai.aliyuncs.com/a.png", "aliyun", &config));
        assert!(!is_owned_by_target("https://pic.oss.aliyuncs.com/a.png", "aliyun", &config));
        let prefixed = cfg(json!({ "bucket": "pic", "region": "oss-cn-guangzhou" }));
        assert!(is_owned_by_target("https://pic.oss-cn-guangzhou.aliyuncs.com/a.png", "aliyun", &prefixed));
    }

    #[test]
    fn qiniu_requires_bucket_and_region() {
        let config = cfg(json!({ "bucket": "pic", "region": "cn-east-1" }));
        assert!(is_owned_by_target("https://pic.s3.cn-east-1.qiniucs.com/a.png", "qiniu", &config));
        assert!(!is_owned_by_target("https://pic.s3.cn-east-2.qiniucs.com/a.png", "qiniu", &config));
        assert!(!is_owned_by_target("https://cdn.example.com/a.png", "qiniu", &config));
    }

    #[test]
    fn baidu_requires_bucket_and_region() {
        let config = cfg(json!({ "bucket": "pic", "region": "bj" }));
        assert!(is_owned_by_target("https://pic.s3.bj.bcebos.com/a.png", "baidu_bos", &config));
        assert!(!is_owned_by_target("https://pic.s3.gz.bcebos.com/a.png", "baidu_bos", &config));
        assert!(!is_owned_by_target("https://pic.s3.bj.bcebos.com.evil.com/a.png", "baidu_bos", &config));
    }

    #[test]
    fn huawei_requires_bucket_and_region() {
        let config = cfg(json!({ "bucket": "pic", "region": "cn-north-4" }));
        assert!(is_owned_by_target(
            "https://pic.obs.cn-north-4.myhuaweicloud.com/a.png",
            "huawei_obs",
            &config
        ));
        assert!(!is_owned_by_target(
            "https://pic.obs.cn-south-1.myhuaweicloud.com/a.png",
            "huawei_obs",
            &config
        ));
    }

    #[test]
    fn minio_requires_endpoint_and_bucket() {
        let config = cfg(json!({ "endpoint": "minio.example.com:9000", "bucket": "pic" }));
        assert!(is_owned_by_target("https://minio.example.com:9000/pic/a.png", "minio", &config));
        assert!(is_owned_by_target("http://minio.example.com:9000/pic/a.png", "minio", &config));
        assert!(is_owned_by_target("https://pic.minio.example.com:9000/a.png", "minio", &config));
        assert!(!is_owned_by_target("https://minio.example.com:9000/other/a.png", "minio", &config));
        assert!(!is_owned_by_target("https://minio.example.com:9001/pic/a.png", "minio", &config));
        assert!(!is_owned_by_target("https://other.example.com:9000/pic/a.png", "minio", &config));
        let with_domain = cfg(json!({
            "endpoint": "http://minio.example.com:9000",
            "bucket": "pic",
            "custom_domain": "https://cdn.example.com"
        }));
        assert!(is_owned_by_target("https://minio.example.com:9000/pic/a.png", "minio", &with_domain));
        assert!(!is_owned_by_target("https://cdn.example.com/pic/a.png", "minio", &with_domain));
    }

    #[test]
    fn easyimage_requires_api_host() {
        let config = cfg(json!({ "api_url": "http://pic.its.sunline.cn/api/index.php" }));
        assert!(is_owned_by_target("https://pic.its.sunline.cn/i/a.png", "easyimage", &config));
        assert!(is_owned_by_target("https://pic.its.sunline.cn:8443/i/a.png", "easyimage", &config));
        assert!(!is_owned_by_target("https://other.sunline.cn/i/a.png", "easyimage", &config));
        assert!(!is_owned_by_target("https://pic.its.sunline.cn.evil.com/i/a.png", "easyimage", &config));
    }

    #[test]
    fn upyun_and_other_never_skip() {
        let config = cfg(json!({ "bucket": "pic", "custom_domain": "https://cdn.example.com" }));
        assert!(!is_owned_by_target("https://cdn.example.com/a.png", "upyun", &config));
        assert!(!is_owned_by_target("https://cdn.example.com/a.png", "other", &config));
        assert!(!is_owned_by_target("not-a-url", "github", &config));
    }
}
