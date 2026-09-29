import { invoke } from '@tauri-apps/api/core';
import type { RequestError } from './types';

type LocalImageUpload = { key: string; name: string; data: string };

function normalizePath(path: string) {
  return path.startsWith('/') ? path : `/${path}`;
}

function parsePathId(path: string) {
  const segments = normalizePath(path).split('/').filter(Boolean);
  const last = segments[segments.length - 1] || '';
  const id = Number.parseInt(last, 10);
  return Number.isFinite(id) && id > 0 ? id : 0;
}

function toRequestError(error: unknown): RequestError {
  if (error instanceof Error) return error as RequestError;
  const message = typeof error === 'string' && error ? error : '请求失败';
  return new Error(message) as RequestError;
}

function readFileAsBase64(file: File) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = String(reader.result || '');
      const base64 = result.includes(',') ? result.slice(result.indexOf(',') + 1) : result;
      resolve(base64);
    };
    reader.onerror = () => reject(new Error(`读取本地图片失败：${file.name}`));
    reader.readAsDataURL(file);
  });
}

async function invokeLocalConvertTask<T>(formData: FormData): Promise<T> {
  const manifestRaw = formData.get('manifest');
  if (typeof manifestRaw !== 'string') throw new Error('本地上传清单格式不正确');
  const manifest = JSON.parse(manifestRaw);
  const images: LocalImageUpload[] = [];
  for (const [key, value] of formData.entries()) {
    if (key === 'manifest' || !(value instanceof File)) continue;
    images.push({ key, name: value.name, data: await readFileAsBase64(value) });
  }
  return invoke<T>('create_local_convert_task', { manifest, images });
}

export function createWorkspaceRequest() {
  return async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
    try {
      if (options.body instanceof FormData) return await invokeLocalConvertTask<T>(options.body);

      const method = (options.method || 'GET').toUpperCase();
      const route = `${method} ${normalizePath(path)}`;
      const body = typeof options.body === 'string' && options.body ? JSON.parse(options.body) : {};

      switch (route) {
        case 'GET /api/picbed/types':
          return await invoke<T>('get_picbed_types');
        case 'GET /api/picbed/configs':
          return await invoke<T>('list_configs');
        case 'POST /api/picbed/configs':
          return await invoke<T>('create_config', { req: body });
        case 'POST /api/picbed/configs/test':
          return await invoke<T>('test_config_draft', { req: body });
        case 'GET /api/convert/records':
          return await invoke<T>('list_records');
        case 'GET /api/sync/github-client-id':
          return await invoke<T>('sync_github_client_id');
        case 'DELETE /api/convert/records':
          return await invoke<T>('delete_records', { req: body });
        case 'POST /api/convert/analyze':
          return await invoke<T>('analyze_markdown', { req: body });
        case 'POST /api/convert/tasks':
          return await invoke<T>('create_convert_task', { req: body });
        case 'POST /api/download/images':
          return await invoke<T>('download_remote_images', { urls: body.urls || [], targetDir: body.target_dir || '' });
        case 'GET /api/app/update-runtime':
          return await invoke<T>('get_update_runtime');
        case 'POST /api/app/update/download':
          return await invoke<T>('download_update', { url: body.url || '', sha256: body.sha256 || '' });
        case 'POST /api/app/update/apply':
          return await invoke<T>('apply_update', { filePath: body.file_path || '' });
        default:
          break;
      }

      const normalized = normalizePath(path);
      const configIdMatch = normalized.match(/^\/api\/picbed\/configs\/(\d+)(\/(default|test))?$/);
      if (configIdMatch) {
        const id = Number.parseInt(configIdMatch[1], 10);
        const action = configIdMatch[3] || '';
        if (method === 'PUT' && action === 'default') return await invoke<T>('set_default_config', { id });
        if (method === 'POST' && action === 'test') return await invoke<T>('test_config_saved', { id });
        if (!action && method === 'PUT') return await invoke<T>('update_config', { id, req: body });
        if (!action && method === 'DELETE') return await invoke<T>('delete_config', { id });
      }

      if (method === 'GET' && /^\/api\/convert\/tasks\/\d+$/.test(normalized)) {
        return await invoke<T>('get_convert_task', { id: parsePathId(normalized) });
      }
      if (method === 'GET' && /^\/api\/convert\/records\/\d+$/.test(normalized)) {
        return await invoke<T>('get_record', { id: parsePathId(normalized) });
      }

      throw new Error(`未支持的本地接口：${route}`);
    } catch (error) {
      throw toRequestError(error);
    }
  };
}
