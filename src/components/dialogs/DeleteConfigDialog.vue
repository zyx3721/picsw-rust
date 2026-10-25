<script setup lang="ts">
import { Trash2 } from 'lucide-vue-next';
import { t } from '../../i18n';
import { nextTick, ref, watch } from 'vue';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';

const { deleteTarget, loading, cancelDeleteConfig, confirmDeleteConfig } = useWorkspaceContext();
const confirmButton = ref<HTMLButtonElement | null>(null);

/// 弹窗打开时聚焦删除按钮，按回车即执行删除
watch(deleteTarget, open => {
  if (open) void nextTick(() => confirmButton.value?.focus());
});
</script>

<template>
  <div v-if="deleteTarget" class="modal-backdrop" role="presentation" @click.self="cancelDeleteConfig">
    <section class="confirm-dialog" role="dialog" aria-modal="true" aria-labelledby="delete-config-title">
      <div class="dialog-icon"><Trash2 :size="22" /></div>
      <div class="dialog-copy">
        <h2 id="delete-config-title">{{ t('dlg.delete.title') }}</h2>
        <p>{{ t('dlg.delete.desc', { name: deleteTarget.config_name }) }}</p>
      </div>
      <div class="dialog-actions">
        <button class="ghost" type="button" @click="cancelDeleteConfig">{{ t('common.cancel') }}</button>
        <button ref="confirmButton" class="danger" type="button" :disabled="loading" @click="confirmDeleteConfig">
          <Trash2 :size="17" />{{ t('dlg.delete.confirm') }}
        </button>
      </div>
    </section>
  </div>
</template>
