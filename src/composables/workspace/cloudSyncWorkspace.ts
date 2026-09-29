import { computed, ref, watch, type Ref } from 'vue';
import { getVersion } from '@tauri-apps/api/app';
import { invoke } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import type { PicbedConfig, RemoteConfigPreview, SyncRemoteState } from './types';
import { decryptWithPasswordEmbedded, decryptWithPasswordParts, encryptWithPassword } from './syncCrypto';

type WorkspaceRequest = <T>(path: string, options?: RequestInit) => Promise<T>;

const GIST_DESCRIPTION = 'PicBed Switcher Encrypted Vault (DO NOT EDIT MANUALLY)';
const GIST_FILENAME = 'picbed-switcher-vault.json';
const TOKEN_STORAGE_KEY = 'picbed_sync_github_token';
const SECRET_BACKEND_STORAGE_KEY = 'picbed_sync_github_token_backend';
const DEVICE_ID_STORAGE_KEY = 'picbed_sync_device_id';
const LAST_SYNC_STORAGE_KEY = 'picbed_sync_last_sync';
const ACCOUNT_STORAGE_KEY = 'picbed_sync_github_account';
const GIST_ID_STORAGE_KEY = 'picbed_sync_gist_id';
const PASSWORD_HINT_STORAGE_KEY = 'picbed_sync_password_hint';
const AUTO_SYNC_STORAGE_KEY = 'picbed_sync_auto';
const BASE_STORAGE_KEY = 'picbed_sync_base';
const ANCHOR_STORAGE_KEY = 'picbed_sync_anchor';
const AUTO_SYNC_DEBOUNCE_MS = 3000;
const VISIBILITY_SYNC_THROTTLE_MS = 30_000;
const API_BASE = 'https://api.github.com';

export type SyncPasswordStatus = 'none' | 'locked' | 'unlocked';
export type DeviceFlowStatus = 'idle' | 'pending' | 'connected' | 'error';
export type SyncBannerKind = 'conflict' | 'blocked' | 'empty';
export type SyncBanner = {
  kind: SyncBannerKind;
  message: string;
  remoteItems: RemoteConfigPreview[];
  remoteSignature: string;
  remoteVersion: number;
  /** conflict：远端密文（供「输入云端密码」解密恢复） */
  remotePayload?: string;
  /** conflict：远端密文的 iv/salt（分立式结构必需） */
  remoteIv?: string;
  remoteSalt?: string;
  /** conflict：本机版本号 */
  localVersion?: number;
  /** conflict：远端设备名 */
  remoteDeviceName?: string;
};
export type SyncAnchor = { signature: string; version: number; updatedAt: number };
export type SyncRevisionPreview = {
  sha: string;
  version: number;
  updatedAt: number;
  updatedAtText: string;
  configCount: number;
  /** 预览生成时刻的本机条数（冻结展示，恢复执行期间不随本机重载跳变） */
  currentCount: number;
  deviceName: string;
  appVersion: string;
  configs: RemoteConfigPreview[];
};
export type SyncHistoryItem = {
  sha: string;
  date: number;
  dateText: string;
  isCurrent: boolean;
  label: string;
  versionBadge: string;
};
export type SyncStatusBadge = { text: string; kind: 'ok' | 'busy' | 'warn' | 'danger' | 'muted' };

function sleep(ms: number) {
  return new Promise(resolve => window.setTimeout(resolve, ms));
}

type CloudSyncWorkspaceDeps = {
  request: WorkspaceRequest;
  configs: Ref<PicbedConfig[]>;
  showMessage: (text: string) => void;
  showError: (text: string) => void;
  reloadConfigs: () => Promise<void>;
};

type GistFileEntry = { content?: string; truncated?: boolean };

function sha256HexSync(value: string) {
  // 16 进制前缀盐 + FNV 变体两轮混淆：仅用于本机「密码是否已设置/是否已解锁」的快速比对，
  // 真正的加密密钥由 Rust 侧 PBKDF2 派生，不依赖此哈希
  let h1 = 0x811c9dc5;
  let h2 = 0x01000193;
  const salted = `picbed-sync-hint:${value}`;
  for (let i = 0; i < salted.length; i += 1) {
    h1 = (h1 ^ salted.charCodeAt(i)) >>> 0;
    h1 = (Math.imul(h1, 16777619)) >>> 0;
    h2 = (h2 + Math.imul(h1 + i, 0x85ebca6b)) >>> 0;
  }
  return `${h1.toString(16).padStart(8, '0')}${h2.toString(16).padStart(8, '0')}${(h1 ^ h2).toString(16).padStart(8, '0')}`;
}

function ensureDeviceId() {
  let id = localStorage.getItem(DEVICE_ID_STORAGE_KEY);
  if (!id) {
    id = typeof crypto.randomUUID === 'function' ? crypto.randomUUID() : `device-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
    localStorage.setItem(DEVICE_ID_STORAGE_KEY, id);
  }
  return id;
}

function platformLabel() {
  const ua = navigator.userAgent;
  if (ua.includes('Windows')) return 'Win32';
  if (ua.includes('Mac OS')) return 'Darwin';
  if (ua.includes('Linux')) return 'Linux';
  return 'Unknown';
}

let cachedAppVersion = '';
async function ensureAppVersion() {
  if (!cachedAppVersion) {
    try {
      cachedAppVersion = await getVersion();
    } catch {
      cachedAppVersion = '';
    }
  }
  return cachedAppVersion;
}

function currentDeviceName() {
  return `PicBed Switcher (${platformLabel()})`;
}

function formatTime(ts: number) {
  const date = new Date(ts);
  const pad = (value: number) => String(value).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function formatRelativeTime(ts: number) {
  const diff = Date.now() - ts;
  if (diff < 60_000) return '刚刚';
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)} 分钟前`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)} 小时前`;
  if (diff < 172_800_000) return '昨天';
  const date = new Date(ts);
  return `${date.getFullYear()}/${date.getMonth() + 1}/${date.getDate()}`;
}

export function useWorkspaceCloudSync({
  request,
  configs,
  showMessage,
  showError,
  reloadConfigs,
}: CloudSyncWorkspaceDeps) {
  const syncAccount = ref(localStorage.getItem(ACCOUNT_STORAGE_KEY) || '');
  const syncPassword = ref('');
  const syncPatInput = ref('');
  const syncClientId = ref('');
  const syncRemoteState = ref<SyncRemoteState>(null);
  const syncBusy = ref(false);
  const passwordHintHash = ref(localStorage.getItem(PASSWORD_HINT_STORAGE_KEY) || '');
  const passwordModalOpen = ref(false);
  const passwordModalMode = ref<'set' | 'change'>('set');
  const githubConnectOpen = ref(false);
  const githubConnectPreparing = ref(false);
  const deviceFlowUserCode = ref('');
  const deviceFlowStatus = ref<DeviceFlowStatus>('idle');
  const deviceFlowError = ref('');
  const patMode = ref(false);
  const autoSyncEnabled = ref(localStorage.getItem(AUTO_SYNC_STORAGE_KEY) !== '0');
  const syncBanner = ref<SyncBanner | null>(null);
  const secretBackend = ref(localStorage.getItem(SECRET_BACKEND_STORAGE_KEY) || '');
  const initialLastSync = (() => {
    try {
      return JSON.parse(localStorage.getItem(LAST_SYNC_STORAGE_KEY) || '') as { at: number; version: number };
    } catch {
      return null;
    }
  })();
  const lastSyncAt = ref(initialLastSync?.at || 0);
  const lastSyncVersion = ref(initialLastSync?.version || 0);
  const syncHistoryOpen = ref(false);
  const syncHistoryLoading = ref(false);
  const syncHistory = ref<SyncHistoryItem[]>([]);
  const selectedRevisionSha = ref('');
  const revisionPreview = ref<SyncRevisionPreview | null>(null);
  const revisionPreviewLoading = ref(false);
  const revisionError = ref('');
  const restoringRevision = ref(false);
  let cachedToken = '';
  let deviceFlowSession = 0;
  let autoSyncTimer: number | undefined;
  let lastVisibleSyncAt = 0;

  const canAutoSync = computed(
    () =>
      autoSyncEnabled.value &&
      syncConnected.value &&
      passwordStatus.value === 'unlocked' &&
      !syncBusy.value &&
      !syncBanner.value
  );
  const autoSyncToggleEnabled = computed(
    () => syncConnected.value && passwordStatus.value === 'unlocked'
  );
  const autoSyncToggleTitle = computed(() =>
    autoSyncToggleEnabled.value ? '' : '需先连接 GitHub 并解锁同步密码'
  );
  /// 开关始终展示实际设置：未连接/未解锁时仅禁用点击，勾选状态不受影响
  const autoSyncDisplayChecked = computed(() => autoSyncEnabled.value);
  /// 3 秒后自动执行一轮完整同步（重新启用自动同步 / 重新连接时触发）
  function scheduleAutoSyncRound() {
    if (autoSyncTimer !== undefined) window.clearTimeout(autoSyncTimer);
    autoSyncTimer = window.setTimeout(() => {
      autoSyncTimer = undefined;
      if (canAutoSync.value) void syncNow();
    }, AUTO_SYNC_DEBOUNCE_MS);
  }

  /// 开关联动：停用即取消待传定时；重新启用后 3 秒自动执行一轮完整同步
  function setAutoSyncEnabled(value: boolean) {
    autoSyncEnabled.value = value;
    if (value) {
      scheduleAutoSyncRound();
      return;
    }
    if (autoSyncTimer !== undefined) {
      window.clearTimeout(autoSyncTimer);
      autoSyncTimer = undefined;
    }
  }

  /// 从 Rust 侧运行时解析 GitHub OAuth App client_id（环境变量 / .env / 构建期注入）
  async function refreshSyncClientId() {
    if (syncClientId.value) return;
    try {
      const clientId = await request<string | null>('/api/sync/github-client-id');
      syncClientId.value = clientId || '';
    } catch {
      syncClientId.value = '';
    }
  }
  void refreshSyncClientId();

  function snapshotConfigs() {
    return fingerprintOf(configs.value.map(syncItemOf));
  }

  /// base 与锚点均持久化到 localStorage：base 先写、锚点后写（顺序不可换，防中途崩溃不一致）
  function loadBaseMap(): Map<string, RemoteConfigPreview> {
    const raw = localStorage.getItem(BASE_STORAGE_KEY);
    if (!raw) return new Map();
    try {
      const parsed = JSON.parse(raw) as RemoteConfigPreview[];
      return new Map(parsed.filter(item => item && item.config_name).map(item => [item.config_name, item]));
    } catch {
      return new Map();
    }
  }

  function saveBase(items: RemoteConfigPreview[]) {
    localStorage.setItem(BASE_STORAGE_KEY, JSON.stringify(items));
  }

  function loadAnchor(): SyncAnchor | null {
    const raw = localStorage.getItem(ANCHOR_STORAGE_KEY);
    if (!raw) return null;
    try {
      return JSON.parse(raw) as SyncAnchor;
    } catch {
      return null;
    }
  }

  function saveAnchor(anchor: SyncAnchor) {
    localStorage.setItem(ANCHOR_STORAGE_KEY, JSON.stringify(anchor));
  }

  async function sha256Hex(value: string) {
    const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(value));
    return Array.from(new Uint8Array(digest))
      .map(byte => byte.toString(16).padStart(2, '0'))
      .join('');
  }

  /// 提交「上次同步」时间与版本：持久化保存，重启后状态行不丢
  async function commitLastSync(version: number) {
    lastSyncAt.value = Date.now();
    lastSyncVersion.value = version;
    localStorage.setItem(LAST_SYNC_STORAGE_KEY, JSON.stringify({ at: lastSyncAt.value, version }));
  }

  function scheduleAutoUpload() {
    if (autoSyncTimer !== undefined) window.clearTimeout(autoSyncTimer);
    autoSyncTimer = window.setTimeout(() => {
      autoSyncTimer = undefined;
      if (!canAutoSync.value) return;
      const baseFingerprint = fingerprintOf([...loadBaseMap().values()]);
      if (snapshotConfigs() === baseFingerprint) return;
      void syncNow();
    }, AUTO_SYNC_DEBOUNCE_MS);
  }

  watch(configs, () => {
    // 首次加载配置作为 base 基线，不触发自动上传，避免启动即覆盖云端
    if (!loadBaseMap().size) {
      saveBase(configs.value.map(syncItemOf));
      return;
    }
    if (!canAutoSync.value) return;
    scheduleAutoUpload();
  }, { deep: true });

  watch(autoSyncEnabled, value => localStorage.setItem(AUTO_SYNC_STORAGE_KEY, value ? '1' : '0'));

  if (typeof document !== 'undefined') {
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState !== 'visible' || !canAutoSync.value) return;
      const now = Date.now();
      if (now - lastVisibleSyncAt < VISIBILITY_SYNC_THROTTLE_MS) return;
      lastVisibleSyncAt = now;
      void syncNow(true);
    });
  }

  const syncConnected = computed(() => syncAccount.value.length > 0);
  const localConfigCount = computed(() => configs.value.length);
  /// 同步状态徽章：同步中 / 冲突 / 阻塞 / 待恢复 / 从未同步 / 就绪
  const syncStatusBadge = computed<SyncStatusBadge>(() => {
    if (syncBusy.value) return { text: '同步中', kind: 'busy' };
    if (syncBanner.value?.kind === 'conflict') return { text: '冲突', kind: 'danger' };
    if (syncBanner.value?.kind === 'blocked') return { text: '已拦截', kind: 'danger' };
    if (syncBanner.value?.kind === 'empty') return { text: '待恢复', kind: 'warn' };
    if (!syncRemoteState.value) return { text: '从未同步', kind: 'muted' };
    return { text: '就绪', kind: 'ok' };
  });
  const remoteVersionBadge = computed(() => {
    const version = syncRemoteState.value?.version || 0;
    return version > 0 ? `v${version}` : '';
  });
  const lastSyncText = computed(() => {
    const at = lastSyncAt.value;
    return at > 0 ? `上次同步: ${formatRelativeTime(at)}` : '上次同步: 从未同步';
  });
  const passwordStatus = computed<SyncPasswordStatus>(() => {
    if (!passwordHintHash.value) return 'none';
    if (syncPassword.value && sha256HexSync(syncPassword.value) === passwordHintHash.value) return 'unlocked';
    return 'locked';
  });
  const canSyncNow = computed(
    () => syncConnected.value && passwordStatus.value === 'unlocked' && !syncBusy.value
  );

  function unauthorized() {
    return !cachedToken;
  }

  /// WebView fetch 无自动重试：直连 api.github.com 存在间歇性超时（尤其国内网络），
  /// 网络类失败自动重试，间隔 1s / 2s。仅对「请求没发出去/中途断开」重试，
  /// HTTP 错误响应（401/404 等）原样抛出交给调用方处理。
  async function fetchWithRetry(path: string, init: RequestInit): Promise<Response> {
    let lastError: unknown = null;
    for (let attempt = 0; attempt < 3; attempt += 1) {
      if (attempt > 0) await sleep(1000 * attempt);
      try {
        return await fetch(`${API_BASE}${path}`, init);
      } catch (error) {
        lastError = error;
      }
    }
    const detail = lastError instanceof Error ? lastError.message : String(lastError || '未知错误');
    throw new Error(
      `GitHub API 请求失败: 网络异常（已自动重试 3 次）: ${detail}。可稍后重试；若本机需要代理访问 GitHub，请设置 HTTPS_PROXY 环境变量后重启应用`
    );
  }

  async function githubFetch(path: string, init: RequestInit = {}) {
    const response = await fetchWithRetry(path, {
      ...init,
      headers: {
        Authorization: `Bearer ${cachedToken}`,
        Accept: 'application/vnd.github+json',
        'X-GitHub-Api-Version': '2022-11-28',
        'Content-Type': 'application/json',
        ...(init.headers || {}),
      },
    });
    if (!response.ok) {
      const detail = await response.text().catch(() => '');
      throw new Error(`GitHub API 请求失败（${response.status}）：${detail.slice(0, 160)}`);
    }
    if (response.status === 204) return null;
    return response.json();
  }

  async function findSyncGist() {
    const gists = (await githubFetch('/gists?per_page=100')) as Array<{
      id: string;
      description?: string;
      files: Record<string, GistFileEntry>;
    }>;
    const found = gists.find(gist => gist.description === GIST_DESCRIPTION && gist.files[GIST_FILENAME]);
    if (!found) return null;
    // 列表接口的 files[].content 可能为空（GitHub 行为不稳定）：统一走单 Gist 接口拉完整内容
    const full = (await githubFetch(`/gists/${found.id}`)) as { files: Record<string, GistFileEntry> };
    const file = full.files[GIST_FILENAME];
    let content = file?.content || '';
    if (!content || file.truncated) {
      throw new Error('云端数据过大且被截断，暂不支持拉取');
    }
    return { id: found.id, content };
  }

  function parseVault(content: string) {
    let vault: {
      meta?: {
        version?: number;
        updatedAt?: number;
        deviceName?: string;
        appVersion?: string;
        iv?: string;
        salt?: string;
      };
      version?: number;
      updatedAt?: number;
      payload?: string;
    };
    try {
      vault = JSON.parse(content);
    } catch {
      throw new Error('云端数据格式不正确');
    }
    if (!vault || typeof vault.payload !== 'string') {
      throw new Error('云端数据格式不正确');
    }
    // 新结构信息在 meta 内（iv/salt 分立），旧结构字段在顶层且密文内嵌，两者兼容读取
    const meta = vault.meta || {};
    return {
      version: typeof meta.version === 'number' ? meta.version : typeof vault.version === 'number' ? vault.version : 0,
      updatedAt:
        typeof meta.updatedAt === 'number' ? meta.updatedAt : typeof vault.updatedAt === 'number' ? vault.updatedAt : 0,
      deviceName: typeof meta.deviceName === 'string' ? meta.deviceName : '',
      appVersion: typeof meta.appVersion === 'string' ? meta.appVersion : '',
      iv: typeof meta.iv === 'string' ? meta.iv : '',
      salt: typeof meta.salt === 'string' ? meta.salt : '',
      payload: vault.payload,
    };
  }

  async function connectGithub() {
    const pat = syncPatInput.value.trim();
    if (!pat) {
      showError('请先粘贴 GitHub 个人访问令牌（需要 gist 权限）');
      return;
    }
    loadingStart();
    try {
      await completeConnection(pat);
      syncPatInput.value = '';
      patMode.value = false;
      githubConnectOpen.value = false;
    } catch (error) {
      cachedToken = '';
      showError(error instanceof Error ? error.message : '连接 GitHub 失败');
    } finally {
      loadingEnd();
    }
  }

  async function completeConnection(token: string) {
    cachedToken = token;
    const user = (await githubFetch('/user')) as { login?: string; name?: string | null };
    // 优先展示 GitHub 资料名（Name），未设置时回退用户名（login）
    const displayName = (user.name || '').trim() || user.login || '未知账号';
    const backend = await invoke<string>('sync_save_credential', { plaintext: token });
    localStorage.setItem(SECRET_BACKEND_STORAGE_KEY, backend);
    localStorage.removeItem(TOKEN_STORAGE_KEY);
    localStorage.setItem(ACCOUNT_STORAGE_KEY, displayName);
    secretBackend.value = backend;
    syncAccount.value = displayName;
    showMessage(`已连接 GitHub 账号 ${displayName}`);
    await refreshRemoteState();
    if (passwordStatus.value === 'unlocked' && autoSyncEnabled.value) scheduleAutoSyncRound();
  }

  /// 打开连接弹窗：先取回设备码再弹窗（按钮呈加载态），无 client_id 时回退令牌粘贴模式
  async function openGithubConnect() {
    if (githubConnectPreparing.value) return;
    githubConnectPreparing.value = true;
    deviceFlowUserCode.value = '';
    deviceFlowStatus.value = 'idle';
    deviceFlowError.value = '';
    patMode.value = false;
    try {
      await refreshSyncClientId();
      if (!syncClientId.value) {
        githubConnectOpen.value = true;
        return;
      }
      const session = ++deviceFlowSession;
      const start = await invoke<{
        device_code: string;
        user_code: string;
        verification_uri: string;
        expires_at: number;
        interval: number;
      }>('github_device_flow_start', { clientId: syncClientId.value, scope: 'gist read:user' });
      if (session !== deviceFlowSession) return;
      deviceFlowUserCode.value = start.user_code.toUpperCase();
      deviceFlowStatus.value = 'pending';
      githubConnectOpen.value = true;
      void pollGithubDeviceFlow(session, start);
    } catch (error) {
      deviceFlowStatus.value = 'error';
      deviceFlowError.value = error instanceof Error ? error.message : String(error);
      githubConnectOpen.value = true;
    } finally {
      githubConnectPreparing.value = false;
    }
  }

  /// 设备码轮询：授权完成自动连接，过期/拒绝置错误态，单次网络失败按 pending 续等
  async function pollGithubDeviceFlow(
    session: number,
    start: { device_code: string; expires_at: number; interval: number },
  ) {
    let intervalMs = Math.max(start.interval, 5) * 1000;
    while (session === deviceFlowSession && Date.now() < start.expires_at) {
      await sleep(intervalMs);
      if (session !== deviceFlowSession) return;
      let result: Record<string, unknown> | null = null;
      try {
        result = await invoke<Record<string, unknown>>('github_device_flow_poll', {
          clientId: syncClientId.value,
          deviceCode: start.device_code,
        });
      } catch {
        result = null;
      }
      if (!result) {
        if (session !== deviceFlowSession) return;
        continue;
      }
      const flowError = typeof result.error === 'string' ? result.error : '';
      if (flowError === 'authorization_pending') continue;
      if (flowError === 'slow_down') {
        intervalMs += 5000;
        continue;
      }
      if (flowError === 'expired_token') {
        deviceFlowStatus.value = 'error';
        deviceFlowError.value = '设备码已过期，请重新发起连接';
        return;
      }
      if (flowError === 'access_denied') {
        deviceFlowStatus.value = 'error';
        deviceFlowError.value = '授权被拒绝';
        return;
      }
      if (typeof result.access_token === 'string') {
        await completeConnection(result.access_token);
        if (session !== deviceFlowSession) return;
        deviceFlowStatus.value = 'connected';
        githubConnectOpen.value = false;
        return;
      }
    }
    if (session === deviceFlowSession && deviceFlowStatus.value === 'pending') {
      deviceFlowStatus.value = 'error';
      deviceFlowError.value = '连接已超时，请重新发起';
    }
  }

  function cancelGithubConnect() {
    deviceFlowSession += 1;
    githubConnectOpen.value = false;
    deviceFlowStatus.value = 'idle';
    deviceFlowUserCode.value = '';
    deviceFlowError.value = '';
    patMode.value = false;
  }

  async function openVerificationPage() {
    try {
      await openUrl('https://github.com/login/device');
    } catch (error) {
      showError(error instanceof Error ? error.message : '打开授权页失败，请手动访问 github.com/login/device');
    }
  }

  async function copyUserCode() {
    try {
      await navigator.clipboard.writeText(deviceFlowUserCode.value);
      showMessage('设备码已复制');
    } catch {
      showError('复制失败，请手动选择设备码复制');
    }
  }

  async function disconnectGithub() {
    cachedToken = '';
    const backend = localStorage.getItem(SECRET_BACKEND_STORAGE_KEY);
    if (backend) {
      try {
        await invoke('sync_delete_credential', { backend });
      } catch {
        // 清理尽力而为：钥匙串删除失败不阻断断开流程
      }
    }
    localStorage.removeItem(SECRET_BACKEND_STORAGE_KEY);
    localStorage.removeItem(TOKEN_STORAGE_KEY);
    localStorage.removeItem(ACCOUNT_STORAGE_KEY);
    localStorage.removeItem(GIST_ID_STORAGE_KEY);
    // 同步快照（base/锚点）一并清除，下次连接从干净状态重新同步
    localStorage.removeItem(BASE_STORAGE_KEY);
    localStorage.removeItem(ANCHOR_STORAGE_KEY);
    secretBackend.value = '';
    syncAccount.value = '';
    syncRemoteState.value = null;
    lastSyncAt.value = 0;
    lastSyncVersion.value = 0;
    localStorage.removeItem(LAST_SYNC_STORAGE_KEY);
    showMessage('已断开 GitHub 连接');
  }

  function openPasswordModal(mode: 'set' | 'change') {
    passwordModalMode.value = mode;
    passwordModalOpen.value = true;
  }

  function closePasswordModal() {
    passwordModalOpen.value = false;
  }

  /// 行内解锁：锁定态直接在同步密码行输入解锁，不再弹窗；
  /// 校验通过后仅在内存持有密码，并立即同步一次对齐云端
  function unlockWithPassword(password: string): string | null {
    if (sha256HexSync(password) !== passwordHintHash.value) {
      return '同步密码不正确';
    }
    syncPassword.value = password;
    if (syncConnected.value) void syncNow();
    return null;
  }

  /// 弹窗提交：set 为首次设置（成功后自动同步一次）；change 为修改密码，
  /// 需先验证当前密码，修改只影响之后的上传故不触发同步
  async function submitPasswordModal(
    mode: 'set' | 'change',
    oldPassword: string,
    password: string,
  ): Promise<string | null> {
    if (mode === 'change' && sha256HexSync(oldPassword) !== passwordHintHash.value) {
      return '当前密码不正确';
    }
    syncPassword.value = password;
    passwordHintHash.value = sha256HexSync(password);
    localStorage.setItem(PASSWORD_HINT_STORAGE_KEY, passwordHintHash.value);
    passwordModalOpen.value = false;
    if (mode === 'set') {
      if (syncConnected.value) void syncNow();
      return null;
    }
    showMessage('同步密码已修改，云端数据将在下次配置变更同步时将以新密码加密');
    return null;
  }

  async function refreshRemoteState() {
    if (unauthorized()) return;
    loadingStart();
    try {
      const gist = await findSyncGist();
      if (!gist) {
        syncRemoteState.value = null;
        localStorage.removeItem(GIST_ID_STORAGE_KEY);
        return;
      }
      localStorage.setItem(GIST_ID_STORAGE_KEY, gist.id);
      const vault = parseVault(gist.content);
      syncRemoteState.value = {
        version: vault.version,
        updatedAt: vault.updatedAt,
        configCount: -1,
      };
    } catch {
      syncRemoteState.value = null;
    } finally {
      loadingEnd();
    }
  }

  async function restoreConnection() {
    const backend = localStorage.getItem(SECRET_BACKEND_STORAGE_KEY);
    const legacyCiphertext = localStorage.getItem(TOKEN_STORAGE_KEY);
    if (!backend && !legacyCiphertext) return;
    try {
      let token = '';
      if (backend) {
        token = (await invoke<string | null>('sync_load_credential', { backend })) || '';
        secretBackend.value = backend;
      } else if (legacyCiphertext) {
        // 旧版本遗留在 WebView 本地存储的固定种子密文：迁移到系统钥匙串后清除
        token = await invoke<string>('sync_load_legacy_credential', { ciphertext: legacyCiphertext });
        const savedBackend = await invoke<string>('sync_save_credential', { plaintext: token });
        localStorage.setItem(SECRET_BACKEND_STORAGE_KEY, savedBackend);
        localStorage.removeItem(TOKEN_STORAGE_KEY);
        secretBackend.value = savedBackend;
      }
      if (!token) {
        localStorage.removeItem(SECRET_BACKEND_STORAGE_KEY);
        localStorage.removeItem(TOKEN_STORAGE_KEY);
        return;
      }
      cachedToken = token;
      const user = (await githubFetch('/user')) as { login?: string; name?: string | null };
      syncAccount.value = (user.name || '').trim() || user.login || '已连接';
      await refreshRemoteState();
    } catch {
      cachedToken = '';
      localStorage.removeItem(SECRET_BACKEND_STORAGE_KEY);
      localStorage.removeItem(TOKEN_STORAGE_KEY);
    }
  }

  function fingerprintOf(items: RemoteConfigPreview[]) {
    // 与配置顺序无关的内容指纹：按配置名排序后序列化
    return JSON.stringify(
      items
        .map(item => [item.picbed_type, item.config_name, item.is_default, item.config])
        .sort((a, b) => String(a[1]).localeCompare(String(b[1])))
    );
  }

  function syncItemOf(config: PicbedConfig): RemoteConfigPreview {
    return {
      picbed_type: config.picbed_type,
      config_name: config.config_name,
      is_default: config.is_default,
      config: (config.config || {}) as Record<string, string>,
    };
  }

  /// 兼容两种密文形态：meta 分立 iv/salt（新）与内嵌编码（旧）
  async function decryptVaultConfigs(vault: ReturnType<typeof parseVault>, password: string): Promise<RemoteConfigPreview[]> {
    const plaintext = vault.iv && vault.salt
      ? await decryptWithPasswordParts(vault.payload, password, vault.iv, vault.salt)
      : await decryptWithPasswordEmbedded(vault.payload, password);
    const parsed = JSON.parse(plaintext) as { configs?: RemoteConfigPreview[] };
    return parsed.configs || [];
  }

  async function decryptConfigsWith(
    payload: string,
    password: string,
    iv?: string,
    salt?: string,
  ): Promise<RemoteConfigPreview[]> {
    const plaintext = iv && salt
      ? await decryptWithPasswordParts(payload, password, iv, salt)
      : await decryptWithPasswordEmbedded(payload, password);
    const parsed = JSON.parse(plaintext) as { configs?: RemoteConfigPreview[] };
    return parsed.configs || [];
  }

  async function uploadPayload(items: RemoteConfigPreview[], version: number): Promise<string> {
    const payload = JSON.stringify({ configs: items, syncedAt: Date.now() });
    // 分立式加密：iv/salt 提升到 meta 字段（对照上游 vault 结构），payload 仅存密文
    const sealed = await encryptWithPassword(payload, syncPassword.value);
    await ensureAppVersion();
    const vault = JSON.stringify(
      {
        meta: {
          version,
          updatedAt: Date.now(),
          deviceId: ensureDeviceId(),
          deviceName: currentDeviceName(),
          appVersion: cachedAppVersion,
          iv: sealed.iv,
          salt: sealed.salt,
          algorithm: 'AES-256-GCM',
          kdf: 'PBKDF2',
          kdfIterations: 600000,
        },
        payload: sealed.payload,
      },
      null,
      2,
    );
    const gistId = localStorage.getItem(GIST_ID_STORAGE_KEY);
    const body = {
      description: GIST_DESCRIPTION,
      public: false,
      files: { [GIST_FILENAME]: { content: vault } },
    };
    if (gistId) {
      await githubFetch(`/gists/${gistId}`, { method: 'PATCH', body: JSON.stringify(body) });
    } else {
      const created = (await githubFetch('/gists', { method: 'POST', body: JSON.stringify(body) })) as { id?: string };
      if (created.id) localStorage.setItem(GIST_ID_STORAGE_KEY, created.id);
    }
    return vault;
  }

  /// syncNow 完整决策树：
  /// 锚点判等（no-op / 仅上传）→ 解密冲突横幅 → 内容一致采纳远端 → 空库保护 →
  /// 三方合并（含删除传播）→ 收缩护栏（BLOCKED 横幅）→ 应用并上传合并结果。
  /// base 与本地指纹均按 config_name 排序归一化：本机列表按 updated_at 排序，若直接
  /// 按存储顺序序列化，会因顺序不同把「内容一致」误判为「本地有改动」而多推版本。
  async function syncNow(quiet = false) {
    if (!canSyncNow.value) return;
    loadingStart();
    try {
      const gist = await findSyncGist();
      const localItems = configs.value.map(syncItemOf);
      const localFingerprint = fingerprintOf(localItems);
      const baseMap = loadBaseMap();
      const baseFingerprint = fingerprintOf([...baseMap.values()]);
      const anchor = loadAnchor();

      // 2b. 远端无文件（首次或 Gist 在云端被删）→ 清掉失效记录后上传重建
      if (!gist) {
        localStorage.removeItem(GIST_ID_STORAGE_KEY);
        if (localItems.length === 0) {
          if (!quiet) showMessage('本地与云端都没有图床配置，无需同步');
          return;
        }
        const vault = await uploadPayload(localItems, 1);
        saveBase(localItems);
        await saveAnchor({ signature: await sha256Hex(vault), version: 1, updatedAt: Date.now() });
        await commitLastSync(1);
        syncRemoteState.value = { version: 1, updatedAt: Date.now(), configCount: localItems.length };
        showMessage(`云端为空，已上传本地 ${localItems.length} 个图床配置（版本 1）`);
        return;
      }

      localStorage.setItem(GIST_ID_STORAGE_KEY, gist.id);
      const vault = parseVault(gist.content);
      const signature = await sha256Hex(gist.content);
      // 锚点判等：签名一致说明远端未被其它设备修改
      const remoteChanged = !anchor || anchor.signature !== signature;

      // 3a. 远端未变
      if (!remoteChanged) {
        if (localFingerprint === baseFingerprint) {
          // 双端一致 → no-op（补写可能缺失的锚点，防御本地存储被清）；一致性检查成功同样刷新上次同步
          if (!anchor) await saveAnchor({ signature, version: vault.version, updatedAt: vault.updatedAt });
          await commitLastSync(vault.version);
          syncRemoteState.value = { version: vault.version, updatedAt: Date.now(), configCount: localItems.length };
          if (!quiet) showMessage('本地与云端已一致，无需同步');
          return;
        }
        const version = vault.version + 1;
        const vaultContent = await uploadPayload(localItems, version);
        saveBase(localItems);
        await saveAnchor({ signature: await sha256Hex(vaultContent), version, updatedAt: Date.now() });
        await commitLastSync(version);
        syncRemoteState.value = { version, updatedAt: Date.now(), configCount: localItems.length };
        showMessage(`已上传本地 ${localItems.length} 个图床配置（版本 ${version}）`);
        return;
      }

      // 3c. 远端已变 → 解密；密码不同则进入冲突横幅
      let remoteItems: RemoteConfigPreview[];
      try {
        remoteItems = await decryptVaultConfigs(vault, syncPassword.value);
      } catch (decryptError) {
        const message = decryptError instanceof Error ? decryptError.message : String(decryptError);
        if (message.includes('同步密码不正确')) {
          syncBanner.value = {
            kind: 'conflict',
            message: '云端数据无法用当前同步密码解密',
            remoteItems: [],
            remoteSignature: signature,
            remoteVersion: vault.version,
            remotePayload: vault.payload,
            remoteIv: vault.iv || undefined,
            remoteSalt: vault.salt || undefined,
            localVersion: loadAnchor()?.version || 0,
            remoteDeviceName: vault.deviceName,
          };
          showMessage('检测到密码冲突，请在横幅中选择处理方式');
          return;
        }
        throw decryptError;
      }
      const remoteFingerprint = fingerprintOf(remoteItems);

      // 内容一致 → 采纳远端（本地对齐 + base/锚点提交），不浪费上传
      if (remoteFingerprint === localFingerprint) {
        saveBase(remoteItems);
        await saveAnchor({ signature, version: vault.version, updatedAt: vault.updatedAt });
        await commitLastSync(vault.version);
        syncRemoteState.value = { version: vault.version, updatedAt: vault.updatedAt, configCount: localItems.length };
        if (!quiet) showMessage('本地与云端配置已一致');
        return;
      }

      // 空库保护：无 base + 本地空 + 远端有数据 → 等用户确认恢复
      if (baseMap.size === 0 && localItems.length === 0 && remoteItems.length > 0) {
        syncBanner.value = {
          kind: 'empty',
          message: `本地没有图床配置而云端有 ${remoteItems.length} 个，恢复后将写入本机`,
          remoteItems,
          remoteSignature: signature,
          remoteVersion: vault.version,
        };
        showMessage('本地没有配置而云端有数据，确认后可从云端恢复');
        return;
      }

      // 三方合并（含删除传播；双改冲突以本地为准并计数）
      const localMap = new Map(localItems.map(item => [item.config_name, item]));
      const remoteMap = new Map(remoteItems.map(item => [item.config_name, item]));
      const merged: RemoteConfigPreview[] = [];
      const remoteChanges: RemoteConfigPreview[] = [];
      const localDeletions: RemoteConfigPreview[] = [];
      let conflicts = 0;

      for (const name of new Set([...localMap.keys(), ...remoteMap.keys()])) {
        const local = localMap.get(name);
        const remote = remoteMap.get(name);
        const base = baseMap.get(name);
        if (local && !remote) {
          if (base && fingerprintOf([local]) === fingerprintOf([base])) {
            // 远端已删除且本地未修改 → 删除传播
            localDeletions.push(local);
          } else {
            merged.push(local);
          }
          continue;
        }
        if (!local && remote) {
          if (base && fingerprintOf([remote]) === fingerprintOf([base])) {
            // 本地已删除且远端未修改 → 保持删除
            continue;
          }
          merged.push(remote);
          remoteChanges.push(remote);
          continue;
        }
        const localChanged = !base || fingerprintOf([local!]) !== fingerprintOf([base!]);
        const remoteChanged = !base || fingerprintOf([remote!]) !== fingerprintOf([base!]);
        if (localChanged && remoteChanged) {
          if (fingerprintOf([local!]) !== fingerprintOf([remote!])) conflicts += 1;
          merged.push(local!);
          continue;
        }
        if (remoteChanged) {
          merged.push(remote!);
          if (fingerprintOf([local!]) !== fingerprintOf([remote!])) remoteChanges.push(remote!);
          continue;
        }
        merged.push(local!);
      }

      // 收缩护栏：合并结果比基准少 ≥30% 且 ≥2 个，或直接少 ≥10 个 → BLOCKED 横幅
      const referenceCount = baseMap.size || remoteItems.length;
      const lost = referenceCount - merged.length;
      if (lost >= 10 || (lost >= 2 && lost >= Math.ceil(referenceCount * 0.3))) {
        syncBanner.value = {
          kind: 'blocked',
          message: `同步被拦截：合并结果比基准少 ${lost} 个配置，可能存在数据丢失`,
          remoteItems,
          remoteSignature: signature,
          remoteVersion: vault.version,
        };
        showMessage('同步被拦截，请在横幅中选择恢复远端版本或强制推送');
        return;
      }

      // 应用远端变更与删除传播
      let applied = 0;
      let deleted = 0;
      for (const item of remoteChanges) {
        const body = JSON.stringify({
          picbed_type: item.picbed_type,
          config_name: item.config_name,
          is_default: item.is_default,
          config: item.config,
        });
        const existing = configs.value.find(config => config.config_name === item.config_name);
        if (existing) {
          await request(`/api/picbed/configs/${existing.id}`, { method: 'PUT', body });
        } else {
          await request('/api/picbed/configs', { method: 'POST', body });
        }
        applied += 1;
      }
      for (const item of localDeletions) {
        const existing = configs.value.find(config => config.config_name === item.config_name);
        if (existing) {
          await request(`/api/picbed/configs/${existing.id}`, { method: 'DELETE' });
          deleted += 1;
        }
      }
      if (applied > 0 || deleted > 0) await reloadConfigs();

      // 合并结果与远端一致（本地无独有贡献）→ 采纳远端，不浪费上传
      if (fingerprintOf(merged) === remoteFingerprint) {
        saveBase(remoteItems);
        await saveAnchor({ signature, version: vault.version, updatedAt: vault.updatedAt });
        await commitLastSync(vault.version);
        syncRemoteState.value = { version: vault.version, updatedAt: vault.updatedAt, configCount: merged.length };
        const parts = [`应用云端变更 ${applied} 项`];
        if (deleted > 0) parts.push(`删除 ${deleted} 项`);
        showMessage(`已采纳云端配置（${parts.join('，')}）`);
        return;
      }

      // 上传合并结果（version = 远端 + 1），提交 base 与锚点
      const version = vault.version + 1;
      const vaultContent = await uploadPayload(merged, version);
      saveBase(merged);
      await saveAnchor({ signature: await sha256Hex(vaultContent), version, updatedAt: vault.updatedAt });
      await commitLastSync(version);
      syncRemoteState.value = { version, updatedAt: Date.now(), configCount: merged.length };

      const parts = [`应用云端变更 ${applied} 项`];
      if (deleted > 0) parts.push(`删除本地 ${deleted} 项`);
      const message = `同步完成：${parts.join('，')}，上传合并结果 ${merged.length} 个配置（版本 ${version}）${conflicts > 0 ? `；${conflicts} 处两侧均修改，以本地为准` : ''}`;
      showMessage(message);
    } catch (error) {
      showError(error instanceof Error ? error.message : '立即同步失败');
    } finally {
      loadingEnd();
    }
  }

  /// 冲突横幅：用当前同步密码以本机配置覆盖云端
  async function overrideCloudWithLocal() {
    const banner = syncBanner.value;
    if (!banner || banner.kind !== 'conflict') return;
    loadingStart();
    try {
      const items = configs.value.map(syncItemOf);
      const version = banner.remoteVersion + 1;
      const vaultContent = await uploadPayload(items, version);
      saveBase(items);
      await saveAnchor({ signature: await sha256Hex(vaultContent), version, updatedAt: Date.now() });
      await commitLastSync(version);
      syncRemoteState.value = { version, updatedAt: Date.now(), configCount: items.length };
      syncBanner.value = null;
      showMessage(`已用本机配置覆盖云端（版本 ${version}）`);
    } catch (error) {
      showError(error instanceof Error ? error.message : '覆盖云端失败');
    } finally {
      loadingEnd();
    }
  }

  /// 把配置项写回本机（存在则更新、缺失则新建）
  /// 云端 config 中可能混入非字符串标记（如脱敏态 masked），写库前仅保留字符串值
  async function applyItemsToLocal(items: RemoteConfigPreview[]) {
    for (const item of items) {
      const config: Record<string, string> = {};
      for (const [key, value] of Object.entries(item.config || {})) {
        if (typeof value === 'string') config[key] = value;
      }
      const body = JSON.stringify({
        picbed_type: item.picbed_type,
        config_name: item.config_name,
        is_default: item.is_default,
        config,
      });
      const existing = configs.value.find(configItem => configItem.config_name === item.config_name);
      if (existing) {
        await request(`/api/picbed/configs/${existing.id}`, { method: 'PUT', body });
      } else {
        await request('/api/picbed/configs', { method: 'POST', body });
      }
    }
    await reloadConfigs();
  }

  /// 整体覆盖本机配置：先删除目标集之外的本机配置，再更新/新建目标集（恢复类操作专用）
  async function replaceItemsToLocal(items: RemoteConfigPreview[]) {
    const targetNames = new Set(items.map(item => item.config_name));
    for (const existing of configs.value) {
      if (!targetNames.has(existing.config_name)) {
        await request(`/api/picbed/configs/${existing.id}`, { method: 'DELETE' });
      }
    }
    await applyItemsToLocal(items);
  }

  // ---------------------------------------------------------------------------
  // 历史版本（Gist 修订历史的浏览与恢复）
  // ---------------------------------------------------------------------------

  /// 确保拿到 Gist ID：优先本地缓存，缺失时云端现查
  async function ensureGistId(): Promise<string | null> {
    if (unauthorized()) return null;
    const cached = localStorage.getItem(GIST_ID_STORAGE_KEY);
    if (cached) return cached;
    const gist = await findSyncGist();
    if (!gist) return null;
    localStorage.setItem(GIST_ID_STORAGE_KEY, gist.id);
    return gist.id;
  }

  async function toggleSyncHistory() {
    if (syncHistoryOpen.value) {
      syncHistoryOpen.value = false;
      selectedRevisionSha.value = '';
      revisionPreview.value = null;
      revisionError.value = '';
      return;
    }
    if (unauthorized()) {
      showError('请先连接 GitHub');
      return;
    }
    syncHistoryOpen.value = true;
    syncHistoryLoading.value = true;
    revisionPreview.value = null;
    revisionError.value = '';
    try {
      const gistId = await ensureGistId();
      if (!gistId) {
        syncHistory.value = [];
        showMessage('未找到同步 Gist，请先完成一次同步');
        return;
      }
      const gist = (await githubFetch(`/gists/${gistId}`)) as {
        history?: Array<{ version: string; committed_at: string }>;
      };
      const total = (gist.history || []).length;
      const currentVersion = syncRemoteState.value?.version || 0;
      syncHistory.value = (gist.history || []).map((item, index) => {
        const isCurrent = index === 0;
        return {
          sha: item.version,
          date: new Date(item.committed_at).getTime(),
          dateText: formatTime(new Date(item.committed_at).getTime()),
          isCurrent,
          // 修订编号：距最新一版差几代就是 #几（当前版本不计入编号）
          label: isCurrent ? '当前版本' : `修订 #${total - index}`,
          versionBadge: isCurrent && currentVersion > 0 ? `v${currentVersion}` : '',
        };
      });
      // 默认选中当前版本（即云端现行内容）
      if (syncHistory.value.length) {
        selectedRevisionSha.value = syncHistory.value[0].sha;
        revisionError.value = '';
      }
    } catch (error) {
      showError(error instanceof Error ? error.message : '获取历史版本失败');
    } finally {
      syncHistoryLoading.value = false;
    }
  }

  /// 选中修订即下载解密做摘要预览；当前版本为默认选中，不解密预览也不可取消
  async function selectRevision(sha: string) {
    const currentItem = syncHistory.value.find(item => item.isCurrent);
    if (currentItem && currentItem.sha === sha) {
      selectedRevisionSha.value = sha;
      revisionPreview.value = null;
      revisionError.value = '';
      return;
    }
    if (selectedRevisionSha.value === sha) {
      selectedRevisionSha.value = '';
      revisionPreview.value = null;
      revisionError.value = '';
      return;
    }
    if (unauthorized() || passwordStatus.value !== 'unlocked') {
      showError('需先连接 GitHub 并解锁同步密码');
      return;
    }
    const gistId = await ensureGistId();
    if (!gistId) {
      showError('未找到同步 Gist');
      return;
    }
    selectedRevisionSha.value = sha;
    revisionPreview.value = null;
    revisionPreviewLoading.value = true;
    try {
      const gist = (await githubFetch(`/gists/${gistId}/${sha}`)) as { files?: Record<string, GistFileEntry> };
      const file = gist.files?.[GIST_FILENAME];
      if (!file) throw new Error('该修订不包含同步文件');
      if (!file.content || file.truncated) throw new Error('该修订数据过大且被截断，暂不支持预览');
      const vault = parseVault(file.content);
      let configs: RemoteConfigPreview[];
      try {
        configs = await decryptVaultConfigs(vault, syncPassword.value);
      } catch {
        // 解密失败以错误卡展示在列表下方（同步密码不同的修订无法预览或恢复）
        revisionError.value = '解密失败：该修订可能使用了与当前不同的同步密码（修改密码只影响之后的上传），无法预览或恢复';
        return;
      }
      if (selectedRevisionSha.value !== sha) return;
      revisionPreview.value = {
        sha,
        version: vault.version,
        updatedAt: vault.updatedAt,
        updatedAtText: formatTime(vault.updatedAt),
        configCount: configs.length,
        currentCount: localConfigCount.value,
        deviceName: vault.deviceName,
        appVersion: vault.appVersion,
        configs,
      };
    } catch (error) {
      if (selectedRevisionSha.value === sha) {
        selectedRevisionSha.value = '';
        revisionError.value = error instanceof Error ? error.message : '预览历史版本失败';
      }
    } finally {
      revisionPreviewLoading.value = false;
    }
  }

  /// 恢复到选中的历史版本：旧数据写回本机，并作为新版本追加推送（不改写 Gist 修订历史）。
  /// 恢复是有意的覆盖，跳过收缩护栏；需已连接、已解锁且无待处理的冲突/阻塞。
  async function restoreRevision() {
    const preview = revisionPreview.value;
    if (!preview || preview.sha !== selectedRevisionSha.value) return;
    if (unauthorized() || passwordStatus.value !== 'unlocked') {
      showError('需先连接 GitHub 并解锁同步密码');
      return;
    }
    if (syncBusy.value) {
      showError('同步正在进行中，请稍后再试');
      return;
    }
    if (syncBanner.value) {
      showError('请先处理当前的冲突/阻塞状态');
      return;
    }
    const gistId = await ensureGistId();
    if (!gistId) {
      showError('未找到同步 Gist');
      return;
    }
    restoringRevision.value = true;
    loadingStart();
    try {
      // 以云端当前版本 + 1 作为恢复后的新版本（追加式，历史不被改写）；
      // 版本号取修订与锚点/云端的较大值，避免恢复把版本号顶回去
      let baseVersion = Math.max(preview.version, loadAnchor()?.version ?? 0);
      if (baseVersion <= 0) {
        const gist = await findSyncGist();
        if (gist) baseVersion = parseVault(gist.content).version;
      }
      await replaceItemsToLocal(preview.configs);
      const version = baseVersion + 1;
      const vaultContent = await uploadPayload(preview.configs, version);
      saveBase(preview.configs);
      await saveAnchor({ signature: await sha256Hex(vaultContent), version, updatedAt: Date.now() });
      await commitLastSync(version);
      syncRemoteState.value = { version, updatedAt: Date.now(), configCount: preview.configs.length };
      revisionPreview.value = null;
      selectedRevisionSha.value = '';
      syncHistoryOpen.value = false;
      showMessage(`已恢复历史版本并推送为版本 ${version}（${preview.configs.length} 个图床配置）`);
    } catch (error) {
      showError(error instanceof Error ? error.message : '恢复历史版本失败');
    } finally {
      restoringRevision.value = false;
      loadingEnd();
    }
  }

  /// 阻塞与空库横幅：恢复远端版本（写入本机并采纳为 base）
  async function restoreRemoteVersion() {
    const banner = syncBanner.value;
    if (!banner || banner.remoteItems.length === 0) return;
    loadingStart();
    try {
      await replaceItemsToLocal(banner.remoteItems);
      saveBase(banner.remoteItems);
      await saveAnchor({ signature: banner.remoteSignature, version: banner.remoteVersion, updatedAt: Date.now() });
      await commitLastSync(banner.remoteVersion);
      syncRemoteState.value = { version: banner.remoteVersion, updatedAt: Date.now(), configCount: banner.remoteItems.length };
      syncBanner.value = null;
      showMessage(`已恢复远端版本（${banner.remoteItems.length} 个配置）`);
    } catch (error) {
      showError(error instanceof Error ? error.message : '恢复远端版本失败');
    } finally {
      loadingEnd();
    }
  }

  /// 冲突横幅：输入云端同步密码解密远端并恢复到本机，本机密码重置为该密码
  async function resolveConflictWithCloudPassword(cloudPassword: string): Promise<string | null> {
    const banner = syncBanner.value;
    if (!banner || banner.kind !== 'conflict' || !banner.remotePayload) {
      return '当前没有待处理的密码冲突';
    }
    if (!cloudPassword.trim()) {
      return '请输入云端同步密码';
    }
    loadingStart();
    try {
      const configs = await decryptConfigsWith(banner.remotePayload, cloudPassword, banner.remoteIv, banner.remoteSalt);
      await applyItemsToLocal(configs);
      // 本机同步密码重置为云端密码，后续同步保持一致
      syncPassword.value = cloudPassword;
      passwordHintHash.value = sha256HexSync(cloudPassword);
      localStorage.setItem(PASSWORD_HINT_STORAGE_KEY, passwordHintHash.value);
      saveBase(configs);
      await saveAnchor({ signature: banner.remoteSignature, version: banner.remoteVersion, updatedAt: Date.now() });
      await commitLastSync(banner.remoteVersion);
      syncRemoteState.value = { version: banner.remoteVersion, updatedAt: Date.now(), configCount: configs.length };
      syncBanner.value = null;
      showMessage(`已使用云端密码恢复远端配置（${configs.length} 个图床配置），本机同步密码已重置`);
      return null;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      return message.includes('同步密码不正确') ? '云端密码不正确，请重试' : message;
    } finally {
      loadingEnd();
    }
  }

  /// 阻塞横幅：跳过收缩护栏强制推送本机配置
  async function forcePushLocal() {
    const banner = syncBanner.value;
    if (!banner || banner.kind !== 'blocked') return;
    loadingStart();
    try {
      const items = configs.value.map(syncItemOf);
      const version = banner.remoteVersion + 1;
      const vaultContent = await uploadPayload(items, version);
      saveBase(items);
      await saveAnchor({ signature: await sha256Hex(vaultContent), version, updatedAt: Date.now() });
      await commitLastSync(version);
      syncRemoteState.value = { version, updatedAt: Date.now(), configCount: items.length };
      syncBanner.value = null;
      showMessage(`已强制推送本机 ${items.length} 个配置（版本 ${version}）`);
    } catch (error) {
      showError(error instanceof Error ? error.message : '强制推送失败');
    } finally {
      loadingEnd();
    }
  }

  function dismissSyncBanner() {
    syncBanner.value = null;
  }

  function loadingStart() {
    syncBusy.value = true;
  }
  function loadingEnd() {
    syncBusy.value = false;
  }

  void restoreConnection();

  return {
    syncAccount,
    secretBackend,
    syncPassword,
    syncPatInput,
    syncRemoteState,
    syncBusy,
    syncConnected,
    localConfigCount,
    syncStatusBadge,
    remoteVersionBadge,
    lastSyncText,
    canSyncNow,
    syncNow,
    syncBanner,
    overrideCloudWithLocal,
    resolveConflictWithCloudPassword,
    restoreRemoteVersion,
    forcePushLocal,
    dismissSyncBanner,
    passwordStatus,
    passwordModalOpen,
    passwordModalMode,
    openPasswordModal,
    closePasswordModal,
    submitPasswordModal,
    unlockWithPassword,
    githubConnectOpen,
    githubConnectPreparing,
    deviceFlowUserCode,
    deviceFlowStatus,
    deviceFlowError,
    deviceFlowAvailable: computed(() => syncClientId.value.length > 0),
    patMode,
    autoSyncEnabled,
    autoSyncDisplayChecked,
    autoSyncToggleEnabled,
    autoSyncToggleTitle,
    setAutoSyncEnabled,
    openGithubConnect,
    cancelGithubConnect,
    openVerificationPage,
    copyUserCode,
    connectGithub,
    disconnectGithub,
    syncHistoryOpen,
    syncHistoryLoading,
    syncHistory,
    selectedRevisionSha,
    revisionPreview,
    revisionPreviewLoading,
    revisionError,
    restoringRevision,
    toggleSyncHistory,
    selectRevision,
    restoreRevision,
    refreshRemoteState,
  };
}
