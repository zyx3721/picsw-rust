<script setup lang="ts">
import { Ban, CheckCircle2, CircleAlert, LoaderCircle, XCircle } from 'lucide-vue-next';
import { computed } from 'vue';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';
import type { TaskDocumentProgress } from '../../composables/workspace/types';
import { t } from '../../i18n';

const { taskProgress, closeTaskProgress, resumeTaskProgress, cancelTaskProgress } = useWorkspaceContext();

const progressPercent = computed(() => {
  const totalImages = taskProgress.documents.reduce((sum, document) => sum + document.total_images, 0);
  const processedImages = taskProgress.documents.reduce((sum, document) => sum + document.processed_images, 0);
  if (totalImages > 0) return Math.min(100, Math.round((processedImages / totalImages) * 100));
  if (!taskProgress.total) return 0;
  return Math.min(100, Math.round((taskProgress.current / taskProgress.total) * 100));
});
const isRunning = computed(() => taskProgress.status === 'running');
const supportsTaskActions = computed(
  () => taskProgress.taskKind === 'convert' || taskProgress.taskKind === 'localUpload' || taskProgress.taskKind === 'download'
);
const cancellable = computed(
  () => supportsTaskActions.value && (taskProgress.status === 'running' || taskProgress.status === 'interrupted')
);
const showFooter = computed(() => supportsTaskActions.value || !isRunning.value);
const resumeLabel = computed(() => {
  if (taskProgress.taskKind === 'convert') return t('dlg.task.resumeConvert');
  if (taskProgress.taskKind === 'localUpload') return t('dlg.task.resumeUpload');
  if (taskProgress.taskKind === 'download') return t('dlg.task.resumeDownload');
  return t('dlg.task.resume');
});
const cancelLabel = computed(() => {
  if (taskProgress.taskKind === 'convert') return t('dlg.task.cancelConvert');
  if (taskProgress.taskKind === 'localUpload') return t('dlg.task.cancelUpload');
  if (taskProgress.taskKind === 'download') return t('dlg.task.cancelDownload');
  return t('dlg.task.cancel');
});
const runningDocumentCount = computed(
  () => taskProgress.documents.filter(document => document.status === 'running').length
);

function documentPercent(document: TaskDocumentProgress) {
  if (document.status === 'success') return 100;
  if (!document.total_images) return 0;
  return Math.min(100, Math.round((document.processed_images / document.total_images) * 100));
}

function documentStatusLabel(status: TaskDocumentProgress['status']) {
  return {
    queued: t('dlg.task.docQueued'),
    running: t('dlg.task.docRunning'),
    success: t('dlg.task.docSuccess'),
    partial: t('dlg.task.docPartial'),
    failed: t('dlg.task.docFailed'),
    interrupted: t('dlg.task.docInterrupted'),
    cancelled: t('dlg.task.docCancelled'),
  }[status];
}

function documentCountLabel(document: TaskDocumentProgress) {
  return t('dlg.task.docImages', { done: document.processed_images, total: document.total_images });
}
</script>

<template>
  <div v-if="taskProgress.open" class="modal-backdrop task-progress-backdrop" role="presentation">
    <section class="confirm-dialog task-progress-dialog" role="dialog" aria-modal="true" aria-labelledby="task-progress-title">
      <div class="dialog-head">
        <div class="dialog-icon task-progress-icon" :data-status="taskProgress.status">
          <LoaderCircle v-if="isRunning" :size="24" class="spin-icon" />
          <CheckCircle2 v-else-if="taskProgress.status === 'success'" :size="24" />
          <CircleAlert v-else-if="taskProgress.status === 'interrupted'" :size="24" />
          <Ban v-else-if="taskProgress.status === 'cancelled'" :size="24" />
          <XCircle v-else :size="24" />
        </div>
        <h2 id="task-progress-title">{{ taskProgress.title }}</h2>
      </div>
      <div class="dialog-copy">
        <p>{{ taskProgress.message }}</p>
        <p v-if="taskProgress.detail">{{ taskProgress.detail }}</p>
      </div>
      <div class="task-progress-meter" aria-hidden="true">
        <span :style="{ width: progressPercent + '%' }"></span>
      </div>
      <div class="task-progress-stats">
        <span>{{ t('dlg.task.progress', { current: taskProgress.current, total: taskProgress.total }) }}</span>
        <span>{{ t('dlg.task.successCount', { count: taskProgress.success }) }}</span>
        <span>{{ t('dlg.task.partialCount', { count: taskProgress.partial }) }}</span>
        <span>{{ t('dlg.task.failedCount', { count: taskProgress.failed }) }}</span>
      </div>
      <div v-if="taskProgress.documents.length" class="task-progress-docs" aria-live="polite">
        <div class="task-progress-docs-head">
          <span>{{ t('dlg.task.docsTitle') }}</span>
          <span v-if="runningDocumentCount > 0" class="task-progress-docs-running">
            {{ t('dlg.task.docsRunning', { count: runningDocumentCount }) }}
          </span>
        </div>
        <div class="task-progress-doc-list">
          <div
            v-for="document in taskProgress.documents"
            :key="document.id"
            class="task-progress-doc"
            :data-status="document.status"
          >
            <div class="task-progress-doc-head">
              <span class="task-progress-doc-name" :title="document.filename">{{ document.filename }}</span>
              <span class="task-progress-doc-status">{{ documentStatusLabel(document.status) }}</span>
              <span class="task-progress-doc-count">{{ documentCountLabel(document) }}</span>
            </div>
            <div class="task-progress-doc-meter" aria-hidden="true">
              <span :style="{ width: documentPercent(document) + '%' }"></span>
            </div>
          </div>
        </div>
      </div>
      <p v-if="isRunning" class="task-progress-warning">{{ t('dlg.task.backgroundHint') }}</p>
      <div v-if="showFooter" class="dialog-actions task-progress-actions">
        <template v-if="supportsTaskActions">
          <button class="success-action" type="button" :disabled="!taskProgress.resumable" @click="resumeTaskProgress">
            {{ resumeLabel }}
          </button>
          <button class="danger" type="button" :disabled="!cancellable" @click="cancelTaskProgress">
            {{ cancelLabel }}
          </button>
        </template>
        <button v-if="!isRunning" class="primary" type="button" @click="closeTaskProgress">{{ t('common.close') }}</button>
      </div>
    </section>
  </div>
</template>
