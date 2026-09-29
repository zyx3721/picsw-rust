<script setup lang="ts">
import { ChevronDown, Download, FileSearch, ImagePlus, Trash2, UploadCloud, Wand2 } from 'lucide-vue-next';
import AppTooltip from '../AppTooltip.vue';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';

const {
  loading,
  localTargetConfigId,
  localTargetDropdownOpen,
  localDocumentDragActive,
  localImageDragActive,
  localDocuments,
  localImages,
  localTargetConfigs,
  selectedLocalTargetConfig,
  localMatchedCount,
  localMissingCount,
  localConvertedCount,
  canUploadLocalBatch,
  localTargetConfigLabel,
  selectLocalTargetConfig,
  localStatusLabel,
  typeLabel,
  handleLocalDocumentFiles,
  handleLocalDocumentDrop,
  handleLocalImageFiles,
  handleLocalImageDrop,
  removeLocalDocument,
  removeLocalImage,
  analyzeLocalBatch,
  uploadLocalBatch,
  downloadLocalFile,
  downloadAllLocalFiles,
} = useWorkspaceContext();
</script>

<template>
<section class="grid convert-layout">
  <div class="panel stack">
    <div class="section-title">
      <div>
        <p class="section-kicker">Local</p>
        <h2>本地图片文档</h2>
      </div>
      <label
        class="upload-control"
        :class="{ dragging: localDocumentDragActive }"
        @dragenter.prevent="localDocumentDragActive = true"
        @dragover.prevent="localDocumentDragActive = true"
        @dragleave.prevent="localDocumentDragActive = false"
        @drop.prevent="handleLocalDocumentDrop"
        ><UploadCloud :size="18" /><span>上传或拖动多个 .md</span
        ><input type="file" multiple accept=".md,text/markdown" @change="handleLocalDocumentFiles"
      /></label>
    </div>
    <div class="batch-list local-list">
      <div
        v-for="item in localDocuments"
        :key="item.id"
        class="batch-row"
        :data-status="item.status"
      >
        <div class="row-info">
          <strong>{{ item.filename }}</strong
          ><span
            >{{ localStatusLabel(item.status) }} · {{ item.references.length }} 个本地引用 · 已匹配
            {{ item.matched }}</span
          ><AppTooltip v-if="item.missing.length" :label="`缺少：${item.missing.join('、')}`" wrap>
            <small class="error-tip-text">缺少：{{ item.missing.join('、') }}</small>
          </AppTooltip>
          <AppTooltip v-else-if="item.error" :label="item.error" wrap>
            <small class="error-tip-text">{{ item.error }}</small>
          </AppTooltip>
        </div>
        <div class="row-actions">
          <button
            class="ghost icon-only"
            type="button"
            :disabled="!item.convertedContent"
            @click="downloadLocalFile(item)"
          >
            <Download :size="17" /></button
          ><button class="danger icon-only" type="button" @click="removeLocalDocument(item.id)">
            <Trash2 :size="17" />
          </button>
        </div>
      </div>
      <p v-if="localDocuments.length === 0" class="empty">上传 Markdown 后，会在这里显示本地图片匹配情况</p>
    </div>
  </div>
  <div class="panel stack">
    <div class="section-title">
      <div>
        <p class="section-kicker">Images</p>
        <h2>本地图片来源</h2>
      </div>
      <label
        class="upload-control"
        :class="{ dragging: localImageDragActive }"
        @dragenter.prevent="localImageDragActive = true"
        @dragover.prevent="localImageDragActive = true"
        @dragleave.prevent="localImageDragActive = false"
        @drop.prevent="handleLocalImageDrop"
        ><ImagePlus :size="18" /><span>上传图片</span
        ><input type="file" multiple accept="image/*" @change="handleLocalImageFiles"
      /></label>
      <label class="upload-control">
        <ImagePlus :size="18" /><span>选择目录</span
        ><input type="file" multiple webkitdirectory @change="handleLocalImageFiles"
      /></label>
    </div>
    <div class="local-summary">
      <span>图片 {{ localImages.length }}</span>
      <span>匹配 {{ localMatchedCount }}</span>
      <span>缺失 {{ localMissingCount }}</span>
    </div>
    <div class="batch-list local-image-list">
      <div v-for="image in localImages" :key="image.key" class="batch-row local-image-row" data-status="analyzed">
        <div>
          <strong>{{ image.name }}</strong>
          <span>{{ image.path }}</span>
        </div>
        <div class="row-actions">
          <button class="danger icon-only" type="button" @click="removeLocalImage(image.key)">
            <Trash2 :size="17" />
          </button>
        </div>
      </div>
      <p v-if="localImages.length === 0" class="empty">选择图片文件或包含图片的目录，用于匹配 Markdown 中的本地路径</p>
    </div>
  </div>
  <div class="panel stack local-action-panel">
    <div class="section-title">
      <div>
        <p class="section-kicker">Target</p>
        <h2>上传设置</h2>
      </div>
    </div>
    <div class="conversion-controls local-controls">
      <label class="select-field local-target-field">
        <span>目标配置</span>
        <div class="custom-select" :class="{ open: localTargetDropdownOpen }">
          <button
            class="select-trigger"
            type="button"
            :aria-expanded="localTargetDropdownOpen"
            @click="localTargetDropdownOpen = !localTargetDropdownOpen"
          >
            <span :class="{ placeholder: !selectedLocalTargetConfig }">
              {{ localTargetConfigLabel(selectedLocalTargetConfig) }}
            </span>
            <ChevronDown :size="18" class="select-chevron" />
          </button>
          <div v-if="localTargetDropdownOpen" class="select-menu">
            <button
              class="select-option placeholder-option"
              type="button"
              :class="{ selected: !localTargetConfigId }"
              @click="selectLocalTargetConfig(0)"
            >
              请选择
            </button>
            <button
              v-for="item in localTargetConfigs"
              :key="item.id"
              class="select-option"
              type="button"
              :class="{ selected: item.id === localTargetConfigId }"
              @click="selectLocalTargetConfig(item.id)"
            >
              <span>{{ item.config_name }}</span>
              <small>{{ typeLabel(item.picbed_type) }}{{ item.is_default ? ' · 默认' : '' }}</small>
            </button>
          </div>
        </div>
      </label>
      <div class="actions compact-actions">
        <button class="secondary" type="button" :disabled="loading" @click="analyzeLocalBatch">
          <FileSearch :size="18" />匹配图片</button
        ><button
          class="primary"
          type="button"
          :disabled="loading || !canUploadLocalBatch"
          @click="uploadLocalBatch"
        >
          <Wand2 :size="18" />上传并替换
        </button>
      </div>
    </div>
    <button class="secondary" type="button" :disabled="!localConvertedCount" @click="downloadAllLocalFiles">
      <Download :size="18" />下载全部结果
    </button>
  </div>
</section>
</template>
