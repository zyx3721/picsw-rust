<script setup lang="ts">
import { AlertTriangle } from 'lucide-vue-next';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';
import { t } from '../../i18n';

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
  <span>{{ t('sync.tokenExpiredHint') }}</span>
  <em>{{ t('sync.handleIt') }}</em>
</button>
</template>
