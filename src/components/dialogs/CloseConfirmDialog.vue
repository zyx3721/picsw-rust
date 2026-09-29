<script setup lang="ts">
import { X } from 'lucide-vue-next';

defineProps<{ open: boolean }>();
const emit = defineEmits<{ 'confirm-exit': []; 'hide-to-tray': []; cancel: [] }>();
</script>

<template>
  <div v-if="open" class="modal-backdrop close-confirm-backdrop" role="presentation" @click.self="emit('cancel')">
    <section class="confirm-dialog close-confirm-dialog" role="dialog" aria-modal="true" aria-labelledby="close-confirm-title">
      <div class="close-dialog-head">
        <div class="dialog-icon"><X :size="20" /></div>
        <h2 id="close-confirm-title">关闭应用</h2>
      </div>
      <div class="dialog-copy">
        <p>要退出应用，还是隐藏到系统托盘继续运行？后台转换任务在隐藏后仍会继续执行。</p>
      </div>
      <div class="dialog-actions close-confirm-actions">
        <button class="ghost" type="button" @click="emit('cancel')">取消</button>
        <button class="success-action" type="button" @click="emit('hide-to-tray')">隐藏</button>
        <button class="danger" type="button" @click="emit('confirm-exit')">退出</button>
      </div>
    </section>
  </div>
</template>
