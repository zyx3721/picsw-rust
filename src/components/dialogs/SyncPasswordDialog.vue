<script setup lang="ts">
import { KeyRound, X } from 'lucide-vue-next';
import { computed, ref, watch } from 'vue';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';
import { t } from '../../i18n';

const props = defineProps<{ open: boolean }>();

const { passwordModalMode, submitPasswordModal, closePasswordModal } = useWorkspaceContext();

const oldPassword = ref('');
const password = ref('');
const confirmPassword = ref('');
const error = ref('');

const isChange = computed(() => passwordModalMode.value === 'change');
const canSubmit = computed(() => {
  if (password.value.length < 6) return false;
  if (isChange.value && !oldPassword.value) return false;
  return password.value === confirmPassword.value;
});

watch(() => props.open, value => {
  if (value) {
    oldPassword.value = '';
    password.value = '';
    confirmPassword.value = '';
    error.value = '';
  }
});

async function handleSubmit() {
  if (!canSubmit.value) return;
  const message = await submitPasswordModal(passwordModalMode.value, oldPassword.value, password.value);
  if (message) error.value = message;
}
</script>

<template>
  <Teleport to="body">
  <div v-if="open" class="modal-backdrop" role="presentation" @click.self="closePasswordModal">
    <section class="confirm-dialog sync-password-dialog" role="dialog" aria-modal="true" aria-labelledby="sync-password-title">
      <div class="sync-dialog-head">
        <h2 id="sync-password-title"><KeyRound :size="20" />{{ isChange ? t('dlg.syncPwd.changeTitle') : t('dlg.syncPwd.setTitle') }}</h2>
        <button class="ghost icon-only" type="button" :aria-label="t('common.close')" @click="closePasswordModal"><X :size="18" /></button>
      </div>
      <label v-if="isChange" class="sync-dialog-field">
        <span>{{ t('dlg.syncPwd.currentLabel') }}</span>
        <input v-model="oldPassword" type="password" :placeholder="t('dlg.syncPwd.currentPlaceholder')" spellcheck="false" autocomplete="off" @keyup.enter="handleSubmit" />
      </label>
      <label class="sync-dialog-field">
        <span>{{ isChange ? t('dlg.syncPwd.newChangeLabel') : t('dlg.syncPwd.newSetLabel') }}</span>
        <input v-model="password" type="password" :placeholder="t('dlg.syncPwd.newPlaceholder')" spellcheck="false" autocomplete="new-password" @keyup.enter="handleSubmit" />
      </label>
      <label class="sync-dialog-field">
        <span>{{ t('dlg.syncPwd.confirmLabel') }}</span>
        <input v-model="confirmPassword" type="password" :placeholder="t('dlg.syncPwd.confirmPlaceholder')" spellcheck="false" autocomplete="new-password" @keyup.enter="handleSubmit" />
      </label>
      <p v-if="error" class="sync-dialog-error">{{ error }}</p>
      <p v-else-if="confirmPassword.length > 0 && password !== confirmPassword" class="sync-dialog-error">{{ t('dlg.syncPwd.mismatch') }}</p>
      <div class="sync-dialog-note">{{ t('dlg.syncPwd.note') }}</div>
      <div class="dialog-actions">
        <button class="ghost" type="button" @click="closePasswordModal">{{ t('common.cancel') }}</button>
        <button class="primary" type="button" :disabled="!canSubmit" @click="handleSubmit">
          {{ isChange ? t('dlg.syncPwd.submitChange') : t('dlg.syncPwd.submitSet') }}
        </button>
      </div>
    </section>
  </div>
  </Teleport>
</template>
