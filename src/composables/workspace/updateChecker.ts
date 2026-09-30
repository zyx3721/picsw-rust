import { getVersion } from '@tauri-apps/api/app';
import { openUrl } from '@tauri-apps/plugin-opener';
import { ref } from 'vue';

const RELEASE_PAGE_URL = 'https://github.com/zyx3721/picsw-rust/releases/latest';
const RELEASE_DOWNLOAD_BASE = 'https://github.com/zyx3721/picsw-rust/releases/download';
const AUTO_CHECK_DELAY_MS = 1000;

export type UpdateStatus = 'idle' | 'checking' | 'available' | 'downloading';

type UpdateRuntime = { os: string; arch: string; portable: boolean };
type UpdateLatest = { tag: string; assets: string[] | null };

type UpdateTask = {
  assetName: string;
  assetUrl: string;
  checksumUrl: string;
};

type UpdateCheckerDeps = {
  request: <T>(path: string, options?: RequestInit) => Promise<T>;
  showMessage: (message: string) => void;
  showError: (message: string) => void;
};

/// 三段式版本号数字比较，返回正值表示 a 更新
function compareVersions(a: string, b: string): number {
  const pa = a.split('.').map(Number);
  const pb = b.split('.').map(Number);
  for (let i = 0; i < 3; i++) {
    const da = pa[i] || 0;
    const db = pb[i] || 0;
    if (da !== db) return da - db;
  }
  return 0;
}

/// 按平台与安装形态匹配 Release 资产，非 Windows 不做自动更新（返回 null 仅提示）；清单缺失时按命名规律直拼下载地址
function pickUpdateAsset(assets: string[] | null, version: string, runtime: UpdateRuntime): UpdateTask | null {
  if (runtime.os !== 'windows') return null;
  const setupTag = runtime.arch === 'amd64' ? 'x64' : runtime.arch;
  const assetName = runtime.portable
    ? `picbed-switcher_${version}_windows_${runtime.arch}.zip`
    : `PicBed.Switcher_${version}_${setupTag}-setup.exe`;
  if (assets && !assets.includes(assetName)) return null;
  return {
    assetName,
    assetUrl: `${RELEASE_DOWNLOAD_BASE}/v${version}/${assetName}`,
    checksumUrl: `${RELEASE_DOWNLOAD_BASE}/v${version}/SHA256SUMS_windows-${runtime.arch}.txt`,
  };
}

export function createUpdateChecker({ request, showMessage, showError }: UpdateCheckerDeps) {
  const updateStatus = ref<UpdateStatus>('idle');
  const updateVersion = ref('');
  let updateTask: UpdateTask | null = null;

  /// 检查 GitHub 最新版本；manual 为手动点击（结果以提示反馈），启动自动检查静默进行
  async function checkUpdate(manual: boolean) {
    if (updateStatus.value === 'checking' || updateStatus.value === 'downloading') return;
    updateStatus.value = 'checking';
    try {
      const [current, runtime, latest] = await Promise.all([
        getVersion(),
        request<UpdateRuntime>('/api/app/update-runtime'),
        request<UpdateLatest>('/api/app/update-latest'),
      ]);
      const version = latest.tag.replace(/^v/, '');
      if (!version || compareVersions(version, current) <= 0) {
        updateStatus.value = 'idle';
        updateVersion.value = '';
        updateTask = null;
        if (manual) showMessage('当前已是最新版');
        return;
      }
      updateVersion.value = version;
      updateTask = pickUpdateAsset(latest.assets, version, runtime);
      updateStatus.value = 'available';
    } catch (error) {
      updateStatus.value = 'idle';
      if (manual) showError(error instanceof Error ? error.message : '检查更新失败');
    }
  }

  /// 下载更新包并交由后端自替换安装；非 Windows 打开发布页手动下载
  async function startUpdate() {
    if (updateStatus.value !== 'available') return;
    const task = updateTask;
    if (!task) {
      updateStatus.value = 'idle';
      try {
        await openUrl(RELEASE_PAGE_URL);
      } catch (error) {
        showError(error instanceof Error ? error.message : '打开发布页失败');
      }
      return;
    }
    updateStatus.value = 'downloading';
    try {
      const filePath = await request<string>('/api/app/update/download', {
        method: 'POST',
        body: JSON.stringify({ url: task.assetUrl, checksum_url: task.checksumUrl, asset_name: task.assetName }),
      });
      await request<void>('/api/app/update/apply', {
        method: 'POST',
        body: JSON.stringify({ file_path: filePath }),
      });
    } catch (error) {
      updateStatus.value = 'available';
      showError(error instanceof Error ? error.message : '更新失败');
    }
  }

  setTimeout(() => {
    void checkUpdate(false);
  }, AUTO_CHECK_DELAY_MS);

  return {
    updateStatus,
    updateVersion,
    checkUpdate: () => checkUpdate(true),
    startUpdate,
  };
}
