<script setup lang="ts">
import { ChevronDown, Download, Eye, FileSearch, Plus, Trash2, UploadCloud, Wand2 } from 'lucide-vue-next';
import AppTooltip from '../AppTooltip.vue';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';
import { t } from '../../i18n';

const {
  loading,
  pasteForm,
  batchFiles,
  targetDropdownOpen,
  uploadDragActive,
  selectedTargetConfig,
  targetConfigs,
  convertForm,
  convertedCount,
  canConvertBatch,
  targetConfigLabel,
  selectTargetConfig,
  typeLabel,
  handleFiles,
  handleFileDrop,
  addPastedDocument,
  removeBatchFile,
  statusLabel,
  downloadFile,
  togglePreview,
  changedLines,
  analyzeBatch,
  convertBatch,
  downloadAll,
} = useWorkspaceContext();
</script>

<template>
<section class="grid convert-layout">
  <div class="panel stack">
    <div class="section-title">
      <div>
        <p class="section-kicker">Batch</p>
        <h2>{{ t('convert.title') }}</h2>
      </div>
      <label
        class="upload-control"
        :class="{ dragging: uploadDragActive }"
        @dragenter.prevent="uploadDragActive = true"
        @dragover.prevent="uploadDragActive = true"
        @dragleave.prevent="uploadDragActive = false"
        @drop.prevent="handleFileDrop"
        ><UploadCloud :size="18" /><span>{{ t('convert.uploadHint') }}</span
        ><input type="file" multiple accept=".md,text/markdown" @change="handleFiles"
      /></label>
    </div>
    <div class="paste-box">
      <label
        >{{ t('convert.pasteName') }}<input v-model.trim="pasteForm.filename" placeholder="pasted.md" /></label
      ><label
        >{{ t('convert.pasteContent') }}<textarea
          v-model="pasteForm.content"
          class="editor small-editor"
          :placeholder="t('convert.pastePlaceholder')"
        ></textarea></label
      ><button class="secondary" type="button" @click="addPastedDocument">
        <Plus :size="18" />{{ t('convert.joinQueue') }}
      </button>
    </div>
    <div class="batch-list">
      <div
        v-for="file in batchFiles"
        :key="file.id"
        class="batch-row"
        :data-status="file.status"
      >
        <div class="row-info">
          <strong>{{ file.filename }}</strong
          ><span
            >{{ statusLabel(file.status) }} ·
            {{ t('convert.imagesReplaced', { images: file.images.length, changed: file.changed }) }}</span
          ><AppTooltip v-if="file.error" :label="file.error" wrap>
            <small class="error-tip-text">{{ file.error }}</small>
          </AppTooltip>
        </div>
        <div class="row-actions">
          <button
            class="ghost icon-only"
            type="button"
            :disabled="!file.convertedContent"
            @click="togglePreview(file)"
          >
            <Eye :size="17" /></button
          ><button
            class="ghost icon-only"
            type="button"
            :disabled="!file.convertedContent"
            @click="downloadFile(file)"
          >
            <Download :size="17" /></button
          ><button class="danger icon-only" type="button" @click="removeBatchFile(file.id)">
            <Trash2 :size="17" />
          </button>
        </div>
        <div v-if="file.previewOpen" class="preview-diff">
          <div v-for="row in changedLines(file)" :key="row.line" class="diff-row">
            <span>#{{ row.line }}</span>
            <code>- {{ row.before }}</code>
            <code>+ {{ row.after }}</code>
          </div>
          <p v-if="changedLines(file).length === 0">{{ t('convert.noDiff') }}</p>
        </div>
      </div>
      <p v-if="batchFiles.length === 0" class="empty">{{ t('convert.empty') }}</p>
    </div>
  </div>
  <div class="panel stack">
    <div class="section-title">
      <div>
        <p class="section-kicker">Target</p>
        <h2>{{ t('convert.settingsTitle') }}</h2>
      </div>
    </div>
    <div class="conversion-controls">
      <label class="select-field">
        <span>{{ t('common.targetConfig') }}</span>
        <div class="custom-select" :class="{ open: targetDropdownOpen }">
          <button
            class="select-trigger"
            type="button"
            :aria-expanded="targetDropdownOpen"
            @click="targetDropdownOpen = !targetDropdownOpen"
          >
            <span :class="{ placeholder: !selectedTargetConfig }">
              {{ targetConfigLabel(selectedTargetConfig) }}
            </span>
            <ChevronDown :size="18" class="select-chevron" />
          </button>
          <div v-if="targetDropdownOpen" class="select-menu">
            <button
              class="select-option placeholder-option"
              type="button"
              :class="{ selected: !convertForm.target_config_id }"
              @click="selectTargetConfig(0)"
            >
              {{ t('common.pleaseSelect') }}
            </button>
            <button
              v-for="item in targetConfigs"
              :key="item.id"
              class="select-option"
              type="button"
              :class="{ selected: item.id === convertForm.target_config_id }"
              @click="selectTargetConfig(item.id)"
            >
              <span>{{ item.config_name }}</span>
              <small>{{ typeLabel(item.picbed_type) }}{{ item.is_default ? t('common.defaultSuffix') : '' }}</small>
            </button>
          </div>
        </div>
      </label>
      <label
        >{{ t('common.targetConfig') }}<select v-model.number="convertForm.target_config_id">
          <option :value="0">{{ t('common.pleaseSelect') }}</option>
          <option v-for="item in targetConfigs" :key="item.id" :value="item.id">
            {{ item.config_name }} · {{ typeLabel(item.picbed_type) }}
          </option>
        </select></label
      >
      <div class="actions compact-actions">
        <button class="secondary" type="button" :disabled="loading" @click="analyzeBatch">
          <FileSearch :size="18" />{{ t('convert.analyze') }}</button
        ><button
          class="primary"
          type="button"
          :disabled="loading || !canConvertBatch"
          @click="convertBatch"
        >
          <Wand2 :size="18" />{{ t('convert.start') }}
        </button>
      </div>
    </div>
    <button class="secondary" type="button" :disabled="!convertedCount" @click="downloadAll">
      <Download :size="18" />{{ t('common.downloadAll') }}
    </button>
  </div>
</section>
</template>
