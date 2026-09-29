<script setup lang="ts">
import { CheckCircle2, LoaderCircle, XCircle } from 'lucide-vue-next';
import { computed } from 'vue';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';

const { taskProgress, closeTaskProgress } = useWorkspaceContext();

const progressPercent = computed(() => {
  if (!taskProgress.total) return 0;
  return Math.min(100, Math.round((taskProgress.current / taskProgress.total) * 100));
});
const isRunning = computed(() => taskProgress.status === 'running');
</script>

<template>
  <div v-if="taskProgress.open" class="modal-backdrop task-progress-backdrop" role="presentation">
    <section class="confirm-dialog task-progress-dialog" role="dialog" aria-modal="true" aria-labelledby="task-progress-title">
      <div class="dialog-icon task-progress-icon" :data-status="taskProgress.status">
        <LoaderCircle v-if="isRunning" :size="24" class="spin-icon" />
        <CheckCircle2 v-else-if="taskProgress.status === 'success'" :size="24" />
        <XCircle v-else :size="24" />
      </div>
      <div class="dialog-copy">
        <h2 id="task-progress-title">{{ taskProgress.title }}</h2>
        <p>{{ taskProgress.message }}</p>
        <p v-if="taskProgress.detail">{{ taskProgress.detail }}</p>
      </div>
      <div class="task-progress-meter" aria-hidden="true">
        <span :style="{ width: progressPercent + '%' }"></span>
      </div>
      <div class="task-progress-stats">
        <span>进度 {{ taskProgress.current }} / {{ taskProgress.total }}</span>
        <span>成功 {{ taskProgress.success }}</span>
        <span>失败 {{ taskProgress.failed }}</span>
      </div>
      <p v-if="isRunning" class="task-progress-warning">任务在后台执行，关闭窗口不会中断进度。</p>
      <div v-else class="dialog-actions">
        <button class="primary" type="button" @click="closeTaskProgress">关闭</button>
      </div>
    </section>
  </div>
</template>
