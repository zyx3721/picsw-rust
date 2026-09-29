import type { PicbedTypeDef } from './types';

export const defaultFilenameFormat = '{y}/{m}/{d}/{origin}{ext}';

export const fallbackTypes: PicbedTypeDef[] = [
  {
    value: 'github',
    label: 'GitHub',
    description: '仓库图床',
    fields: [
      { key: 'repository', label: '仓库名', placeholder: 'owner/repo', required: true, secret: false },
      { key: 'branch', label: '分支名', placeholder: 'main', required: true, secret: false },
      {
        key: 'token',
        label: 'Token',
        placeholder: 'GitHub Personal Access Token',
        required: true,
        secret: true,
      },
      { key: 'storage_path', label: '存储路径', placeholder: 'images/blog', required: false, secret: false },
      {
        key: 'filename_format',
        label: '文件命名格式',
        placeholder: defaultFilenameFormat,
        required: false,
        secret: false,
      },
    ],
  },
  { value: 'gitee', label: 'Gitee', description: '仓库图床', fields: [] },
  { value: 'tencent', label: '腾讯云 COS', description: '对象存储', fields: [] },
  { value: 'aliyun', label: '阿里云 OSS', description: '对象存储', fields: [] },
  { value: 'qiniu', label: '七牛云 Kodo', description: '对象存储', fields: [] },
  { value: 'baidu_bos', label: '百度云 BOS', description: '对象存储', fields: [] },
  { value: 'huawei_obs', label: '华为云 OBS', description: '对象存储', fields: [] },
  { value: 'upyun', label: '又拍云 USS', description: '对象存储', fields: [] },
  { value: 'minio', label: 'MinIO', description: 'S3 兼容对象存储', fields: [] },
  { value: 'easyimage', label: 'EasyImage', description: '自建图床', fields: [] },
  {
    value: 'other',
    label: '其他',
    description: '通用上传接口',
    fields: [
      {
        key: 'api_url',
        label: 'API 地址',
        placeholder: 'https://img.example.com/api/index.php',
        required: true,
        secret: false,
      },
      { key: 'token', label: 'Token', placeholder: 'Upload API token', required: true, secret: true },
      {
        key: 'filename_format',
        label: '文件命名格式',
        placeholder: defaultFilenameFormat,
        required: false,
        secret: false,
      },
    ],
  },
];

export const zhTypeLabels: Record<string, string> = {
  github: 'GitHub',
  gitee: 'Gitee',
  tencent: '腾讯云 COS',
  aliyun: '阿里云 OSS',
  qiniu: '七牛云 Kodo',
  baidu_bos: '百度云 BOS',
  huawei_obs: '华为云 OBS',
  upyun: '又拍云 USS',
  minio: 'MinIO',
  easyimage: 'EasyImage',
  other: '其他',
  unknown: '未知来源',
  mixed: '混合来源',
};
export const zhFieldLabels: Record<string, string> = {
  repository: '仓库名',
  branch: '分支名',
  token: 'Token',
  storage_path: '存储路径',
  custom_domain: '自定义域名',
  secret_id: 'SecretId',
  secret_key: 'SecretKey',
  bucket: '存储桶',
  region: '地域',
  access_key_id: 'AccessKeyId',
  access_key_secret: 'AccessKeySecret',
  secret_access_key: 'SecretAccessKey',
  endpoint: '地域',
  access_key: 'AccessKey',
  operator: '操作员',
  password: '密码',
  use_ssl: '是否使用 SSL',
  api_url: 'API 地址',
  base_url: '公开访问根地址',
  auth_token: '认证 Token',
  filename_format: '文件命名格式',
};

export function defaultConfigValues(picbedType: string): Record<string, string> {
  if (picbedType === 'minio') return { filename_format: defaultFilenameFormat, use_ssl: 'false' };
  return picbedType === 'easyimage' ? {} : { filename_format: defaultFilenameFormat };
}

