<script setup lang="ts">
import { ExternalLink } from 'lucide-vue-next';
import { onMounted, ref } from 'vue';
import { getVersion } from '@tauri-apps/api/app';
import { openUrl } from '@tauri-apps/plugin-opener';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';

const GITHUB_URL = 'https://github.com/zyx3721/picsw-rust';
const { showError } = useWorkspaceContext();

const appVersion = ref('');

onMounted(async () => {
  try {
    appVersion.value = await getVersion();
  } catch {
    appVersion.value = '';
  }
});

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
