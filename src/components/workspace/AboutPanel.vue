<script setup lang="ts">
import { Bug, ChevronDown, Code2, Download, ExternalLink, FileText, LogOut, Package, RefreshCw } from 'lucide-vue-next';
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { getVersion } from '@tauri-apps/api/app';
import { openUrl } from '@tauri-apps/plugin-opener';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';
import type { CloseAction } from '../../composables/workspace/closeBehavior';
import { relativeTimeText } from '../../composables/workspace/updateChecker';
import AppTooltip from '../AppTooltip.vue';

const GITHUB_URL = 'https://github.com/zyx3721/picsw-rust';
const ISSUES_URL = 'https://github.com/zyx3721/picsw-rust/issues';
const RELEASES_URL = 'https://github.com/zyx3721/picsw-rust/releases';
const CLOSE_ACTION_OPTIONS: { value: CloseAction; label: string }[] = [
  { value: 'ask', label: '每次询问' },
  { value: 'tray', label: '最小化到托盘' },
  { value: 'quit', label: '退出应用' },
];
const {
  showError,
  updateStatus,
  updateVersion,
  updateProgress,
  updateNotes,
  lastCheckAt,
  showUpToDate,
  autoCheckUpdate,
  setAutoCheckUpdate,
  checkUpdate,
  startUpdate,
  applyUpdate,
  openUpdatePackage,
  closeAction,
  setCloseAction,
} = useWorkspaceContext();

const appVersion = ref('');
const closeActionOpen = ref(false);

const updateBusy = computed(
  () => updateStatus.value === 'checking' || updateStatus.value === 'downloading'
);

/// 下载进度百分比与可读字节数（总长未知时只显示已下载量）
const updateProgressText = computed(() => {
  const progress = updateProgress.value;
  if (!progress) return '';
  const done = formatBytes(progress.downloaded);
  if (progress.total > 0) {
    const percent = Math.min(100, Math.floor((progress.downloaded / progress.total) * 100));
    return `${percent}%（${done} / ${formatBytes(progress.total)}）`;
  }
  return `已下载 ${done}`;
});

/// 自动检查开关行的描述：有检查记录时附上次检查时间
const autoCheckDescription = computed(() => {
  const base = '启动后静默检查新版本（每 24 小时一次），可随时手动检查';
  return lastCheckAt.value > 0 ? `上次检查：${relativeTimeText(lastCheckAt.value)}；${base}` : base;
});

/// 下拉框展示当前关闭行为文案
const closeActionLabel = computed(
  () => CLOSE_ACTION_OPTIONS.find(option => option.value === closeAction.value)?.label ?? '每次询问'
);

function toggleCloseActionMenu() {
  closeActionOpen.value = !closeActionOpen.value;
}

function chooseCloseAction(value: CloseAction) {
  setCloseAction(value);
  closeActionOpen.value = false;
}

function closeCloseActionMenu(event: PointerEvent) {
  const target = event.target;
  if (target instanceof Element && target.closest('.about-close-select')) return;
  closeActionOpen.value = false;
}

onMounted(() => document.addEventListener('pointerdown', closeCloseActionMenu));
onBeforeUnmount(() => document.removeEventListener('pointerdown', closeCloseActionMenu));

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/// 按钮文案随更新状态机切换，宽度随文本自适应
const updateButtonText = computed(() => {
  if (updateStatus.value === 'checking') return '检查中...';
  if (updateStatus.value === 'downloading') return '更新中...';
  if (updateStatus.value === 'available' && updateVersion.value) return `更新到 v${updateVersion.value}`;
  return '检查更新';
});

onMounted(async () => {
  try {
    appVersion.value = await getVersion();
  } catch {
    appVersion.value = '';
  }
});

/// 可用更新时开始下载安装，否则发起一次手动检查
async function handleUpdateClick() {
  if (updateStatus.value === 'available') {
    await startUpdate();
    return;
  }
  await checkUpdate();
}

/// 在系统浏览器打开外部链接
async function openExternal(url: string, fallback: string) {
  try {
    await openUrl(url);
  } catch (error) {
    showError(error instanceof Error ? error.message : fallback);
  }
}

/// 打开项目仓库主页
async function openGithubRepo() {
  await openExternal(GITHUB_URL, '打开项目主页失败');
}

/// 打开问题反馈页
async function openIssues() {
  await openExternal(ISSUES_URL, '打开反馈页失败');
}

/// 打开发布说明页
async function openReleases() {
  await openExternal(RELEASES_URL, '打开发布页失败');
}
</script>

<template>
<section class="sync-layout">
  <div class="panel stack">
    <div class="section-title">
      <div>
        <p class="section-kicker">About</p>
        <h2>关于<span class="about-title-hint">版本与项目信息</span></h2>
      </div>
      <div class="about-update-area">
        <span v-if="showUpToDate" class="about-update-tag">已是最新版</span>
        <AppTooltip v-if="updateStatus === 'available' && updateVersion" :label="updateNotes" pre>
          <span class="about-update-tag">检测到新版本： v{{ updateVersion }}</span>
        </AppTooltip>
        <span v-if="updateStatus === 'downloading'" class="about-update-tag">{{ updateProgressText }}</span>
        <template v-if="updateStatus === 'downloaded'">
          <span class="about-update-tag">更新包已就绪</span>
          <button class="secondary about-update-btn" type="button" @click="openUpdatePackage">打开更新包</button>
          <button class="primary about-update-btn" type="button" @click="applyUpdate">应用更新</button>
        </template>
        <button v-else class="primary about-update-btn" type="button" :disabled="updateBusy" @click="handleUpdateClick">
          <RefreshCw v-if="updateStatus === 'checking' || updateStatus === 'downloading'" :size="17" class="spin-icon" />
          <Download v-else :size="17" />{{ updateButtonText }}
        </button>
      </div>
    </div>

    <div class="about-row">
      <div class="about-row-main">
        <span class="about-row-icon"><RefreshCw :size="20" /></span>
        <div class="about-row-info">
          <strong>自动检查更新</strong>
          <span>{{ autoCheckDescription }}</span>
        </div>
      </div>
      <label class="sync-toggle">
        <input type="checkbox" :checked="autoCheckUpdate" @change="setAutoCheckUpdate(($event.target as HTMLInputElement).checked)" />
        <span class="sync-toggle-track" aria-hidden="true"></span>
      </label>
    </div>

    <div class="about-row">
      <div class="about-row-main">
        <span class="about-row-icon"><LogOut :size="20" /></span>
        <div class="about-row-info">
          <strong>关闭窗口时</strong>
          <span>每次询问为首次关闭时弹窗；最小化后应用在后台运行，转换任务不受影响，可从系统托盘恢复窗口或退出</span>
        </div>
      </div>
      <div class="custom-select about-close-select" :class="{ open: closeActionOpen }">
        <button class="select-trigger" type="button" :aria-expanded="closeActionOpen" @click="toggleCloseActionMenu">
          <span>{{ closeActionLabel }}</span>
          <ChevronDown :size="18" class="select-chevron" />
        </button>
        <div v-if="closeActionOpen" class="select-menu">
          <button
            v-for="option in CLOSE_ACTION_OPTIONS"
            :key="option.value"
            class="select-option"
            :class="{ selected: option.value === closeAction }"
            type="button"
            @click="chooseCloseAction(option.value)"
          >{{ option.label }}</button>
        </div>
      </div>
    </div>

    <div class="about-row">
      <div class="about-row-main">
        <span class="about-row-icon"><Package :size="20" /></span>
        <div class="about-row-info">
          <strong>PicBed Switcher 版本</strong>
        </div>
      </div>
      <span class="about-version">{{ appVersion || '—' }}</span>
    </div>

    <div class="about-row">
      <div class="about-row-main">
        <span class="about-row-icon"><Code2 :size="20" /></span>
        <div class="about-row-info">
          <strong>GitHub</strong>
          <span>源代码，欢迎 Star</span>
        </div>
      </div>
      <button class="about-link" type="button" @click="openGithubRepo">
        github.com/zyx3721/picsw-rust
        <ExternalLink :size="14" />
      </button>
    </div>

    <div class="about-row">
      <div class="about-row-main">
        <span class="about-row-icon"><Bug :size="20" /></span>
        <div class="about-row-info">
          <strong>反馈问题</strong>
          <span>报告 Bug 或提出建议</span>
        </div>
      </div>
      <button class="about-link" type="button" @click="openIssues">
        github.com/zyx3721/picsw-rust/issues
        <ExternalLink :size="14" />
      </button>
    </div>

    <div class="about-row">
      <div class="about-row-main">
        <span class="about-row-icon"><FileText :size="20" /></span>
        <div class="about-row-info">
          <strong>更新内容</strong>
          <span>查看发布说明</span>
        </div>
      </div>
      <button class="about-link" type="button" @click="openReleases">
        github.com/zyx3721/picsw-rust/releases
        <ExternalLink :size="14" />
      </button>
    </div>
  </div>
</section>
</template>
