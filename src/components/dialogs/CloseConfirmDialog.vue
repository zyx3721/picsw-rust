<script setup lang="ts">
import { ref, watch } from 'vue';
import { X } from 'lucide-vue-next';
import type { CloseAction } from '../../composables/workspace/closeBehavior';

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{
  decide: [action: Exclude<CloseAction, 'ask'>, remember: boolean];
  cancel: [];
}>();

const remember = ref(true);

watch(
  () => props.open,
  open => {
    if (open) remember.value = true;
  }
);

/// 回传本次选择与是否记住，由父组件负责执行退出或最小化
function decide(action: Exclude<CloseAction, 'ask'>) {
  emit('decide', action, remember.value);
}
</script>

<template>
  <div v-if="open" class="modal-backdrop close-confirm-backdrop" role="presentation" @click.self="emit('cancel')">
    <section class="confirm-dialog close-confirm-dialog" role="dialog" aria-modal="true" aria-labelledby="close-confirm-title">
      <button class="close-dialog-dismiss" type="button" aria-label="取消关闭" @click="emit('cancel')">
        <X :size="18" />
      </button>
      <div class="close-dialog-head">
        <h2 id="close-confirm-title">关闭应用</h2>
      </div>
      <div class="dialog-copy">
        <p>最小化到托盘后应用在后台运行，转换任务不受影响，可随时从托盘恢复窗口或退出。</p>
      </div>
      <label class="checkbox close-dialog-remember">
        <input v-model="remember" type="checkbox" />
        记住我的选择（可随时在「关于 → 关闭窗口时」中修改）
      </label>
      <div class="dialog-actions close-confirm-actions">
        <button class="danger" type="button" @click="decide('quit')">退出应用</button>
        <button class="success-action" type="button" @click="decide('tray')">最小化到托盘</button>
      </div>
    </section>
  </div>
</template>
