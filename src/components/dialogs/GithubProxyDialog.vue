<script setup lang="ts">
import { FileSearch, Wand2 } from 'lucide-vue-next';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';
import { t } from '../../i18n';

const {
  githubProxyDialogOpen,
  githubProxyEnabled,
  githubProxyURL,
  loading,
  closeGithubProxyDialog,
  confirmGithubProxyConvert,
} = useWorkspaceContext();
</script>

<template>
  <div v-if="githubProxyDialogOpen" class="modal-backdrop" role="presentation" @click.self="closeGithubProxyDialog">
    <section class="confirm-dialog github-proxy-dialog" role="dialog" aria-modal="true" aria-labelledby="github-proxy-title">
      <div class="dialog-icon github-proxy-icon"><FileSearch :size="22" /></div>
      <div class="dialog-copy">
        <h2 id="github-proxy-title">{{ t('dlg.proxy.title') }}</h2>
        <p>{{ t('dlg.proxy.desc') }}</p>
      </div>
      <label class="checkbox proxy-toggle"><input v-model="githubProxyEnabled" type="checkbox" />{{ t('dlg.proxy.enable') }}</label>
      <label v-if="githubProxyEnabled" class="proxy-url-field">
        <span class="field-label-text">{{ t('dlg.proxy.urlLabel') }}</span>
        <input v-model.trim="githubProxyURL" placeholder="https://gh-proxy.com/" />
      </label>
      <div class="dialog-actions">
        <button class="ghost" type="button" @click="closeGithubProxyDialog">{{ t('common.cancel') }}</button>
        <button class="primary" type="button" :disabled="loading" @click="confirmGithubProxyConvert">
          <Wand2 :size="17" />{{ loading ? t('dlg.proxy.converting') : t('dlg.proxy.continue') }}
        </button>
      </div>
    </section>
  </div>
</template>
