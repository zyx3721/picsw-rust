import { getVersion } from '@tauri-apps/api/app';
import { Channel } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import { ref } from 'vue';

const RELEASE_PAGE_URL = 'https://github.com/zyx3721/picsw-rust/releases/latest';
const RELEASE_DOWNLOAD_BASE = 'https://github.com/zyx3721/picsw-rust/releases/download';
const AUTO_CHECK_DELAY_MS = 8000;
const AUTO_CHECK_INTERVAL_MS = 24 * 60 * 60 * 1000;
const UP_TO_DATE_RESET_MS = 5000;
const AUTO_CHECK_STORAGE_KEY = 'picbed_update_auto_check';
const LAST_CHECK_STORAGE_KEY = 'picbed_update_last_check_at';

export type UpdateStatus = 'idle' | 'checking' | 'available' | 'downloading' | 'downloaded';

type UpdateRuntime = { os: string; arch: string; portable: boolean; package: string };
type UpdateLatest = { tag: string; assets: string[] | null; notes: string; html_url: string; published_at: string };
type UpdateProgress = { downloaded: number; total: number };
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

/// 各平台安装包资产名的架构段与运行时 arch 的差异映射（与 CI 产物命名一一对应）
function setupArch(arch: string) {
  return arch === 'amd64' ? 'x64' : arch;
}
function dmgArch(arch: string) {
  return arch === 'amd64' ? 'x64' : arch === 'arm64' ? 'aarch64' : arch;
}
function rpmArch(arch: string) {
  return arch === 'amd64' ? 'x86_64' : arch;
}

/// 按平台、安装形态与系统包类型匹配 Release 资产（与 CI 产物命名一一对应）；
/// 清单缺失时按命名规律直拼下载地址；无匹配返回 null（点击时回退打开发布页）
function pickUpdateAsset(assets: string[] | null, version: string, runtime: UpdateRuntime): UpdateTask | null {
  const { os, arch, portable, package: pkg } = runtime;
  let assetName = '';
  if (os === 'windows') {
    assetName = portable
      ? `picbed-switcher_${version}_windows_${arch}.zip`
      : `PicBed.Switcher_${version}_${setupArch(arch)}-setup.exe`;
  } else if (os === 'macos') {
    assetName = portable
      ? `picbed-switcher_${version}_macos_${arch}.tar.gz`
      : `PicBed Switcher_${version}_${dmgArch(arch)}.dmg`;
  } else if (os === 'linux') {
    assetName = portable
      ? `picbed-switcher_${version}_linux_${arch}.tar.gz`
      : pkg === 'rpm'
        ? `picbed-switcher-${version}-1.${rpmArch(arch)}.rpm`
        : `PicBed Switcher_${version}_${arch}.deb`;
  } else {
    return null;
  }
  if (assets && !assets.includes(assetName)) return null;
  return {
    assetName,
    assetUrl: `${RELEASE_DOWNLOAD_BASE}/v${version}/${assetName}`,
    checksumUrl: `${RELEASE_DOWNLOAD_BASE}/v${version}/SHA256SUMS_${os}-${arch}.txt`,
  };
}

function autoCheckStorageEnabled() {
  return localStorage.getItem(AUTO_CHECK_STORAGE_KEY) !== '0';
}

/// 距上次检查超过 24 小时（或从未检查过）才发起启动静默检查，节制 GitHub 匿名接口限额
function shouldRunStartupCheck() {
  const last = Number(localStorage.getItem(LAST_CHECK_STORAGE_KEY)) || 0;
  return Date.now() - last >= AUTO_CHECK_INTERVAL_MS;
}

export function createUpdateChecker({ request, showMessage, showError }: UpdateCheckerDeps) {
  const updateStatus = ref<UpdateStatus>('idle');
  const updateVersion = ref('');
  const updateProgress = ref<UpdateProgress | null>(null);
  const updateNotes = ref('');
  const lastCheckAt = ref(Number(localStorage.getItem(LAST_CHECK_STORAGE_KEY)) || 0);
  const showUpToDate = ref(false);
  const autoCheckUpdate = ref(autoCheckStorageEnabled());
  let downloadedPath = '';
  let updateTask: UpdateTask | null = null;
  let upToDateTimer: number | undefined;

  function setAutoCheckUpdate(value: boolean) {
    autoCheckUpdate.value = value;
    localStorage.setItem(AUTO_CHECK_STORAGE_KEY, value ? '1' : '0');
  }

  /// 检查 GitHub 最新版本；manual 为手动点击（结果以状态胶囊反馈），启动自动检查静默进行
  async function checkUpdate(manual: boolean) {
    if (updateStatus.value === 'checking' || updateStatus.value === 'downloading') return;
    updateStatus.value = 'checking';
    try {
      const [current, runtime, latest] = await Promise.all([
        getVersion(),
        request<UpdateRuntime>('/api/app/update-runtime'),
        request<UpdateLatest>('/api/app/update-latest'),
      ]);
      lastCheckAt.value = Date.now();
      localStorage.setItem(LAST_CHECK_STORAGE_KEY, String(Date.now()));
      const version = latest.tag.replace(/^v/, '');
      if (!version || compareVersions(version, current) <= 0) {
        updateStatus.value = 'idle';
        updateVersion.value = '';
        updateTask = null;
        updateNotes.value = '';
        if (manual) {
          showUpToDate.value = true;
          if (upToDateTimer !== undefined) window.clearTimeout(upToDateTimer);
          upToDateTimer = window.setTimeout(() => {
            upToDateTimer = undefined;
            showUpToDate.value = false;
          }, UP_TO_DATE_RESET_MS);
        }
        return;
      }
      updateVersion.value = version;
      updateNotes.value = latest.notes;
      updateTask = pickUpdateAsset(latest.assets, version, runtime);
      updateStatus.value = 'available';
      if (manual) showMessage(`发现新版本 v${version}，可更新到最新版`);
    } catch (error) {
      updateStatus.value = 'idle';
      if (manual) showError(error instanceof Error ? error.message : '检查更新失败');
    }
  }

  /// 下载更新包（进度经 Channel 回传），完成后自动安装；失败回 available 可重试
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
    updateProgress.value = { downloaded: 0, total: 0 };
    try {
      const channel = new Channel<UpdateProgress>();
      channel.onmessage = progress => {
        updateProgress.value = progress;
      };
      const filePath = await request<string>('/api/app/update/download', {
        method: 'POST',
        body: JSON.stringify({ url: task.assetUrl, checksum_url: task.checksumUrl, asset_name: task.assetName }),
        channel,
      } as RequestInit);
      downloadedPath = filePath;
      updateStatus.value = 'downloaded';
      await applyUpdate();
    } catch (error) {
      updateStatus.value = updateVersion.value ? 'available' : 'idle';
      updateProgress.value = null;
      showError(error instanceof Error ? error.message : '更新失败');
    }
  }

  /// 应用已下载的更新包（downloaded 态可重试）：成功即进程退出由安装器接管，失败停留本态
  async function applyUpdate() {
    if (updateStatus.value !== 'downloaded' || !downloadedPath) return;
    try {
      await request<void>('/api/app/update/apply', {
        method: 'POST',
        body: JSON.stringify({ file_path: downloadedPath }),
      });
    } catch (error) {
      showError(error instanceof Error ? error.message : '应用更新失败，可重试或打开更新包手动安装');
    }
  }

  /// 打开已下载的更新包（自动安装失败的兜底出口；便携形态由后端返回手动替换指引）
  async function openUpdatePackage() {
    if (!downloadedPath) return;
    try {
      await request<void>('/api/app/update/open', {
        method: 'POST',
        body: JSON.stringify({ file_path: downloadedPath }),
      });
    } catch (error) {
      showError(error instanceof Error ? error.message : '打开更新包失败');
    }
  }

  setTimeout(() => {
    if (!autoCheckStorageEnabled() || !shouldRunStartupCheck()) return;
    void checkUpdate(false);
  }, AUTO_CHECK_DELAY_MS);

  return {
    updateStatus,
    updateVersion,
    updateProgress,
    updateNotes,
    lastCheckAt,
    showUpToDate,
    autoCheckUpdate,
    setAutoCheckUpdate,
    checkUpdate: () => checkUpdate(true),
    startUpdate,
    applyUpdate,
    openUpdatePackage,
  };
}

/// 把时间戳格式化为「X 分钟/小时/天前」的相对时间
export function relativeTimeText(timestamp: number) {
  const elapsed = Date.now() - timestamp;
  if (elapsed < 60 * 1000) return '刚刚';
  if (elapsed < 60 * 60 * 1000) return `${Math.floor(elapsed / (60 * 1000))} 分钟前`;
  if (elapsed < 24 * 60 * 60 * 1000) return `${Math.floor(elapsed / (60 * 60 * 1000))} 小时前`;
  return `${Math.floor(elapsed / (24 * 60 * 60 * 1000))} 天前`;
}
