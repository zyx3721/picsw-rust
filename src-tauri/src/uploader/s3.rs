use super::{cfg_str, custom_public_url, http_client, object_path, trim_cfg, ImageFile, UploadResult};
use hmac::{Hmac, Mac};
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use sha2::{Digest, Sha256};
use std::time::Duration;

const S3_URI_SET: &AsciiSet = &NON_ALPHANUMERIC.remove(b'-').remove(b'_').remove(b'.').remove(b'~');

#[derive(Debug, Clone)]
pub struct S3CompatibleConfig {
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
    pub region: String,
    pub endpoint: String,
    pub secure: bool,
    pub path_style: bool,
    pub storage_path: String,
    pub custom_domain: String,
}

pub async fn upload_tencent_cos(cfg: &serde_json::Map<String, serde_json::Value>, image: &ImageFile) -> Result<UploadResult, String> {
    let secret_id = trim_cfg(cfg, "secret_id");
    let secret_key = trim_cfg(cfg, "secret_key");
    let bucket = trim_cfg(cfg, "bucket");
    let region = trim_cfg(cfg, "region");
    if secret_id.is_empty() || secret_key.is_empty() || bucket.is_empty() || region.is_empty() {
        return Err("腾讯云 COS 配置缺少 SecretId、SecretKey、存储桶或地域".to_string());
    }
    upload_s3_compatible(
        &S3CompatibleConfig {
            access_key: secret_id,
            secret_key,
            bucket,
            region,
            endpoint: format!("cos.{}.myqcloud.com", trim_cfg(cfg, "region")),
            secure: true,
            path_style: false,
            storage_path: cfg_str(cfg, "storage_path").to_string(),
            custom_domain: cfg_str(cfg, "custom_domain").to_string(),
        },
        image,
    )
    .await
}

pub async fn upload_aliyun_oss(cfg: &serde_json::Map<String, serde_json::Value>, image: &ImageFile) -> Result<UploadResult, String> {
    let access_key_id = trim_cfg(cfg, "access_key_id");
    let access_key_secret = trim_cfg(cfg, "access_key_secret");
    let bucket = trim_cfg(cfg, "bucket");
    let mut region = trim_cfg(cfg, "region");
    if region.is_empty() {
        region = trim_cfg(cfg, "endpoint");
    }
    if access_key_id.is_empty() || access_key_secret.is_empty() || bucket.is_empty() || region.is_empty() {
        return Err("阿里云 OSS 配置缺少 AccessKeyId、AccessKeySecret、存储桶或地域".to_string());
    }
    upload_s3_compatible(
        &S3CompatibleConfig {
            access_key: access_key_id,
            secret_key: access_key_secret,
            bucket,
            region: region.clone(),
            endpoint: format!("oss-{}.aliyuncs.com", region),
            secure: true,
            path_style: false,
            storage_path: cfg_str(cfg, "storage_path").to_string(),
            custom_domain: cfg_str(cfg, "custom_domain").to_string(),
        },
        image,
    )
    .await
}

pub async fn upload_qiniu_kodo(cfg: &serde_json::Map<String, serde_json::Value>, image: &ImageFile) -> Result<UploadResult, String> {
    let access_key = trim_cfg(cfg, "access_key");
    let secret_key = trim_cfg(cfg, "secret_key");
    let bucket = trim_cfg(cfg, "bucket");
    let region = trim_cfg(cfg, "region");
    if access_key.is_empty() || secret_key.is_empty() || bucket.is_empty() || region.is_empty() {
        return Err("七牛云 Kodo 配置缺少 AccessKey、SecretKey、存储桶或地域".to_string());
    }
    upload_s3_compatible(
        &S3CompatibleConfig {
            access_key,
            secret_key,
            bucket,
            region: region.clone(),
            endpoint: format!("s3.{}.qiniucs.com", region),
            secure: true,
            path_style: false,
            storage_path: cfg_str(cfg, "storage_path").to_string(),
            custom_domain: cfg_str(cfg, "custom_domain").to_string(),
        },
        image,
    )
    .await
}

pub async fn upload_baidu_bos(cfg: &serde_json::Map<String, serde_json::Value>, image: &ImageFile) -> Result<UploadResult, String> {
    let access_key_id = trim_cfg(cfg, "access_key_id");
    let secret_access_key = trim_cfg(cfg, "secret_access_key");
    let bucket = trim_cfg(cfg, "bucket");
    let region = trim_cfg(cfg, "region");
    if access_key_id.is_empty() || secret_access_key.is_empty() || bucket.is_empty() || region.is_empty() {
        return Err("百度云 BOS 配置缺少 AccessKeyId、SecretAccessKey、存储桶或地域".to_string());
    }
    upload_s3_compatible(
        &S3CompatibleConfig {
            access_key: access_key_id,
            secret_key: secret_access_key,
            bucket,
            region: region.clone(),
            endpoint: format!("s3.{}.bcebos.com", region),
            secure: true,
            path_style: false,
            storage_path: cfg_str(cfg, "storage_path").to_string(),
            custom_domain: cfg_str(cfg, "custom_domain").to_string(),
        },
        image,
    )
    .await
}

pub async fn upload_huawei_obs(cfg: &serde_json::Map<String, serde_json::Value>, image: &ImageFile) -> Result<UploadResult, String> {
    let access_key_id = trim_cfg(cfg, "access_key_id");
    let secret_access_key = trim_cfg(cfg, "secret_access_key");
    let bucket = trim_cfg(cfg, "bucket");
    let region = trim_cfg(cfg, "region");
    if access_key_id.is_empty() || secret_access_key.is_empty() || bucket.is_empty() || region.is_empty() {
        return Err("华为云 OBS 配置缺少 AccessKeyId、SecretAccessKey、存储桶或地域".to_string());
    }
    upload_s3_compatible(
        &S3CompatibleConfig {
            access_key: access_key_id,
            secret_key: secret_access_key,
            bucket,
            region: region.clone(),
            endpoint: format!("obs.{}.myhuaweicloud.com", region),
            secure: true,
            path_style: false,
            storage_path: cfg_str(cfg, "storage_path").to_string(),
            custom_domain: cfg_str(cfg, "custom_domain").to_string(),
        },
        image,
    )
    .await
}

pub async fn upload_minio(cfg: &serde_json::Map<String, serde_json::Value>, image: &ImageFile) -> Result<UploadResult, String> {
    let access_key = trim_cfg(cfg, "access_key");
    let secret_key = trim_cfg(cfg, "secret_key");
    let bucket = trim_cfg(cfg, "bucket");
    let raw_endpoint = trim_cfg(cfg, "endpoint");
    if access_key.is_empty() || secret_key.is_empty() || bucket.is_empty() || raw_endpoint.is_empty() {
        return Err("MinIO 配置缺少 Endpoint、AccessKey、SecretKey 或存储桶".to_string());
    }
    let (endpoint, secure) = normalize_s3_endpoint(&raw_endpoint, cfg_str(cfg, "use_ssl"));
    upload_s3_compatible(
        &S3CompatibleConfig {
            access_key,
            secret_key,
            bucket,
            region: trim_cfg(cfg, "region"),
            endpoint,
            secure,
            path_style: true,
            storage_path: cfg_str(cfg, "storage_path").to_string(),
            custom_domain: cfg_str(cfg, "custom_domain").to_string(),
        },
        image,
    )
    .await
}

fn normalize_s3_endpoint(endpoint: &str, use_ssl: &str) -> (String, bool) {
    let trimmed = use_ssl.trim();
    let mut secure = trimmed.eq_ignore_ascii_case("true") || trimmed == "1";
    let mut host = endpoint.trim().to_string();
    if let Ok(parsed) = url::Url::parse(endpoint.trim()) {
        if parsed.host_str().is_some() {
            secure = parsed.scheme() != "http";
            host = parsed.host_str().unwrap_or("").to_string();
            if let Some(port) = parsed.port() {
                host = format!("{}:{}", host, port);
            }
        }
    }
    (host.trim_end_matches('/').to_string(), secure)
}

fn uri_encode_key(key: &str) -> String {
    key.split('/')
        .map(|segment| utf8_percent_encode(segment, S3_URI_SET).to_string())
        .collect::<Vec<_>>()
        .join("/")
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("hmac accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

async fn upload_s3_compatible(cfg: &S3CompatibleConfig, image: &ImageFile) -> Result<UploadResult, String> {
    let key = object_path(&cfg.storage_path, &image.filename);
    let region = if cfg.region.trim().is_empty() { "us-east-1".to_string() } else { cfg.region.trim().to_string() };
    let scheme = if cfg.secure { "https" } else { "http" };
    let host = if cfg.path_style {
        cfg.endpoint.clone()
    } else {
        format!("{}.{}", cfg.bucket, cfg.endpoint)
    };
    let canonical_uri = if cfg.path_style {
        format!("/{}/{}", cfg.bucket, uri_encode_key(&key))
    } else {
        format!("/{}", uri_encode_key(&key))
    };
    let request_url = format!("{}://{}{}", scheme, host, canonical_uri);

    let now_utc = chrono::Utc::now();
    let amz_date = now_utc.format("%Y%m%dT%H%M%SZ").to_string();
    let datestamp = now_utc.format("%Y%m%d").to_string();
    let payload_hash = hex::encode(Sha256::digest(&image.data));

    let canonical_headers = format!(
        "content-type:{}\nhost:{}\nx-amz-content-sha256:{}\nx-amz-date:{}\n",
        image.content_type, host, payload_hash, amz_date
    );
    let signed_headers = "content-type;host;x-amz-content-sha256;x-amz-date";
    let canonical_request = format!(
        "PUT\n{}\n\n{}\n{}\n{}",
        canonical_uri, canonical_headers, signed_headers, payload_hash
    );
    let scope = format!("{}/{}/s3/aws4_request", datestamp, region);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{}\n{}",
        amz_date,
        scope,
        hex::encode(Sha256::digest(canonical_request.as_bytes()))
    );
    let k_date = hmac_sha256(format!("AWS4{}", cfg.secret_key).as_bytes(), datestamp.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, b"s3");
    let k_signing = hmac_sha256(&k_service, b"aws4_request");
    let signature = hex::encode(hmac_sha256(&k_signing, string_to_sign.as_bytes()));
    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
        cfg.access_key, scope, signed_headers, signature
    );

    let resp = http_client()
        .put(&request_url)
        .header("Content-Type", &image.content_type)
        .header("x-amz-content-sha256", &payload_hash)
        .header("x-amz-date", &amz_date)
        .header("Authorization", authorization)
        .body(image.data.clone())
        .timeout(Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        let body = body.trim();
        let truncated: String = if body.chars().count() > 512 {
            body.chars().take(512).collect()
        } else {
            body.to_string()
        };
        return Err(format!("上传失败：{}", truncated));
    }

    let custom = custom_public_url(&cfg.custom_domain, &key);
    if !custom.is_empty() {
        return Ok(UploadResult { url: custom });
    }
    let url = if cfg.path_style {
        format!("{}://{}/{}/{}", scheme, cfg.endpoint, cfg.bucket, key)
    } else {
        format!("{}://{}.{}/{}", scheme, cfg.bucket, cfg.endpoint, key)
    };
    Ok(UploadResult { url })
}
