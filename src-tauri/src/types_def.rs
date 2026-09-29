use serde::Serialize;

use crate::markdown::DEFAULT_FILENAME_FORMAT;

#[derive(Debug, Clone, Serialize)]
pub struct ConfigField {
    pub key: String,
    pub label: String,
    pub placeholder: String,
    pub required: bool,
    pub secret: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PicbedTypeDef {
    pub value: String,
    pub label: String,
    pub description: String,
    pub fields: Vec<ConfigField>,
}

fn filename_format_field() -> ConfigField {
    ConfigField {
        key: "filename_format".to_string(),
        label: "Filename format".to_string(),
        placeholder: DEFAULT_FILENAME_FORMAT.to_string(),
        required: false,
        secret: false,
    }
}

fn field(key: &str, label: &str, placeholder: &str, required: bool, secret: bool) -> ConfigField {
    ConfigField {
        key: key.to_string(),
        label: label.to_string(),
        placeholder: placeholder.to_string(),
        required,
        secret,
    }
}

pub fn picbed_type_defs() -> Vec<PicbedTypeDef> {
    vec![
        PicbedTypeDef {
            value: "github".to_string(),
            label: "GitHub".to_string(),
            description: "GitHub repository storage.".to_string(),
            fields: vec![
                field("repository", "Repository", "owner/repo", true, false),
                field("branch", "Branch", "main", true, false),
                field("token", "Token", "GitHub Personal Access Token", true, true),
                field("storage_path", "Storage path", "images/blog", false, false),
                filename_format_field(),
            ],
        },
        PicbedTypeDef {
            value: "gitee".to_string(),
            label: "Gitee".to_string(),
            description: "Gitee repository storage.".to_string(),
            fields: vec![
                field("repository", "Repository", "owner/repo", true, false),
                field("branch", "Branch", "master", true, false),
                field("token", "Token", "Gitee Access Token", true, true),
                field("storage_path", "Storage path", "images/blog", false, false),
                filename_format_field(),
            ],
        },
        PicbedTypeDef {
            value: "tencent".to_string(),
            label: "Tencent COS".to_string(),
            description: "Tencent Cloud COS storage.".to_string(),
            fields: vec![
                field("secret_id", "SecretId", "AKID...", true, true),
                field("secret_key", "SecretKey", "SecretKey", true, true),
                field("bucket", "Bucket", "bucket-1250000000", true, false),
                field("region", "Region", "ap-guangzhou", true, false),
                field("storage_path", "Storage path", "markdown/images", false, false),
                field("custom_domain", "Public domain", "https://cdn.example.com", false, false),
                filename_format_field(),
            ],
        },
        PicbedTypeDef {
            value: "aliyun".to_string(),
            label: "Aliyun OSS".to_string(),
            description: "Aliyun OSS storage.".to_string(),
            fields: vec![
                field("access_key_id", "AccessKeyId", "LTAI...", true, true),
                field("access_key_secret", "AccessKeySecret", "AccessKeySecret", true, true),
                field("bucket", "Bucket", "bucket-name", true, false),
                field("region", "Region", "cn-guangzhou", true, false),
                field("storage_path", "Storage path", "markdown/images", false, false),
                field("custom_domain", "Public domain", "https://cdn.example.com", false, false),
                filename_format_field(),
            ],
        },
        PicbedTypeDef {
            value: "qiniu".to_string(),
            label: "Qiniu".to_string(),
            description: "Qiniu Kodo storage.".to_string(),
            fields: vec![
                field("access_key", "AccessKey", "AccessKey", true, true),
                field("secret_key", "SecretKey", "SecretKey", true, true),
                field("bucket", "Bucket", "bucket-name", true, false),
                field("region", "Region", "cn-east-1", true, false),
                field("storage_path", "Storage path", "markdown/images", false, false),
                field("custom_domain", "Custom domain / CDN test domain", "https://cdn.example.com", true, false),
                filename_format_field(),
            ],
        },
        PicbedTypeDef {
            value: "baidu_bos".to_string(),
            label: "Baidu BOS".to_string(),
            description: "Baidu Cloud BOS storage.".to_string(),
            fields: vec![
                field("access_key_id", "AccessKeyId", "AccessKeyId", true, true),
                field("secret_access_key", "SecretAccessKey", "SecretAccessKey", true, true),
                field("bucket", "Bucket", "bucket-name", true, false),
                field("region", "Region", "bj", true, false),
                field("storage_path", "Storage path", "markdown/images", false, false),
                field("custom_domain", "Public domain", "https://cdn.example.com", false, false),
                filename_format_field(),
            ],
        },
        PicbedTypeDef {
            value: "huawei_obs".to_string(),
            label: "Huawei OBS".to_string(),
            description: "Huawei Cloud OBS storage.".to_string(),
            fields: vec![
                field("access_key_id", "AccessKeyId", "AccessKeyId", true, true),
                field("secret_access_key", "SecretAccessKey", "SecretAccessKey", true, true),
                field("bucket", "Bucket", "bucket-name", true, false),
                field("region", "Region", "cn-north-4", true, false),
                field("storage_path", "Storage path", "markdown/images", false, false),
                field("custom_domain", "Public domain", "https://cdn.example.com", false, false),
                filename_format_field(),
            ],
        },
        PicbedTypeDef {
            value: "upyun".to_string(),
            label: "UpYun USS".to_string(),
            description: "UpYun cloud storage.".to_string(),
            fields: vec![
                field("bucket", "Service name", "service-name", true, false),
                field("operator", "Operator", "operator", true, false),
                field("password", "Password", "Operator password", true, true),
                field("storage_path", "Storage path", "markdown/images", false, false),
                field("custom_domain", "Acceleration domain / test domain", "https://img.example.com", true, false),
                filename_format_field(),
            ],
        },
        PicbedTypeDef {
            value: "minio".to_string(),
            label: "MinIO".to_string(),
            description: "Self-hosted S3-compatible storage.".to_string(),
            fields: vec![
                field("endpoint", "Endpoint", "minio.example.com:9000", true, false),
                field("access_key", "AccessKey", "minioadmin", true, true),
                field("secret_key", "SecretKey", "minioadmin", true, true),
                field("bucket", "Bucket", "bucket-name", true, false),
                field("region", "Region", "us-east-1", false, false),
                field("use_ssl", "Use SSL", "true / false", false, false),
                field("storage_path", "Storage path", "markdown/images", false, false),
                field("custom_domain", "Public domain", "https://cdn.example.com", false, false),
                filename_format_field(),
            ],
        },
        PicbedTypeDef {
            value: "easyimage".to_string(),
            label: "EasyImage".to_string(),
            description: "Self-hosted EasyImage service.".to_string(),
            fields: vec![
                field("api_url", "API URL", "https://img.example.com/api/index.php", true, false),
                field("token", "Token", "EasyImage Token", true, true),
            ],
        },
        PicbedTypeDef {
            value: "other".to_string(),
            label: "Other".to_string(),
            description: "Generic image host service.".to_string(),
            fields: vec![
                field("api_url", "API URL", "https://img.example.com/api/index.php", true, false),
                field("token", "Token", "Upload API token", true, true),
                filename_format_field(),
            ],
        },
    ]
}

pub fn find_picbed_type(value: &str) -> Option<PicbedTypeDef> {
    picbed_type_defs().into_iter().find(|item| item.value == value)
}

pub fn display_field_label(field_item: &ConfigField) -> String {
    let labels = [
        ("repository", "仓库名"),
        ("branch", "分支名"),
        ("token", "Token"),
        ("storage_path", "存储路径"),
        ("custom_domain", "自定义域名"),
        ("secret_id", "SecretId"),
        ("secret_key", "SecretKey"),
        ("bucket", "存储桶"),
        ("region", "地域"),
        ("access_key_id", "AccessKeyId"),
        ("access_key_secret", "AccessKeySecret"),
        ("secret_access_key", "SecretAccessKey"),
        ("endpoint", "Endpoint"),
        ("access_key", "AccessKey"),
        ("operator", "操作员"),
        ("password", "密码"),
        ("use_ssl", "是否使用 SSL"),
        ("api_url", "API 地址"),
        ("base_url", "公开访问根地址"),
        ("auth_token", "认证 Token"),
    ];
    labels
        .iter()
        .find(|(key, _)| *key == field_item.key)
        .map(|(_, label)| label.to_string())
        .unwrap_or_else(|| field_item.label.clone())
}
