<script setup lang="ts">
import { Trash2 } from 'lucide-vue-next';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';

const { deleteTarget, loading, cancelDeleteConfig, confirmDeleteConfig } = useWorkspaceContext();
</script>

<template>
  <div v-if="deleteTarget" class="modal-backdrop" role="presentation" @click.self="cancelDeleteConfig">
    <section class="confirm-dialog" role="dialog" aria-modal="true" aria-labelledby="delete-config-title">
      <div class="dialog-icon"><Trash2 :size="22" /></div>
      <div class="dialog-copy">
        <h2 id="delete-config-title">删除图床配置</h2>
        <p>确认删除“{{ deleteTarget.config_name }}”吗？删除后无法用于后续转换。</p>
      </div>
      <div class="dialog-actions">
        <button class="ghost" type="button" @click="cancelDeleteConfig">取消</button>
        <button class="danger" type="button" :disabled="loading" @click="confirmDeleteConfig">
          <Trash2 :size="17" />删除
        </button>
      </div>
    </section>
  </div>
</template>
