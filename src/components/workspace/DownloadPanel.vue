<script setup lang="ts">
import { DownloadCloud, FileSearch, FolderOpen, Trash2, UploadCloud } from 'lucide-vue-next';
import AppTooltip from '../AppTooltip.vue';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';
import { t } from '../../i18n';

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
  canDownloadBatch,
  typeLabel,
  statusLabel,
  handleDownloadDocumentFiles,
  handleDownloadDocumentDrop,
  removeDownloadDocument,
  selectDownloadDir,
  analyzeDownloadDocuments,
  downloadAllImages,
} = useWorkspaceContext();
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
    <div class="batch-list local-list">
      <div v-for="item in downloadDocuments" :key="item.id" class="batch-row" :data-status="item.status">
        <div class="row-info">
          <strong>{{ item.filename }}</strong
          ><span>{{ statusLabel(item.status) }} · {{ t('download.remoteImages', { count: item.images.length }) }}</span
          ><AppTooltip v-if="item.error" :label="item.error" wrap>
            <small class="error-tip-text">{{ item.error }}</small>
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
    <div class="local-summary">
      <span>{{ t('download.imagesCount', { count: downloadTotalCount }) }}</span>
      <span>{{ t('download.successCount', { count: downloadSuccessCount }) }}</span>
      <span>{{ t('download.failedCount', { count: downloadFailedCount }) }}</span>
    </div>
    <div class="batch-list local-image-list">
      <div
        v-for="image in downloadItems"
        :key="image.key"
        class="batch-row local-image-row"
        :data-status="image.status"
      >
        <div>
          <strong>{{ image.name }}</strong>
          <AppTooltip :label="image.url" wrap>
            <span>{{ image.url }}</span>
          </AppTooltip>
          <AppTooltip v-if="image.error" :label="image.error" wrap>
            <small class="error-tip-text">{{ image.error }}</small>
          </AppTooltip>
        </div>
        <small>{{ typeLabel(image.picbed) }}</small>
      </div>
      <p v-if="downloadItems.length === 0" class="empty">{{ t('download.listEmpty') }}</p>
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
</section>
</template>
