<script setup lang="ts">
import { DownloadCloud, FileSearch, FolderOpen, Trash2, UploadCloud } from 'lucide-vue-next';
import { computed, ref } from 'vue';
import AppTooltip from '../AppTooltip.vue';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';
import type { DownloadDocument } from '../../composables/workspace/types';
import { t } from '../../i18n';
import { localizeError } from '../../i18n/errorText';

const {
  loading,
  downloadDragActive,
  downloadDocuments,
  downloadItems,
  downloadTargetDir,
  downloadActive,
  downloadTotalCount,
  downloadSuccessCount,
  downloadFailedCount,
  downloadFilter,
  downloadFilteredItems,
  canDownloadBatch,
  downloadStatusLabel,
  setDownloadFilter,
  typeLabel,
  handleDownloadDocumentFiles,
  handleDownloadDocumentDrop,
  removeDownloadDocument,
  clearDownloadDocuments,
  selectDownloadDir,
  analyzeDownloadDocuments,
  downloadAllImages,
} = useWorkspaceContext();

const clearConfirmOpen = ref(false);
const downloadListSummary = computed(() => {
  let images = 0;
  let duplicates = 0;
  for (const document of downloadDocuments.value) {
    images += document.images.length;
    const uniqueImages = new Set(document.images.map(image => image.url.trim())).size;
    duplicates += Math.max(0, document.images.length - uniqueImages);
  }
  return t('download.listSummary', {
    documents: downloadDocuments.value.length,
    images,
    duplicateSuffix: duplicates > 0 ? t('download.duplicateSuffix', { count: duplicates }) : '',
  });
});

function remoteImagesLabel(item: DownloadDocument) {
  const uniqueImages = new Set(item.images.map(image => image.url.trim())).size;
  const duplicates = Math.max(0, item.images.length - uniqueImages);
  return `${t('download.remoteImages', { count: item.images.length })}${duplicates > 0 ? t('download.duplicateSuffix', { count: duplicates }) : ''}`;
}

/// 确认后清空网络图片文档与识别结果
function confirmClearDocuments() {
  clearConfirmOpen.value = false;
  clearDownloadDocuments();
}
</script>

<template>
<section class="grid convert-layout">
  <div class="panel stack">
    <div class="section-title">
      <div>
        <p class="section-kicker">Remote</p>
        <h2>{{ t('download.docTitle') }}</h2>
      </div>
      <label
        class="upload-control"
        :class="{ dragging: downloadDragActive }"
        @dragenter.prevent="downloadDragActive = true"
        @dragover.prevent="downloadDragActive = true"
        @dragleave.prevent="downloadDragActive = false"
        @drop.prevent="handleDownloadDocumentDrop"
        ><UploadCloud :size="18" /><span>{{ t('convert.uploadHint') }}</span
        ><input type="file" multiple accept=".md,text/markdown" @change="handleDownloadDocumentFiles"
      /></label>
    </div>
    <div v-if="downloadDocuments.length" class="batch-list-head">
      <span>{{ downloadListSummary }}</span>
      <button
        class="ghost danger-text"
        type="button"
        :disabled="loading || downloadActive"
        @click="clearConfirmOpen = true"
      >
        <Trash2 :size="15" />{{ t('convert.clearAll') }}
      </button>
    </div>
    <div class="batch-list local-list">
      <div v-for="item in downloadDocuments" :key="item.id" class="batch-row" :data-status="item.status">
        <div class="row-info">
          <strong>{{ item.filename }}</strong
          ><span>{{ downloadStatusLabel(item.status) }} · {{ remoteImagesLabel(item) }}</span
          ><AppTooltip v-if="item.error" :label="localizeError(item.error)" wrap>
            <small class="error-tip-text">{{ localizeError(item.error) }}</small>
          </AppTooltip>
        </div>
        <div class="row-actions">
          <button class="danger icon-only" type="button" @click="removeDownloadDocument(item.id)">
            <Trash2 :size="17" />
          </button>
        </div>
      </div>
      <p v-if="downloadDocuments.length === 0" class="empty">{{ t('download.docEmpty') }}</p>
    </div>
  </div>
  <div class="panel stack">
    <div class="section-title">
      <div>
        <p class="section-kicker">Images</p>
        <h2>{{ t('download.imagesTitle') }}</h2>
      </div>
    </div>
    <div class="local-summary summary-filters" role="group" :aria-label="t('download.filterGroup')">
      <button
        class="summary-filter"
        :class="{ active: downloadFilter === 'all' }"
        type="button"
        :aria-pressed="downloadFilter === 'all'"
        @click="setDownloadFilter('all')"
      >
        {{ t('download.imagesCount', { count: downloadTotalCount }) }}
      </button>
      <button
        class="summary-filter"
        :class="{ active: downloadFilter === 'success' }"
        type="button"
        :aria-pressed="downloadFilter === 'success'"
        @click="setDownloadFilter('success')"
      >
        {{ t('download.successCount', { count: downloadSuccessCount }) }}
      </button>
      <button
        class="summary-filter"
        :class="{ active: downloadFilter === 'failed' }"
        type="button"
        :aria-pressed="downloadFilter === 'failed'"
        @click="setDownloadFilter('failed')"
      >
        {{ t('download.failedCount', { count: downloadFailedCount }) }}
      </button>
    </div>
    <div class="batch-list local-image-list">
      <div
        v-for="image in downloadFilteredItems"
        :key="image.key"
        class="batch-row local-image-row"
        :data-status="image.status"
      >
        <div>
          <AppTooltip class="local-image-name" :label="image.name" wrap>
            <strong>{{ image.name }}</strong>
          </AppTooltip>
          <AppTooltip class="local-image-path" :label="image.url" wrap>
            <span>{{ image.url }}</span>
          </AppTooltip>
          <AppTooltip v-if="image.error" :label="localizeError(image.error)" wrap>
            <small class="error-tip-text">{{ localizeError(image.error) }}</small>
          </AppTooltip>
        </div>
        <small>{{ typeLabel(image.picbed) }}</small>
      </div>
      <p v-if="downloadFilteredItems.length === 0" class="empty">
        {{ downloadItems.length ? t('download.filterEmpty') : t('download.listEmpty') }}
      </p>
    </div>
  </div>
  <div class="panel stack local-action-panel">
    <div class="section-title">
      <div>
        <p class="section-kicker">Target</p>
        <h2>{{ t('download.settingsTitle') }}</h2>
      </div>
    </div>
    <div class="conversion-controls local-controls">
      <label class="select-field local-target-field">
        <span>{{ t('download.dirLabel') }}</span>
        <div class="download-dir-field">
          <input
            v-model="downloadTargetDir"
            type="text"
            :placeholder="t('download.dirPlaceholder')"
            spellcheck="false"
          /><button class="ghost icon-only" type="button" :title="t('common.chooseDir')" @click="selectDownloadDir">
            <FolderOpen :size="17" />
          </button>
        </div>
      </label>
      <div class="actions compact-actions">
        <button class="secondary" type="button" :disabled="loading || downloadDocuments.length === 0" @click="analyzeDownloadDocuments">
          <FileSearch :size="18" />{{ t('download.analyzeDocs') }}</button
        ><button class="primary" type="button" :disabled="!canDownloadBatch" @click="downloadAllImages">
          <DownloadCloud :size="18" />{{ downloadActive ? t('download.downloading') : t('download.start') }}
        </button>
      </div>
    </div>
  </div>
  <Teleport to="body">
    <div v-if="clearConfirmOpen" class="modal-backdrop" role="presentation" @click.self="clearConfirmOpen = false">
      <section class="confirm-dialog" role="dialog" aria-modal="true" aria-labelledby="clear-download-title">
        <div class="dialog-head">
          <div class="dialog-icon"><Trash2 :size="22" /></div>
          <h2 id="clear-download-title">{{ t('download.clearAllTitle') }}</h2>
        </div>
        <div class="dialog-copy">
          <p>{{ t('download.clearAllDesc', { count: downloadDocuments.length }) }}</p>
        </div>
        <div class="dialog-actions">
          <button class="ghost" type="button" @click="clearConfirmOpen = false">{{ t('common.cancel') }}</button>
          <button class="danger" type="button" @click="confirmClearDocuments">
            <Trash2 :size="17" />{{ t('download.clearAllConfirm') }}
          </button>
        </div>
      </section>
    </div>
  </Teleport>
</section>
</template>
