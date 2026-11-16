/// 后端中文错误到英文的固定句式映射，仅在英文界面生效
import { getLocale } from './index';

type TranslationRule = {
  pattern: RegExp;
  replacement: string;
};

const RULES: TranslationRule[] = [
  { pattern: /（已自动重试 (\d+) 次）/, replacement: ' (auto-retried $1 times)' },
  {
    pattern: /上传失败：\{"message":"(?:文件名已存在|文件已存在|文件已经存在)"\}/,
    replacement: 'Upload failed: a file with the same name already exists',
  },
  { pattern: /上传失败：\{"message":"文件新建失败"\}/, replacement: 'Upload failed: the file could not be created' },
  {
    pattern: /Gitee 目标文件已存在且内容不同：(.*?)，请调整文件命名格式或存储路径/,
    replacement: 'Gitee target file already exists with different content: $1; change the filename format or storage path',
  },
  {
    pattern: /Gitee 目标路径已被同名目录占用：(.*?)，请更换存储路径或文件命名格式/,
    replacement: 'Gitee target path is already used by a directory: $1; change the storage path or filename format',
  },
  {
    pattern: /Gitee 配置缺少仓库、分支或 Token/,
    replacement: 'Gitee configuration is missing the repository, branch, or token',
  },
  { pattern: /Gitee 上传响应缺少图片地址/, replacement: 'Gitee upload response did not include an image URL' },
  { pattern: /Gitee 上传失败/, replacement: 'Gitee upload failed' },
  {
    pattern: /GitHub 配置缺少仓库、分支或 Token/,
    replacement: 'GitHub configuration is missing the repository, branch, or token',
  },
  {
    pattern: /腾讯云 COS 配置缺少 SecretId、SecretKey、存储桶或地域/,
    replacement: 'Tencent COS configuration is missing SecretId, SecretKey, bucket, or region',
  },
  {
    pattern: /阿里云 OSS 配置缺少 AccessKeyId、AccessKeySecret、存储桶或地域/,
    replacement: 'Aliyun OSS configuration is missing AccessKeyId, AccessKeySecret, bucket, or region',
  },
  {
    pattern: /七牛云 Kodo 配置缺少 AccessKey、SecretKey、存储桶或地域/,
    replacement: 'Qiniu Kodo configuration is missing AccessKey, SecretKey, bucket, or region',
  },
  {
    pattern: /百度云 BOS 配置缺少 AccessKeyId、SecretAccessKey、存储桶或地域/,
    replacement: 'Baidu BOS configuration is missing AccessKeyId, SecretAccessKey, bucket, or region',
  },
  {
    pattern: /华为云 OBS 配置缺少 AccessKeyId、SecretAccessKey、存储桶或地域/,
    replacement: 'Huawei OBS configuration is missing AccessKeyId, SecretAccessKey, bucket, or region',
  },
  {
    pattern: /MinIO 配置缺少 Endpoint、AccessKey、SecretKey 或存储桶/,
    replacement: 'MinIO configuration is missing Endpoint, AccessKey, SecretKey, or bucket',
  },
  {
    pattern: /又拍云 USS 配置缺少服务名、操作员或密码/,
    replacement: 'Upyun USS configuration is missing the service name, operator, or password',
  },
  {
    pattern: /又拍云 USS 配置缺少加速域名，无法生成公开访问地址/,
    replacement: 'Upyun USS configuration is missing the acceleration domain, so a public URL cannot be generated',
  },
  { pattern: /EasyImage 配置缺少 API 地址或 Token/, replacement: 'EasyImage configuration is missing the API URL or token' },
  { pattern: /暂不支持上传到(.+)$/, replacement: 'Uploading to $1 is not supported' },
  { pattern: /本地图片 (\S+) 未匹配到上传文件/, replacement: 'Local image $1 has no matching uploaded file' },
  { pattern: /存在 (\d+) 个图片无法转换/, replacement: '$1 images could not be converted' },
  { pattern: /本地路径无法通过批量转换上传/, replacement: 'Local paths cannot be uploaded by batch conversion' },
  { pattern: /没有本地图片地址被转换/, replacement: 'No local image paths were converted' },
  { pattern: /没有图片地址被转换/, replacement: 'No image URLs were converted' },
  { pattern: /目标图床配置不存在/, replacement: 'The target image host configuration does not exist' },
  { pattern: /读取图床配置失败/, replacement: 'Failed to read the image host configuration' },
  { pattern: /Markdown 内容不能为空/, replacement: 'Markdown content cannot be empty' },
  { pattern: /未识别到图片地址/, replacement: 'No image URLs were found' },
  { pattern: /该地址不是有效图片/, replacement: 'The URL does not point to a valid image' },
  { pattern: /上传文件不是有效图片/, replacement: 'The selected file is not a valid image' },
  { pattern: /图片大小不能超过 20MB/, replacement: 'Images must be 20 MB or smaller' },
  { pattern: /图片地址仅支持 HTTP 或 HTTPS/, replacement: 'Only HTTP or HTTPS image URLs are supported' },
  { pattern: /图片地址不正确/, replacement: 'Invalid image URL' },
  { pattern: /图片下载失败：/, replacement: 'Image download failed: ' },
  { pattern: /上传图片 (\S+) 失败：/, replacement: 'Failed to upload image $1: ' },
  { pattern: /下载图片 (\S+) 失败：/, replacement: 'Failed to download image $1: ' },
  { pattern: /上传失败：/, replacement: 'Upload failed: ' },
];

function applyFirstRule(message: string): string | null {
  for (const rule of RULES) {
    if (rule.pattern.test(message)) return message.replace(rule.pattern, rule.replacement);
  }
  return null;
}

/// 按固定句式把后端中文错误转换为英文，未命中时保留原文
export function localizeError(message: string): string {
  if (!message || getLocale() !== 'en') return message;
  let current = message;
  for (let pass = 0; pass < 4; pass += 1) {
    const next = applyFirstRule(current);
    if (next === null || next === current) break;
    current = next;
  }
  return current;
}
