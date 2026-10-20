<script setup lang="ts">
import { AlertTriangle } from 'lucide-vue-next';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';

const { activeTab, syncConnected, syncTokenInvalid, setActiveTab } = useWorkspaceContext();
</script>

<template>
<button
  v-if="syncConnected && syncTokenInvalid && activeTab !== 'cloudSync'"
  class="sync-token-banner"
  type="button"
  @click="setActiveTab('cloudSync')"
>
  <AlertTriangle :size="16" />
  <span>GitHub 授权已失效，云同步已暂停 —— 点击处理（可先重新验证，数据不会丢失）</span>
  <em>去处理</em>
</button>
</template>
