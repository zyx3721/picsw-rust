<script setup lang="ts">
import { KeyRound, X } from 'lucide-vue-next';
import { computed, ref, watch } from 'vue';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';

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
        <h2 id="sync-password-title"><KeyRound :size="20" />{{ isChange ? '修改同步密码' : '设置同步密码' }}</h2>
        <button class="ghost icon-only" type="button" aria-label="关闭" @click="closePasswordModal"><X :size="18" /></button>
      </div>
      <label v-if="isChange" class="sync-dialog-field">
        <span>当前密码</span>
        <input v-model="oldPassword" type="password" placeholder="输入当前同步密码" spellcheck="false" autocomplete="off" @keyup.enter="handleSubmit" />
      </label>
      <label class="sync-dialog-field">
        <span>{{ isChange ? '新密码（至少 6 位）' : '同步密码（至少 6 位）' }}</span>
        <input v-model="password" type="password" placeholder="输入同步密码" spellcheck="false" autocomplete="new-password" @keyup.enter="handleSubmit" />
      </label>
      <label class="sync-dialog-field">
        <span>确认密码</span>
        <input v-model="confirmPassword" type="password" placeholder="再次输入同步密码" spellcheck="false" autocomplete="new-password" @keyup.enter="handleSubmit" />
      </label>
      <p v-if="error" class="sync-dialog-error">{{ error }}</p>
      <p v-else-if="confirmPassword.length > 0 && password !== confirmPassword" class="sync-dialog-error">两次输入的密码不一致</p>
      <div class="sync-dialog-note">
        云端只存密文（AES-256-GCM + PBKDF2）。密码不做任何保存——多台设备须使用相同密码；遗忘后云端数据将无法解密，只能删除同步 Gist 重来。
      </div>
      <div class="dialog-actions">
        <button class="ghost" type="button" @click="closePasswordModal">取消</button>
        <button class="primary" type="button" :disabled="!canSubmit" @click="handleSubmit">
          {{ isChange ? '修改' : '设置' }}
        </button>
      </div>
    </section>
  </div>
  </Teleport>
</template>
