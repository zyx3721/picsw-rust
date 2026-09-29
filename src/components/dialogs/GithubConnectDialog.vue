<script setup lang="ts">
import { Cloud, ExternalLink, X } from 'lucide-vue-next';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';

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
        <h2 id="github-connect-title"><Cloud :size="20" />连接 GitHub</h2>
        <button class="ghost icon-only" type="button" aria-label="关闭" @click="cancelGithubConnect"><X :size="18" /></button>
      </div>

      <template v-if="!patMode">
        <template v-if="deviceFlowAvailable">
          <p class="connect-step">1. 点击下方按钮打开 GitHub 授权页，输入设备码：</p>
          <div class="connect-code-row">
            <span class="connect-user-code" data-status="pending">{{ deviceFlowUserCode || '····-····' }}</span>
            <button class="secondary" type="button" :disabled="!deviceFlowUserCode" @click="openVerificationPage">
              <ExternalLink :size="16" />打开 github.com/login/device
            </button>
          </div>
          <p class="connect-step">2. 授权后本页会自动完成连接，请稍候...</p>
          <p v-if="deviceFlowStatus === 'pending'" class="connect-waiting"><span class="connect-spinner" />等待授权完成...</p>
          <p v-if="deviceFlowStatus === 'connected'" class="connect-done">已连接，正在拉取云端状态...</p>
          <p v-if="deviceFlowStatus === 'error'" class="sync-dialog-error">{{ deviceFlowError }}</p>
        </template>
        <p v-else class="sync-dialog-note">
          未检测到 GitHub OAuth App 的 client_id：可在构建期以 VITE_SYNC_GITHUB_CLIENT_ID 注入，或在程序目录、数据目录的 .env 文件及系统环境变量中配置同名变量，配置后即可使用设备码授权。
        </p>
      </template>

      <template v-else>
        <label class="sync-dialog-field">
          <span>个人访问令牌（需 gist 权限）</span>
          <input
            v-model="syncPatInput"
            type="password"
            placeholder="粘贴 GitHub 个人访问令牌（PAT）"
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
        >使用个人访问令牌（PAT）连接</button>
        <button class="ghost" type="button" @click="cancelGithubConnect">取消</button>
        <button
          v-if="patMode"
          class="primary"
          type="button"
          :disabled="!syncPatInput.trim()"
          @click="connectGithub"
        >连接</button>
        <button
          v-else-if="deviceFlowUserCode"
          class="primary"
          type="button"
          @click="copyUserCode"
        >复制设备码</button>
      </div>
      <p v-if="syncRemoteState" class="sync-dialog-note">云端已有同步数据（版本 {{ syncRemoteState.version }}），连接后可拉取导入</p>
    </section>
  </div>
  </Teleport>
</template>
