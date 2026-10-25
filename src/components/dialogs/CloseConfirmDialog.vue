<script setup lang="ts">
import { ref, watch } from 'vue';
import { X } from 'lucide-vue-next';
import { t } from '../../i18n';
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
      <button class="close-dialog-dismiss" type="button" :aria-label="t('dlg.close.cancelAria')" @click="emit('cancel')">
        <X :size="18" />
      </button>
      <div class="close-dialog-head">
        <h2 id="close-confirm-title">{{ t('dlg.close.title') }}</h2>
      </div>
      <div class="dialog-copy">
        <p>{{ t('dlg.close.desc') }}</p>
      </div>
      <label class="checkbox close-dialog-remember">
        <input v-model="remember" type="checkbox" />
        {{ t('dlg.close.remember') }}
      </label>
      <div class="dialog-actions close-confirm-actions">
        <button class="danger" type="button" @click="decide('quit')">{{ t('about.closeQuit') }}</button>
        <button class="success-action" type="button" @click="decide('tray')">{{ t('about.closeTray') }}</button>
      </div>
    </section>
  </div>
</template>
