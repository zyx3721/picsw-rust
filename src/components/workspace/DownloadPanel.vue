<script setup lang="ts">
import { DownloadCloud, FileSearch, FolderOpen, Trash2, UploadCloud } from 'lucide-vue-next';
import AppTooltip from '../AppTooltip.vue';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';

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
        <h2>网络图片文档</h2>
      </div>
      <label
        class="upload-control"
        :class="{ dragging: downloadDragActive }"
        @dragenter.prevent="downloadDragActive = true"
        @dragover.prevent="downloadDragActive = true"
        @dragleave.prevent="downloadDragActive = false"
        @drop.prevent="handleDownloadDocumentDrop"
        ><UploadCloud :size="18" /><span>上传或拖动多个 .md</span
        ><input type="file" multiple accept=".md,text/markdown" @change="handleDownloadDocumentFiles"
      /></label>
    </div>
    <div class="batch-list local-list">
      <div v-for="item in downloadDocuments" :key="item.id" class="batch-row" :data-status="item.status">
        <div class="row-info">
          <strong>{{ item.filename }}</strong
          ><span>{{ statusLabel(item.status) }} · {{ item.images.length }} 个网络图片</span
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
      <p v-if="downloadDocuments.length === 0" class="empty">上传 Markdown 后，文档会显示在这里</p>
    </div>
  </div>
  <div class="panel stack">
    <div class="section-title">
      <div>
        <p class="section-kicker">Images</p>
        <h2>网络图片列表</h2>
      </div>
    </div>
    <div class="local-summary">
      <span>图片 {{ downloadTotalCount }}</span>
      <span>成功 {{ downloadSuccessCount }}</span>
      <span>失败 {{ downloadFailedCount }}</span>
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
      <p v-if="downloadItems.length === 0" class="empty">上传 Markdown 后，会在这里显示网络图片匹配情况</p>
    </div>
  </div>
  <div class="panel stack local-action-panel">
    <div class="section-title">
      <div>
        <p class="section-kicker">Target</p>
        <h2>下载设置</h2>
      </div>
    </div>
    <div class="conversion-controls local-controls">
      <label class="select-field local-target-field">
        <span>下载目录</span>
        <div class="download-dir-field">
          <input
            v-model="downloadTargetDir"
            type="text"
            placeholder="选择或输入图片保存目录"
            spellcheck="false"
          /><button class="ghost icon-only" type="button" title="选择目录" @click="selectDownloadDir">
            <FolderOpen :size="17" />
          </button>
        </div>
      </label>
      <div class="actions compact-actions">
        <button class="secondary" type="button" :disabled="loading || downloadDocuments.length === 0" @click="analyzeDownloadDocuments">
          <FileSearch :size="18" />分析文档</button
        ><button class="primary" type="button" :disabled="!canDownloadBatch" @click="downloadAllImages">
          <DownloadCloud :size="18" />{{ downloadActive ? '下载中…' : '下载全部图片' }}
        </button>
      </div>
    </div>
  </div>
</section>
</template>
