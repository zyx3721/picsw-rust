<script setup lang="ts">
import { ChevronDown, Download, FileSearch, ImagePlus, Trash2, UploadCloud, Wand2 } from 'lucide-vue-next';
import AppTooltip from '../AppTooltip.vue';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';
import { t } from '../../i18n';

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
        <h2>{{ t('local.docTitle') }}</h2>
      </div>
      <label
        class="upload-control"
        :class="{ dragging: localDocumentDragActive }"
        @dragenter.prevent="localDocumentDragActive = true"
        @dragover.prevent="localDocumentDragActive = true"
        @dragleave.prevent="localDocumentDragActive = false"
        @drop.prevent="handleLocalDocumentDrop"
        ><UploadCloud :size="18" /><span>{{ t('convert.uploadHint') }}</span
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
            >{{ localStatusLabel(item.status) }} ·
            {{ t('local.refsMatched', { refs: item.references.length, matched: item.matched }) }}</span
          ><AppTooltip v-if="item.missing.length" :label="t('local.missingList', { list: item.missing.join('、') })" wrap>
            <small class="error-tip-text">{{ t('local.missingList', { list: item.missing.join('、') }) }}</small>
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
      <p v-if="localDocuments.length === 0" class="empty">{{ t('local.docEmpty') }}</p>
    </div>
  </div>
  <div class="panel stack">
    <div class="section-title">
      <div>
        <p class="section-kicker">Images</p>
        <h2>{{ t('local.imagesTitle') }}</h2>
      </div>
      <label
        class="upload-control"
        :class="{ dragging: localImageDragActive }"
        @dragenter.prevent="localImageDragActive = true"
        @dragover.prevent="localImageDragActive = true"
        @dragleave.prevent="localImageDragActive = false"
        @drop.prevent="handleLocalImageDrop"
        ><ImagePlus :size="18" /><span>{{ t('local.uploadImages') }}</span
        ><input type="file" multiple accept="image/*" @change="handleLocalImageFiles"
      /></label>
      <label class="upload-control">
        <ImagePlus :size="18" /><span>{{ t('common.chooseDir') }}</span
        ><input type="file" multiple webkitdirectory @change="handleLocalImageFiles"
      /></label>
    </div>
    <div class="local-summary">
      <span>{{ t('local.imagesCount', { count: localImages.length }) }}</span>
      <span>{{ t('local.matchedCount', { count: localMatchedCount }) }}</span>
      <span>{{ t('local.missingCount', { count: localMissingCount }) }}</span>
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
      <p v-if="localImages.length === 0" class="empty">{{ t('local.imageEmpty') }}</p>
    </div>
  </div>
  <div class="panel stack local-action-panel">
    <div class="section-title">
      <div>
        <p class="section-kicker">Target</p>
        <h2>{{ t('local.settingsTitle') }}</h2>
      </div>
    </div>
    <div class="conversion-controls local-controls">
      <label class="select-field local-target-field">
        <span>{{ t('common.targetConfig') }}</span>
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
              {{ t('common.pleaseSelect') }}
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
              <small>{{ typeLabel(item.picbed_type) }}{{ item.is_default ? t('common.defaultSuffix') : '' }}</small>
            </button>
          </div>
        </div>
      </label>
      <div class="actions compact-actions">
        <button class="secondary" type="button" :disabled="loading" @click="analyzeLocalBatch">
          <FileSearch :size="18" />{{ t('local.matchImages') }}</button
        ><button
          class="primary"
          type="button"
          :disabled="loading || !canUploadLocalBatch"
          @click="uploadLocalBatch"
        >
          <Wand2 :size="18" />{{ t('local.uploadReplace') }}
        </button>
      </div>
    </div>
    <button class="secondary" type="button" :disabled="!localConvertedCount" @click="downloadAllLocalFiles">
      <Download :size="18" />{{ t('common.downloadAll') }}
    </button>
  </div>
</section>
</template>
