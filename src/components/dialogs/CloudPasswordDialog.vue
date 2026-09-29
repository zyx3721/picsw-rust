<script setup lang="ts">
import { Download, Eye, EyeOff, X } from 'lucide-vue-next';
import { ref, watch } from 'vue';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';

const { cloudPwdModalOpen, resolveConflictWithCloudPassword, closeCloudPwdModal } = useWorkspaceContext();

const cloudPassword = ref('');
const cloudPwdVisible = ref(false);
const submitting = ref(false);
const error = ref('');

watch(cloudPwdModalOpen, value => {
  if (value) {
    cloudPassword.value = '';
    cloudPwdVisible.value = false;
    error.value = '';
  }
});

async function handleSubmit() {
  if (!cloudPassword.value || submitting.value) return;
  submitting.value = true;
  error.value = '';
  try {
    const message = await resolveConflictWithCloudPassword(cloudPassword.value);
    if (message) {
      error.value = message;
      return;
    }
    closeCloudPwdModal();
  } finally {
    submitting.value = false;
  }
}
</script>

<template>
  <Teleport to="body">
  <div v-if="cloudPwdModalOpen" class="modal-backdrop" role="presentation" @click.self="closeCloudPwdModal">
    <section class="confirm-dialog cloud-pwd-dialog" role="dialog" aria-modal="true" aria-labelledby="cloud-pwd-title">
      <div class="sync-dialog-head">
        <h2 id="cloud-pwd-title" class="blue"><Download :size="16" />输入云端密码</h2>
        <button class="ghost icon-only" type="button" aria-label="关闭" @click="closeCloudPwdModal"><X :size="18" /></button>
      </div>
      <p class="cloud-pwd-desc">
        云端数据由另一台设备的同步密码加密。输入该密码以解密并恢复；恢复后本机的同步密码将被重置为云端密码。
      </p>
      <div class="cloud-pwd-input-row">
        <input
          v-model="cloudPassword"
          :type="cloudPwdVisible ? 'text' : 'password'"
          placeholder="云端（另一台设备的）同步密码"
          spellcheck="false"
          autocomplete="off"
          autofocus
          @keyup.enter="handleSubmit"
        />
        <button
          class="field-action"
          type="button"
          :aria-label="cloudPwdVisible ? '隐藏密码' : '显示密码'"
          @click="cloudPwdVisible = !cloudPwdVisible"
        >
          <EyeOff v-if="cloudPwdVisible" :size="18" />
          <Eye v-else :size="18" />
        </button>
      </div>
      <p v-if="error" class="sync-dialog-error">{{ error }}</p>
      <div class="dialog-actions">
        <button class="ghost" type="button" @click="closeCloudPwdModal">取消</button>
        <button class="primary blue" type="button" :disabled="submitting || !cloudPassword" @click="handleSubmit">
          {{ submitting ? '解密中…' : '解密并恢复' }}
        </button>
      </div>
    </section>
  </div>
  </Teleport>
</template>
