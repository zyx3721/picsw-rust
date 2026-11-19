<script setup lang="ts">
import { Sparkles } from 'lucide-vue-next';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';
import { t } from '../../i18n';

const { activeTab, updateStatus, updateVersion, setActiveTab } = useWorkspaceContext();
</script>

<template>
<button
  v-if="updateStatus === 'available' && updateVersion && activeTab !== 'about'"
  class="update-banner"
  type="button"
  @click="setActiveTab('about')"
>
  <Sparkles :size="16" />
  <span>{{ t('update.availableHint', { version: updateVersion }) }}</span>
  <em>{{ t('update.viewUpdate') }}</em>
</button>
</template>
