<script setup lang="ts">
import { FileSearch, Wand2 } from 'lucide-vue-next';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';

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
        <h2 id="github-proxy-title">检测到 GitHub 图床地址</h2>
        <p>当前识别结果包含 GitHub 图片文件地址，可在转换前为这些地址添加代理前缀，便于第三方服务下载。</p>
      </div>
      <label class="checkbox proxy-toggle"><input v-model="githubProxyEnabled" type="checkbox" />启用图片代理</label>
      <label v-if="githubProxyEnabled" class="proxy-url-field">
        <span class="field-label-text">代理地址</span>
        <input v-model.trim="githubProxyURL" placeholder="https://gh-proxy.com/" />
      </label>
      <div class="dialog-actions">
        <button class="ghost" type="button" @click="closeGithubProxyDialog">取消</button>
        <button class="primary" type="button" :disabled="loading" @click="confirmGithubProxyConvert">
          <Wand2 :size="17" />{{ loading ? '转换中...' : '继续转换' }}
        </button>
      </div>
    </section>
  </div>
</template>
