<script setup lang="ts">
import { Cloud, ExternalLink, X } from 'lucide-vue-next';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';
import { t } from '../../i18n';

const {
  syncPatInput,
  syncRemoteState,
  githubConnectOpen,
  deviceFlowUserCode,
  deviceFlowStatus,
  deviceFlowError,
  deviceFlowAvailable,
  patMode,
  cancelGithubConnect,
  openVerificationPage,
  copyUserCode,
  connectGithub,
} = useWorkspaceContext();
</script>

<template>
  <Teleport to="body">
  <div v-if="githubConnectOpen" class="modal-backdrop" role="presentation" @click.self="cancelGithubConnect">
    <section class="confirm-dialog github-connect-dialog" role="dialog" aria-modal="true" aria-labelledby="github-connect-title">
      <div class="sync-dialog-head">
        <h2 id="github-connect-title"><Cloud :size="20" />{{ t('sync.connect') }}</h2>
        <button class="ghost icon-only" type="button" :aria-label="t('common.close')" @click="cancelGithubConnect"><X :size="18" /></button>
      </div>

      <template v-if="!patMode">
        <template v-if="deviceFlowAvailable">
          <p class="connect-step">{{ t('dlg.connect.step1') }}</p>
          <div class="connect-code-row">
            <span class="connect-user-code" data-status="pending">{{ deviceFlowUserCode || '····-····' }}</span>
            <button class="secondary" type="button" :disabled="!deviceFlowUserCode" @click="openVerificationPage">
              <ExternalLink :size="16" />{{ t('dlg.connect.openDevice') }}
            </button>
          </div>
          <p class="connect-step">{{ t('dlg.connect.step2') }}</p>
          <p v-if="deviceFlowStatus === 'pending'" class="connect-waiting"><span class="connect-spinner" />{{ t('dlg.connect.waiting') }}</p>
          <p v-if="deviceFlowStatus === 'connected'" class="connect-done">{{ t('dlg.connect.fetching') }}</p>
          <p v-if="deviceFlowStatus === 'error'" class="sync-dialog-error">{{ deviceFlowError }}</p>
        </template>
        <p v-else class="sync-dialog-note">{{ t('dlg.connect.noClientId') }}</p>
      </template>

      <template v-else>
        <label class="sync-dialog-field">
          <span>{{ t('dlg.connect.patLabel') }}</span>
          <input
            v-model="syncPatInput"
            type="password"
            :placeholder="t('dlg.connect.patPlaceholder')"
            spellcheck="false"
            autocomplete="off"
          />
        </label>
      </template>

      <div class="dialog-actions" :class="{ 'connect-fallback-row': !patMode && !deviceFlowAvailable }">
        <button
          v-if="!patMode && !deviceFlowAvailable"
          class="secondary"
          type="button"
          @click="patMode = true"
        >{{ t('dlg.connect.usePat') }}</button>
        <button class="ghost" type="button" @click="cancelGithubConnect">{{ t('common.cancel') }}</button>
        <button
          v-if="patMode"
          class="primary"
          type="button"
          :disabled="!syncPatInput.trim()"
          @click="connectGithub"
        >{{ t('sync.connect') }}</button>
        <button
          v-else-if="deviceFlowUserCode"
          class="primary"
          type="button"
          @click="copyUserCode"
        >{{ t('dlg.connect.copyCode') }}</button>
      </div>
      <p v-if="syncRemoteState" class="sync-dialog-note">{{ t('dlg.connect.remoteNote', { version: syncRemoteState.version }) }}</p>
    </section>
  </div>
  </Teleport>
</template>
