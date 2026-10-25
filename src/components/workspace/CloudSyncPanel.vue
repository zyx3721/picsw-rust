<script setup lang="ts">
import { AlertTriangle, Cloud, Database, Download, History, KeyRound, Lock, RefreshCw, Trash2, Upload, X, Zap } from 'lucide-vue-next';
import AppTooltip from '../AppTooltip.vue';
import SyncPasswordDialog from '../dialogs/SyncPasswordDialog.vue';
import GithubConnectDialog from '../dialogs/GithubConnectDialog.vue';
import CloudPasswordDialog from '../dialogs/CloudPasswordDialog.vue';
import { computed, ref, watch } from 'vue';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';
import { t } from '../../i18n';

const {
  syncAccount,
  secretBackend,
  syncBusy,
  syncConnected,
  syncTokenInvalid,
  localConfigCount,
  syncStatusBadge,
  remoteVersionBadge,
  lastSyncText,
  canSyncNow,
  syncNow,
  syncBanner,
  overrideCloudWithLocal,
  pushEmptyVaultLocal,
  restoreRemoteVersion,
  forcePushLocal,
  showError,
  passwordStatus,
  passwordBackend,
  lockSyncPassword,
  passwordModalOpen,
  openPasswordModal,
  openCloudPwdModal,
  unlockWithPassword,
  openGithubConnect,
  revalidateConnection,
  githubConnectPreparing,
  autoSyncDisplayChecked,
  autoSyncToggleEnabled,
  autoSyncToggleTitle,
  setAutoSyncEnabled,
  disconnectGithub,
  syncHistoryOpen,
  syncHistoryLoading,
  syncHistory,
  selectedRevisionSha,
  revisionPreview,
  revisionPreviewLoading,
  revisionError,
  restoringRevision,
  toggleSyncHistory,
  selectRevision,
  restoreRevision,
} = useWorkspaceContext();

const unlockInput = ref('');
const restoreConfirmArmed = ref(false);

/// 恢复预览：所选修订配置数少于当前时返回缩减条数（用于红色标注）
const revisionShrink = computed(() => {
  const preview = revisionPreview.value;
  if (!preview) return 0;
  return Math.max(preview.currentCount - preview.configCount, 0);
});

watch(syncHistoryOpen, open => {
  if (!open) restoreConfirmArmed.value = false;
});

/// 选择修订即解除恢复确认态
function handleSelectRevision(sha: string) {
  restoreConfirmArmed.value = false;
  void selectRevision(sha);
}

/// 恢复按钮两步确认：首次点击进入确认态，再次点击才执行恢复
async function handleRestoreClick() {
  if (!restoreConfirmArmed.value) {
    restoreConfirmArmed.value = true;
    return;
  }
  restoreConfirmArmed.value = false;
  await restoreRevision();
}

/// 行内解锁提交：失败经悬浮提示报错，成功清空输入
function submitUnlock() {
  if (!unlockInput.value) return;
  const message = unlockWithPassword(unlockInput.value);
  if (message) {
    showError(message);
    return;
  }
  unlockInput.value = '';
}
</script>

<template>
<section class="sync-layout">
  <div class="panel stack">
    <div class="section-title">
      <div>
        <p class="section-kicker">Sync</p>
        <h2>{{ t('sync.title') }}</h2>
      </div>
      <span class="sync-badge">{{ t('sync.subtitle') }}</span>
    </div>

    <div v-if="syncBanner" class="sync-banner" :class="`banner-${syncBanner.kind}`">
      <div class="sync-banner-body">
        <span class="sync-banner-text">
          <AlertTriangle v-if="syncBanner.kind !== 'conflict'" :size="15" class="sync-banner-alert-icon" />{{ syncBanner.message }}
        </span>
        <span v-if="syncBanner.kind === 'conflict'" class="sync-banner-detail">
          {{ t('sync.conflictDetailA', { remoteVersion: syncBanner.remoteVersion }) }}<template v-if="syncBanner.remoteDeviceName">{{ t('sync.conflictDetailDevice', { device: syncBanner.remoteDeviceName }) }}</template>{{ t('sync.conflictDetailB', { localVersion: syncBanner.localVersion ?? 0 }) }}
        </span>
        <span v-if="syncBanner.kind === 'empty'" class="sync-banner-detail">
          {{ t('sync.emptyDetail', { count: syncBanner.remoteItems.length }) }}
        </span>
        <span v-if="syncBanner.kind === 'blocked'" class="sync-banner-detail">{{ syncBanner.detail }}</span>
      </div>
      <div class="sync-banner-actions">
        <button
          v-if="syncBanner.kind === 'conflict'"
          class="secondary"
          type="button"
          :disabled="syncBusy"
          @click="openCloudPwdModal"
        >
          <Download :size="16" />{{ t('sync.useCloud') }}
        </button>
        <button
          v-if="syncBanner.kind === 'conflict'"
          class="warn"
          type="button"
          :disabled="syncBusy"
          @click="overrideCloudWithLocal"
        >{{ t('sync.useLocal') }}</button>
        <button
          v-if="syncBanner.kind === 'blocked'"
          class="secondary"
          type="button"
          :disabled="syncBusy"
          @click="restoreRemoteVersion"
        >
          <Download :size="16" />{{ t('sync.restoreCloud') }}
        </button>
        <button
          v-if="syncBanner.kind === 'blocked'"
          class="danger"
          type="button"
          :disabled="syncBusy"
          @click="forcePushLocal"
        >
          <Upload :size="16" />{{ t('sync.forcePush') }}
        </button>
        <button
          v-if="syncBanner.kind === 'empty'"
          class="secondary"
          type="button"
          :disabled="syncBusy"
          @click="restoreRemoteVersion"
        >
          <Download :size="16" />{{ t('sync.restoreCloud') }}
        </button>
        <button
          v-if="syncBanner.kind === 'empty'"
          class="info"
          type="button"
          :disabled="syncBusy"
          @click="pushEmptyVaultLocal"
        >
          <Upload :size="16" />{{ t('sync.pushLocal') }}
        </button>
      </div>
    </div>

    <div class="sync-row">
      <div class="sync-row-info">
        <span class="sync-row-icon"><Cloud :size="20" /></span>
        <div>
          <strong>GitHub Gist</strong>
          <span v-if="syncConnected && syncTokenInvalid">{{ t('sync.tokenExpiredHint') }}</span>
          <span v-else-if="syncConnected">{{ t('sync.accountHint', { account: syncAccount, backend: secretBackend === 'file' ? t('sync.backendFile') : t('sync.backendKeychain') }) }}</span>
          <span v-else>{{ t('sync.notConnectedHint') }}</span>
        </div>
      </div>
      <div class="sync-row-actions">
        <template v-if="!syncConnected">
          <button
            class="secondary"
            type="button"
            :disabled="syncBusy || githubConnectPreparing"
            @click="openGithubConnect"
          >
            <RefreshCw v-if="githubConnectPreparing" :size="17" class="spin-icon" />
            <Cloud v-else :size="17" />{{ t('sync.connect') }}
          </button>
        </template>
        <template v-else-if="syncTokenInvalid">
          <span class="sync-connected-tag invalid">{{ t('sync.tokenInvalidTag') }}</span>
          <AppTooltip :label="t('sync.revalidateTip')" center>
            <button
              class="secondary"
              type="button"
              :disabled="syncBusy"
              @click="revalidateConnection"
            >
              <KeyRound :size="17" />{{ t('sync.revalidate') }}
            </button>
          </AppTooltip>
          <button
            class="secondary"
            type="button"
            :disabled="syncBusy || githubConnectPreparing"
            @click="openGithubConnect"
          >
            <RefreshCw v-if="githubConnectPreparing" :size="17" class="spin-icon" />
            <Cloud v-else :size="17" />{{ t('sync.reconnect') }}
          </button>
          <AppTooltip :label="t('sync.disconnectTip')" center>
            <button class="ghost sync-disconnect" type="button" :disabled="syncBusy" @click="disconnectGithub">
              <Trash2 :size="17" />{{ t('sync.disconnect') }}
            </button>
          </AppTooltip>
        </template>
        <template v-else>
          <span class="sync-connected-tag">{{ t('sync.connectedTag') }}</span>
          <AppTooltip :label="t('sync.disconnectTip')" center>
            <button class="ghost sync-disconnect" type="button" :disabled="syncBusy" @click="disconnectGithub">
              <Trash2 :size="17" />{{ t('sync.disconnect') }}
            </button>
          </AppTooltip>
        </template>
      </div>
    </div>

    <div class="sync-row">
      <div class="sync-row-info">
        <span class="sync-row-icon"><KeyRound :size="20" /></span>
        <div>
          <strong>{{ t('sync.passwordTitle') }}</strong>
          <span v-if="passwordStatus === 'none'">{{ t('sync.passwordNoneHint') }}</span>
          <span v-else-if="passwordStatus === 'locked'">{{ t('sync.passwordLockedHint') }}</span>
          <span v-else-if="passwordBackend">{{ t('sync.passwordRememberedHint') }}</span>
          <span v-else>{{ t('sync.passwordMemoryHint') }}</span>
        </div>
      </div>
      <div class="sync-row-actions">
        <template v-if="passwordStatus === 'none'">
          <span class="sync-state-tag warn">{{ t('sync.noPasswordTag') }}</span>
          <button class="secondary" type="button" :disabled="syncBusy" @click="openPasswordModal('set')">
            <KeyRound :size="17" />{{ t('sync.setPassword') }}
          </button>
        </template>
        <template v-else-if="passwordStatus === 'locked'">
          <span class="sync-state-tag muted">{{ t('sync.lockedTag') }}</span>
          <div class="sync-unlock-row">
            <input
              v-model="unlockInput"
              type="password"
              :placeholder="t('sync.passwordPlaceholder')"
              spellcheck="false"
              autocomplete="off"
              @keyup.enter="submitUnlock"
            />
            <button class="primary" type="button" :disabled="!unlockInput" @click="submitUnlock">
              <Lock :size="17" />{{ t('sync.unlock') }}
            </button>
          </div>
        </template>
        <template v-else>
          <span class="sync-state-tag ok">{{ t('sync.unlockedTag') }}</span>
          <AppTooltip :label="t('sync.changePasswordTip')" center>
            <button class="secondary" type="button" :disabled="syncBusy" @click="openPasswordModal('change')">
              <KeyRound :size="17" />{{ t('sync.changePassword') }}
            </button>
          </AppTooltip>
          <AppTooltip :label="t('sync.lockTip')" center>
            <button class="secondary" type="button" :disabled="syncBusy" @click="lockSyncPassword">
              <Lock :size="17" />{{ t('sync.lock') }}
            </button>
          </AppTooltip>
        </template>
      </div>
    </div>

    <div class="sync-row">
      <div class="sync-row-info">
        <span class="sync-row-icon"><Database :size="20" /></span>
        <div>
          <strong>{{ t('sync.contentTitle') }}</strong>
          <span>{{ t('sync.contentHint') }}</span>
        </div>
      </div>
      <span class="sync-count-tag">{{ t('sync.localCount', { count: localConfigCount }) }}</span>
    </div>

    <div class="sync-row">
      <div class="sync-row-info">
        <span class="sync-row-icon"><Zap :size="20" /></span>
        <div>
          <strong>{{ t('sync.autoTitle') }}</strong>
          <span>{{ t('sync.autoHint') }}</span>
        </div>
      </div>
      <label
        class="sync-toggle"
        :title="autoSyncToggleTitle"
        :class="{ disabled: !autoSyncToggleEnabled }"
      >
        <input
          type="checkbox"
          :checked="autoSyncDisplayChecked"
          :disabled="!autoSyncToggleEnabled"
          @change="setAutoSyncEnabled(($event.target as HTMLInputElement).checked)"
        />
        <span class="sync-toggle-track" aria-hidden="true"></span>
      </label>
    </div>

    <div class="sync-row">
      <div class="sync-row-info">
        <span class="sync-row-icon"><RefreshCw :size="20" /></span>
        <div class="sync-status-line">
          <div class="sync-status-title">
            <strong>{{ t('sync.statusTitle') }}</strong>
            <span class="sync-status-badge" :data-kind="syncStatusBadge.kind">{{ syncStatusBadge.text }}</span>
            <span v-if="remoteVersionBadge" class="sync-status-badge version">{{ remoteVersionBadge }}</span>
          </div>
          <span class="sync-status-sub">{{ lastSyncText }}</span>
        </div>
      </div>
      <div class="sync-row-actions">
        <button class="secondary" type="button" :disabled="!canSyncNow" @click="toggleSyncHistory">
          <History :size="17" />{{ t('sync.historyVersions') }}
        </button>
        <button class="primary" type="button" :disabled="!canSyncNow" @click="syncNow()">
          <RefreshCw :size="17" :class="{ 'spin-icon': syncBusy }" />{{ syncBusy ? t('sync.syncing') : t('sync.now') }}
        </button>
      </div>
    </div>

    <Teleport to="body">
      <div v-if="syncHistoryOpen" class="modal-backdrop" role="presentation" @click.self="toggleSyncHistory">
        <section class="confirm-dialog sync-history-dialog" role="dialog" aria-modal="true" aria-labelledby="sync-history-title">
          <div class="sync-dialog-head">
            <h2 id="sync-history-title"><History :size="20" />{{ t('sync.historyVersions') }}</h2>
            <button class="ghost icon-only" type="button" :aria-label="t('common.close')" @click="toggleSyncHistory"><X :size="18" /></button>
          </div>
          <p class="sync-history-desc">
            {{ t('syncHistory.descHead') }}<strong>{{ t('syncHistory.descEm') }}</strong>{{ t('syncHistory.descTail') }}
          </p>
          <div class="sync-history-list">
            <p v-if="syncHistoryLoading" class="sync-history-hint">
              <RefreshCw :size="14" class="spin-icon" />{{ t('syncHistory.loading') }}
            </p>
            <p v-else-if="syncHistory.length === 0" class="sync-history-hint">{{ t('syncHistory.empty') }}</p>
            <template v-else>
              <div class="sync-history-items">
                <button
                  v-for="item in syncHistory"
                  :key="item.sha"
                  type="button"
                  :class="['sync-history-item', { selected: selectedRevisionSha === item.sha }]"
                  :disabled="restoringRevision"
                  @click="handleSelectRevision(item.sha)"
                >
                  <span class="sync-history-item-main">
                    <span class="sync-history-item-title">
                      <strong>{{ item.label }}</strong>
                      <span v-if="item.versionBadge" class="sync-status-badge version">{{ item.versionBadge }}</span>
                    </span>
                    <small>{{ item.dateText }}</small>
                  </span>
                  <span class="sync-history-item-sha">{{ item.sha.slice(0, 7) }}</span>
                </button>
              </div>
              <p v-if="revisionPreviewLoading" class="sync-history-decrypting">
                <RefreshCw :size="14" class="spin-icon" />{{ t('syncHistory.decrypting') }}
              </p>
              <div v-else-if="revisionPreview" class="sync-history-preview-card">
                <p class="sync-history-preview-title">
                  <AlertTriangle :size="14" />{{ t('syncHistory.previewTitle') }}
                </p>
                <p>
                  {{ t('syncHistory.previewConfigs', { current: revisionPreview.currentCount, target: revisionPreview.configCount }) }}<template v-if="revisionShrink > 0"><span class="sync-preview-shrink">{{ t('syncHistory.previewShrink', { count: revisionShrink }) }}</span></template>
                </p>
                <p>
                  {{ t('syncHistory.previewSource', { version: revisionPreview.version }) }}<template v-if="revisionPreview.deviceName"> · {{ revisionPreview.deviceName }}</template><template v-if="revisionPreview.appVersion">（{{ revisionPreview.appVersion }}）</template> · {{ t('syncHistory.previewDataTime', { time: revisionPreview.updatedAtText }) }}
                </p>
                <p class="sync-history-preview-note">{{ t('syncHistory.previewNote') }}</p>
              </div>
              <p v-else-if="revisionError" class="sync-history-error-card">{{ revisionError }}</p>
            </template>
          </div>
          <div class="dialog-actions">
            <button class="ghost" type="button" @click="toggleSyncHistory">{{ t('common.close') }}</button>
            <button
              class="danger"
              :class="{ 'restore-ready': !!revisionPreview && !restoringRevision && !syncBusy }"
              type="button"
              :disabled="!revisionPreview || revisionPreviewLoading || restoringRevision || syncBusy"
              @click="handleRestoreClick"
            >{{ restoringRevision ? t('syncHistory.restoring') : restoreConfirmArmed ? t('syncHistory.confirmRestore', { version: remoteVersionBadge || 'v?' }) : t('syncHistory.restore') }}</button>
          </div>
        </section>
      </div>
    </Teleport>

    <SyncPasswordDialog :open="passwordModalOpen" />
    <CloudPasswordDialog />
    <GithubConnectDialog />
  </div>
</section>
</template>
