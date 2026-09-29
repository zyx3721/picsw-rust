<script setup lang="ts">
import { Download, ExternalLink, RefreshCw } from 'lucide-vue-next';
import { computed, onMounted, ref } from 'vue';
import { getVersion } from '@tauri-apps/api/app';
import { openUrl } from '@tauri-apps/plugin-opener';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';

const GITHUB_URL = 'https://github.com/zyx3721/picsw-rust';
const { showError, updateStatus, updateVersion, checkUpdate, startUpdate } = useWorkspaceContext();

const appVersion = ref('');

const updateBusy = computed(() => updateStatus.value === 'checking' || updateStatus.value === 'downloading');

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

/// 在系统浏览器打开项目仓库主页
async function openGithubRepo() {
  try {
    await openUrl(GITHUB_URL);
  } catch (error) {
    showError(error instanceof Error ? error.message : '打开项目主页失败');
  }
}
</script>

<template>
<section class="sync-layout">
  <div class="panel stack">
    <div class="section-title">
      <div>
        <p class="section-kicker">About</p>
        <h2>关于</h2>
      </div>
      <div class="about-update-area">
        <span v-if="updateStatus === 'available' && updateVersion" class="about-update-tag">检测到新版本： v{{ updateVersion }}</span>
        <button class="primary about-update-btn" type="button" :disabled="updateBusy" @click="handleUpdateClick">
          <RefreshCw v-if="updateStatus === 'checking' || updateStatus === 'downloading'" :size="17" class="spin-icon" />
          <Download v-else :size="17" />{{ updateButtonText }}
        </button>
      </div>
    </div>

    <div class="about-row">
      <div class="about-row-info">
        <strong>PicBed Switcher 版本</strong>
      </div>
      <span class="about-version">{{ appVersion || '—' }}</span>
    </div>

    <div class="about-row">
      <div class="about-row-info">
        <strong>GitHub</strong>
        <span>项目地址，欢迎 Star / 反馈 Issue</span>
      </div>
      <button class="about-link" type="button" @click="openGithubRepo">
        github.com/zyx3721/picsw-rust
        <ExternalLink :size="14" />
      </button>
    </div>
  </div>
</section>
</template>
