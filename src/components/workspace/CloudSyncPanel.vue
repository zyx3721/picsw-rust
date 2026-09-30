<script setup lang="ts">
import { AlertTriangle, Cloud, Database, Download, History, KeyRound, Lock, RefreshCw, Trash2, Upload, X, Zap } from 'lucide-vue-next';
import AppTooltip from '../AppTooltip.vue';
import SyncPasswordDialog from '../dialogs/SyncPasswordDialog.vue';
import GithubConnectDialog from '../dialogs/GithubConnectDialog.vue';
import CloudPasswordDialog from '../dialogs/CloudPasswordDialog.vue';
import { computed, ref, watch } from 'vue';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';

const {
  syncAccount,
  secretBackend,
  syncBusy,
  syncConnected,
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

/// 恢复按钮两步确认（对照 dockpilot）：首次点击进入确认态，再次点击才执行恢复
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
        <h2>云同步配置</h2>
      </div>
      <span class="sync-badge">多台设备间加密同步配置（GitHub 私有 Gist）</span>
    </div>

    <div v-if="syncBanner" class="sync-banner" :class="`banner-${syncBanner.kind}`">
      <div class="sync-banner-body">
        <span class="sync-banner-text">
          <AlertTriangle v-if="syncBanner.kind !== 'conflict'" :size="15" class="sync-banner-alert-icon" />{{ syncBanner.message }}
        </span>
        <span v-if="syncBanner.kind === 'conflict'" class="sync-banner-detail">
          云端版本 v{{ syncBanner.remoteVersion }}<template v-if="syncBanner.remoteDeviceName">（{{ syncBanner.remoteDeviceName }}）</template>，本机版本 v{{ syncBanner.localVersion }}。两台设备的同步密码不同。
        </span>
        <span v-if="syncBanner.kind === 'empty'" class="sync-banner-detail">
          本机没有图床配置，云端存在 {{ syncBanner.remoteItems.length }} 条图床配置（配置内容已端到端加密，云端只存密文）。为防止误覆盖云端，请选择如何处理：
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
          <Download :size="16" />使用云端（输入云端密码）
        </button>
        <button
          v-if="syncBanner.kind === 'conflict'"
          class="warn"
          type="button"
          :disabled="syncBusy"
          @click="overrideCloudWithLocal"
        >使用本地覆盖云端</button>
        <button
          v-if="syncBanner.kind === 'blocked'"
          class="secondary"
          type="button"
          :disabled="syncBusy"
          @click="restoreRemoteVersion"
        >
          <Download :size="16" />恢复云端数据
        </button>
        <button
          v-if="syncBanner.kind === 'blocked'"
          class="danger"
          type="button"
          :disabled="syncBusy"
          @click="forcePushLocal"
        >
          <Upload :size="16" />强制推送
        </button>
        <button
          v-if="syncBanner.kind === 'empty'"
          class="secondary"
          type="button"
          :disabled="syncBusy"
          @click="restoreRemoteVersion"
        >
          <Download :size="16" />恢复云端数据
        </button>
        <button
          v-if="syncBanner.kind === 'empty'"
          class="info"
          type="button"
          :disabled="syncBusy"
          @click="pushEmptyVaultLocal"
        >
          <Upload :size="16" />推送本机数据
        </button>
      </div>
    </div>

    <div class="sync-row">
      <div class="sync-row-info">
        <span class="sync-row-icon"><Cloud :size="20" /></span>
        <div>
          <strong>GitHub Gist</strong>
          <span v-if="syncConnected">{{ syncAccount }}（令牌存{{ secretBackend === 'file' ? '加密本机文件' : '系统钥匙串' }}）</span>
          <span v-else>未连接 —— 授权后配置将加密存入你的私有 Gist</span>
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
            <Cloud v-else :size="17" />连接 GitHub
          </button>
        </template>
        <template v-else>
          <span class="sync-connected-tag">已连接</span>
          <AppTooltip label="断开连接（清除本地令牌与同步快照）" center>
            <button class="ghost sync-disconnect" type="button" :disabled="syncBusy" @click="disconnectGithub">
              <Trash2 :size="17" />断开
            </button>
          </AppTooltip>
        </template>
      </div>
    </div>

    <div class="sync-row">
      <div class="sync-row-info">
        <span class="sync-row-icon"><KeyRound :size="20" /></span>
        <div>
          <strong>同步密码</strong>
          <span v-if="passwordStatus === 'none'">用于端到端加密云端数据（遗忘后云端数据无法恢复）</span>
          <span v-else-if="passwordStatus === 'locked'">解锁后本机会记住密码，下次启动自动解锁</span>
          <span v-else-if="passwordBackend">密码已记住到本机密钥库，启动时自动解锁</span>
          <span v-else>密码仅在本次运行内存中持有（本机密钥库不可用）</span>
        </div>
      </div>
      <div class="sync-row-actions">
        <template v-if="passwordStatus === 'none'">
          <span class="sync-state-tag warn">未设密码</span>
          <button class="secondary" type="button" :disabled="syncBusy" @click="openPasswordModal('set')">
            <KeyRound :size="17" />设置密码
          </button>
        </template>
        <template v-else-if="passwordStatus === 'locked'">
          <span class="sync-state-tag muted">已锁定</span>
          <div class="sync-unlock-row">
            <input
              v-model="unlockInput"
              type="password"
              placeholder="同步密码"
              spellcheck="false"
              autocomplete="off"
              @keyup.enter="submitUnlock"
            />
            <button class="primary" type="button" :disabled="!unlockInput" @click="submitUnlock">
              <Lock :size="17" />解锁
            </button>
          </div>
        </template>
        <template v-else>
          <span class="sync-state-tag ok">已解锁</span>
          <AppTooltip label="修改同步密码" center>
            <button class="secondary" type="button" :disabled="syncBusy" @click="openPasswordModal('change')">
              <KeyRound :size="17" />修改密码
            </button>
          </AppTooltip>
          <AppTooltip label="锁定并清除本机记住的密码（下次启动需手动解锁）" center>
            <button class="secondary" type="button" :disabled="syncBusy" @click="lockSyncPassword">
              <Lock :size="17" />锁定
            </button>
          </AppTooltip>
        </template>
      </div>
    </div>

    <div class="sync-row">
      <div class="sync-row-info">
        <span class="sync-row-icon"><Database :size="20" /></span>
        <div>
          <strong>同步内容</strong>
          <span>仅同步全部图床配置（加密后存储）；转换记录、任务等本地数据不上云</span>
        </div>
      </div>
      <span class="sync-count-tag">本地 {{ localConfigCount }} 个配置</span>
    </div>

    <div class="sync-row">
      <div class="sync-row-info">
        <span class="sync-row-icon"><Zap :size="20" /></span>
        <div>
          <strong>自动同步</strong>
          <span>配置变更 3 秒后自动上传；启动与窗口切回时自动检查云端更新</span>
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
            <strong>同步状态</strong>
            <span class="sync-status-badge" :data-kind="syncStatusBadge.kind">{{ syncStatusBadge.text }}</span>
            <span v-if="remoteVersionBadge" class="sync-status-badge version">{{ remoteVersionBadge }}</span>
          </div>
          <span class="sync-status-sub">{{ lastSyncText }}</span>
        </div>
      </div>
      <div class="sync-row-actions">
        <button class="secondary" type="button" :disabled="!canSyncNow" @click="toggleSyncHistory">
          <History :size="17" />历史版本
        </button>
        <button class="primary" type="button" :disabled="!canSyncNow" @click="syncNow()">
          <RefreshCw :size="17" :class="{ 'spin-icon': syncBusy }" />{{ syncBusy ? '同步中…' : '立即同步' }}
        </button>
      </div>
    </div>

    <Teleport to="body">
      <div v-if="syncHistoryOpen" class="modal-backdrop" role="presentation" @click.self="toggleSyncHistory">
        <section class="confirm-dialog sync-history-dialog" role="dialog" aria-modal="true" aria-labelledby="sync-history-title">
          <div class="sync-dialog-head">
            <h2 id="sync-history-title"><History :size="20" />历史版本</h2>
            <button class="ghost icon-only" type="button" aria-label="关闭" @click="toggleSyncHistory"><X :size="18" /></button>
          </div>
          <p class="sync-history-desc">
            浏览并恢复 Gist 修订历史中的旧版配置数据。恢复是<strong>整体覆盖（不是合并）</strong>：本机与云端都会变成所选修订的内容，并作为新版本推送，不会改写修订历史。
          </p>
          <div class="sync-history-list">
            <p v-if="syncHistoryLoading" class="sync-history-hint">
              <RefreshCw :size="14" class="spin-icon" />正在获取修订历史…
            </p>
            <p v-else-if="syncHistory.length === 0" class="sync-history-hint">暂无修订历史，完成一次同步后这里会显示记录</p>
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
                <RefreshCw :size="14" class="spin-icon" />正在解密该修订…
              </p>
              <div v-else-if="revisionPreview" class="sync-history-preview-card">
                <p class="sync-history-preview-title">
                  <AlertTriangle :size="14" />恢复预览 —— 整体覆盖，不是合并
                </p>
                <p>
                  图床配置：<span :class="{ 'sync-preview-shrink': revisionShrink > 0 }">{{ revisionPreview.currentCount }}</span> → {{ revisionPreview.configCount }} 条<template v-if="revisionShrink > 0"><span class="sync-preview-shrink">（将减少 {{ revisionShrink }} 条）</span></template>
                </p>
                <p>
                  来源：版本 v{{ revisionPreview.version }}<template v-if="revisionPreview.deviceName"> · {{ revisionPreview.deviceName }}</template><template v-if="revisionPreview.appVersion">（{{ revisionPreview.appVersion }}）</template> · 数据时间 {{ revisionPreview.updatedAtText }}
                </p>
                <p class="sync-history-preview-note">恢复后本机与云端都变成该内容并作为新版本推送；被覆盖的内容可从修订历史再次恢复回来</p>
              </div>
              <p v-else-if="revisionError" class="sync-history-error-card">{{ revisionError }}</p>
            </template>
          </div>
          <div class="dialog-actions">
            <button class="ghost" type="button" @click="toggleSyncHistory">关闭</button>
            <button
              class="danger"
              :class="{ 'restore-ready': !!revisionPreview && !restoringRevision && !syncBusy }"
              type="button"
              :disabled="!revisionPreview || revisionPreviewLoading || restoringRevision || syncBusy"
              @click="handleRestoreClick"
            >{{ restoringRevision ? '恢复中…' : restoreConfirmArmed ? `确认恢复（覆盖当前 ${remoteVersionBadge || 'v?'}）` : '恢复此版本（覆盖当前配置）' }}</button>
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
