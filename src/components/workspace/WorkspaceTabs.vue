<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { ArrowRightLeft, Cloud, DownloadCloud, FolderUp, History, Info, Settings } from 'lucide-vue-next';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';

type WorkspaceTab = 'convert' | 'localUpload' | 'download' | 'configs' | 'records' | 'cloudSync' | 'about';

const { activeTab, setActiveTab, isAuthed } = useWorkspaceContext();
const tabOrder: WorkspaceTab[] = ['convert', 'localUpload', 'download', 'configs', 'records', 'cloudSync', 'about'];
const tabsRef = ref<HTMLElement | null>(null);
const tabButtonRefs = ref<Record<WorkspaceTab, HTMLButtonElement | null>>({
  convert: null,
  localUpload: null,
  download: null,
  configs: null,
  records: null,
  cloudSync: null,
  about: null,
});
const tabsIndicatorStyle = ref<Record<string, string>>({
  '--tab-duration': '1000ms',
  transform: 'translateX(0px)',
  width: '0px',
});
let tabsResizeObserver: ResizeObserver | undefined;

function setTabButtonRef(tab: WorkspaceTab, el: unknown) {
  tabButtonRefs.value[tab] = el instanceof HTMLButtonElement ? el : null;
}
function tabMoveDuration(fromTab: WorkspaceTab, toTab: WorkspaceTab) {
  const distance = Math.abs(tabOrder.indexOf(toTab) - tabOrder.indexOf(fromTab)) || 1;
  return String(distance * 1000) + 'ms';
}
function updateTabsIndicator(duration = '0ms') {
  const currentTab = activeTab.value as WorkspaceTab;
  const currentButton = tabButtonRefs.value[currentTab];
  if (!tabsRef.value || !currentButton) return;
  tabsIndicatorStyle.value = {
    '--tab-duration': duration,
    transform: 'translateX(' + currentButton.offsetLeft + 'px)',
    width: currentButton.offsetWidth + 'px',
  };
}
function handleTabClick(tab: WorkspaceTab) {
  setActiveTab(tab);
}
function handleTabsResize() {
  updateTabsIndicator('0ms');
}

watch(activeTab, (tab, oldTab) => {
  const duration = oldTab ? tabMoveDuration(oldTab as WorkspaceTab, tab as WorkspaceTab) : '0ms';
  void nextTick(() => updateTabsIndicator(duration));
});
watch(isAuthed, authed => {
  if (!authed) return;
  void nextTick(() => updateTabsIndicator('0ms'));
});

onMounted(() => {
  void nextTick(() => updateTabsIndicator('0ms'));
  tabsResizeObserver = new ResizeObserver(handleTabsResize);
  if (tabsRef.value) tabsResizeObserver.observe(tabsRef.value);
  window.addEventListener('resize', handleTabsResize);
});
onBeforeUnmount(() => {
  tabsResizeObserver?.disconnect();
  window.removeEventListener('resize', handleTabsResize);
});
</script>

<template>
  <nav ref="tabsRef" class="tabs">
    <span class="tabs-indicator" :style="tabsIndicatorStyle" aria-hidden="true"></span>
    <button :ref="el => setTabButtonRef('convert', el)" :class="{ active: activeTab === 'convert' }" @click="handleTabClick('convert')">
      <ArrowRightLeft :size="18" />批量转换
    </button>
    <button :ref="el => setTabButtonRef('localUpload', el)" :class="{ active: activeTab === 'localUpload' }" @click="handleTabClick('localUpload')">
      <FolderUp :size="18" />本地上传
    </button>
    <button :ref="el => setTabButtonRef('download', el)" :class="{ active: activeTab === 'download' }" @click="handleTabClick('download')">
      <DownloadCloud :size="18" />下载图片
    </button>
    <button :ref="el => setTabButtonRef('configs', el)" :class="{ active: activeTab === 'configs' }" @click="handleTabClick('configs')">
      <Settings :size="18" />图床配置
    </button>
    <button :ref="el => setTabButtonRef('records', el)" :class="{ active: activeTab === 'records' }" @click="handleTabClick('records')">
      <History :size="18" />历史记录
    </button>
    <button :ref="el => setTabButtonRef('cloudSync', el)" :class="{ active: activeTab === 'cloudSync' }" @click="handleTabClick('cloudSync')">
      <Cloud :size="18" />云同步
    </button>
    <button :ref="el => setTabButtonRef('about', el)" :class="{ active: activeTab === 'about' }" @click="handleTabClick('about')">
      <Info :size="18" />关于
    </button>
  </nav>
</template>
