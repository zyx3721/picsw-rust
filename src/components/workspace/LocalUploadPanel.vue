<script setup lang="ts">
import { ChevronDown, Download, FileSearch, FolderOpen, ImagePlus, Trash2, UploadCloud, Wand2 } from 'lucide-vue-next';
import { computed, ref } from 'vue';
import AppTooltip from '../AppTooltip.vue';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';
import type { ConversionRecordDetail, LocalDocument, ScannedLocalImage } from '../../composables/workspace/types';
import { t } from '../../i18n';
import { localizeError } from '../../i18n/errorText';

const {
  loading,
  showMessage,
  localTargetConfigId,
  localTargetDropdownOpen,
  localDocumentDragActive,
  localImageDragActive,
  localDocuments,
  localImages,
  localImageFilter,
  localFilteredImages,
  localMissingReferences,
  localTargetConfigs,
  selectedLocalTargetConfig,
  localMatchedCount,
  localMissingCount,
  localConvertedCount,
  canUploadLocalBatch,
  localTargetConfigLabel,
  selectLocalTargetConfig,
  setLocalImageFilter,
  localStatusLabel,
  typeLabel,
  handleLocalDocumentFiles,
  handleLocalDocumentDrop,
  handleLocalImageFiles,
  handleLocalImageDrop,
  scanLocalImageDirectory,
  addScannedLocalImages,
  removeLocalDocument,
  removeLocalImage,
  clearLocalDocuments,
  clearLocalImages,
  analyzeLocalBatch,
  uploadLocalBatch,
  downloadLocalFile,
  downloadAllLocalFiles,
} = useWorkspaceContext();

type ClearTarget = 'documents' | 'images';

const clearTarget = ref<ClearTarget | null>(null);
const pendingScan = ref<ScannedLocalImage[] | null>(null);
const clearCount = computed(() =>
  clearTarget.value === 'images' ? localImages.value.length : localDocuments.value.length
);
const clearTitle = computed(() =>
  clearTarget.value === 'images' ? t('local.clearImagesTitle') : t('local.clearDocsTitle')
);
const clearDesc = computed(() =>
  clearTarget.value === 'images'
    ? t('local.clearImagesDesc', { count: clearCount.value })
    : t('local.clearDocsDesc', { count: clearCount.value })
);
const clearConfirmLabel = computed(() =>
  clearTarget.value === 'images' ? t('local.clearImagesConfirm') : t('local.clearDocsConfirm')
);

/// 确认后按当前目标清空文档或本地图片
function confirmClear() {
  if (clearTarget.value === 'images') clearLocalImages();
  else if (clearTarget.value === 'documents') clearLocalDocuments();
  clearTarget.value = null;
}

function failedDetails(item: LocalDocument): ConversionRecordDetail[] {
  return (item.details || []).filter(detail => detail.status === 'failed');
}

function skippedDetails(item: LocalDocument): ConversionRecordDetail[] {
  return (item.details || []).filter(detail => detail.status === 'skipped');
}

async function chooseImageDirectory() {
  const images = await scanLocalImageDirectory();
  if (!images) return;
  if (!images.length) {
    showMessage(t('msg.local.scanEmpty'));
    return;
  }
  pendingScan.value = images;
}

function confirmScan() {
  if (pendingScan.value) addScannedLocalImages(pendingScan.value);
  pendingScan.value = null;
}
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
    <div v-if="localDocuments.length" class="batch-list-head">
      <span>{{ t('local.docCount', { count: localDocuments.length }) }}</span>
      <button class="ghost danger-text" type="button" :disabled="loading" @click="clearTarget = 'documents'">
        <Trash2 :size="15" />{{ t('convert.clearAll') }}
      </button>
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
          ><AppTooltip v-if="item.missing.length" :label="t('local.missingList', { list: item.missing.join(t('common.listSeparator')) })" wrap>
            <small class="error-tip-text">{{ t('local.missingList', { list: item.missing.join(t('common.listSeparator')) }) }}</small>
          </AppTooltip>
          <AppTooltip v-else-if="item.error" :label="localizeError(item.error)" wrap>
            <small class="error-tip-text">{{ localizeError(item.error) }}</small>
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
        <div v-if="failedDetails(item).length" class="doc-issue-list">
          <p class="doc-issue-title">{{ t('convert.issueFailed') }} · {{ failedDetails(item).length }}</p>
          <div v-for="detail in failedDetails(item).slice(0, 4)" :key="detail.id" class="doc-issue-row">
            <span class="doc-issue-url" :title="detail.original_url">{{ detail.original_url }}</span>
            <AppTooltip v-if="detail.error" :label="localizeError(detail.error)" wrap>
              <span class="doc-issue-error">{{ localizeError(detail.error) }}</span>
            </AppTooltip>
          </div>
          <small v-if="failedDetails(item).length > 4" class="doc-issue-more">{{ t('convert.issueMore', { count: failedDetails(item).length - 4 }) }}</small>
        </div>
        <div v-if="skippedDetails(item).length" class="doc-skip-list">
          <p class="doc-skip-title">{{ t('convert.issueSkipped') }} · {{ skippedDetails(item).length }}</p>
          <div v-for="detail in skippedDetails(item).slice(0, 4)" :key="detail.id" class="doc-skip-row">
            <span class="doc-skip-url" :title="detail.original_url">{{ detail.original_url }}</span>
            <span class="doc-skip-reason">{{ t('convert.skipReason') }}</span>
          </div>
          <small v-if="skippedDetails(item).length > 4" class="doc-skip-more">{{ t('convert.skipMore', { count: skippedDetails(item).length - 4 }) }}</small>
        </div>
      </div>
      <p v-if="localDocuments.length === 0" class="empty">{{ t('local.docEmpty') }}</p>
    </div>
  </div>
  <div class="panel stack">
    <div class="section-title local-images-title">
      <div>
        <p class="section-kicker">Images</p>
        <h2>{{ t('local.imagesTitle') }}</h2>
      </div>
      <div class="upload-controls">
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
        <button class="upload-control" type="button" :disabled="loading" @click="chooseImageDirectory">
          <FolderOpen :size="18" /><span>{{ t('common.chooseDir') }}</span>
        </button>
      </div>
    </div>
    <div class="local-summary summary-filters" role="group" :aria-label="t('local.filterGroup')">
      <button
        class="summary-filter"
        :class="{ active: localImageFilter === 'all' }"
        type="button"
        :aria-pressed="localImageFilter === 'all'"
        @click="setLocalImageFilter('all')"
      >
        {{ t('local.imagesCount', { count: localImages.length }) }}
      </button>
      <button
        class="summary-filter"
        :class="{ active: localImageFilter === 'matched' }"
        type="button"
        :aria-pressed="localImageFilter === 'matched'"
        @click="setLocalImageFilter('matched')"
      >
        {{ t('local.matchedCount', { count: localMatchedCount }) }}
      </button>
      <button
        class="summary-filter"
        :class="{ active: localImageFilter === 'missing' }"
        type="button"
        :aria-pressed="localImageFilter === 'missing'"
        @click="setLocalImageFilter('missing')"
      >
        {{ t('local.missingCount', { count: localMissingCount }) }}
      </button>
    </div>
    <div v-if="localImages.length" class="batch-list-head">
      <span>{{ t('local.imageCountLabel', { count: localImages.length }) }}</span>
      <button class="ghost danger-text" type="button" :disabled="loading" @click="clearTarget = 'images'">
        <Trash2 :size="15" />{{ t('convert.clearAll') }}
      </button>
    </div>
    <div v-if="localImageFilter !== 'missing'" class="batch-list local-image-list">
      <div v-for="image in localFilteredImages" :key="image.key" class="batch-row local-image-row" data-status="analyzed">
        <div>
          <AppTooltip class="local-image-name" :label="image.name" wrap>
            <strong>{{ image.name }}</strong>
          </AppTooltip>
          <AppTooltip class="local-image-path" :label="image.path" wrap>
            <span>{{ image.path }}</span>
          </AppTooltip>
        </div>
        <div class="row-actions">
          <button class="danger icon-only" type="button" @click="removeLocalImage(image.key)">
            <Trash2 :size="17" />
          </button>
        </div>
      </div>
      <p v-if="localFilteredImages.length === 0" class="empty">
        {{ localImages.length ? t('local.filterEmpty') : t('local.imageEmpty') }}
      </p>
    </div>
    <div v-else class="batch-list local-image-list">
      <div v-for="path in localMissingReferences" :key="path" class="batch-row local-image-row" data-status="failed">
        <div>
          <AppTooltip class="local-image-name" :label="path" wrap>
            <strong>{{ path }}</strong>
          </AppTooltip>
          <span>{{ t('local.missingBadge') }}</span>
        </div>
      </div>
      <p v-if="localMissingReferences.length === 0" class="empty">{{ t('local.filterEmpty') }}</p>
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
  <Teleport to="body">
    <div v-if="clearTarget" class="modal-backdrop" role="presentation" @click.self="clearTarget = null">
      <section class="confirm-dialog" role="dialog" aria-modal="true" aria-labelledby="clear-local-title">
        <div class="dialog-head">
          <div class="dialog-icon"><Trash2 :size="22" /></div>
          <h2 id="clear-local-title">{{ clearTitle }}</h2>
        </div>
        <div class="dialog-copy">
          <p>{{ clearDesc }}</p>
        </div>
        <div class="dialog-actions">
          <button class="ghost" type="button" @click="clearTarget = null">{{ t('common.cancel') }}</button>
          <button class="danger" type="button" @click="confirmClear">
            <Trash2 :size="17" />{{ clearConfirmLabel }}
          </button>
        </div>
      </section>
    </div>
    <div v-if="pendingScan" class="modal-backdrop" role="presentation" @click.self="pendingScan = null">
      <section class="confirm-dialog" role="dialog" aria-modal="true" aria-labelledby="scan-local-title">
        <div class="dialog-head">
          <div class="dialog-icon"><FolderOpen :size="22" /></div>
          <h2 id="scan-local-title">{{ t('local.scanTitle') }}</h2>
        </div>
        <div class="dialog-copy">
          <p>{{ t('local.scanDesc', { count: pendingScan.length }) }}</p>
        </div>
        <div class="dialog-actions">
          <button class="ghost" type="button" @click="pendingScan = null">{{ t('common.cancel') }}</button>
          <button class="primary" type="button" @click="confirmScan">
            <ImagePlus :size="17" />{{ t('local.scanConfirm') }}
          </button>
        </div>
      </section>
    </div>
  </Teleport>
</section>
</template>
