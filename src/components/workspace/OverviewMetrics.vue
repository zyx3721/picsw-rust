<script setup lang="ts">
import { Database, FileSearch, History, Wand2 } from 'lucide-vue-next';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';
import { t } from '../../i18n';

const { configs, totalImages, convertedCount, successRecords } = useWorkspaceContext();
</script>

<template>
  <section class="overview">
    <div class="metric-tile">
      <span><Database :size="18" /> {{ t('metric.configs') }}</span><strong>{{ configs.length }}</strong>
    </div>
    <div class="metric-tile accent">
      <span><FileSearch :size="18" /> {{ t('metric.queueImages') }}</span><strong>{{ totalImages }}</strong>
    </div>
    <div class="metric-tile">
      <span><Wand2 :size="18" /> {{ t('metric.queueSuccess') }}</span><strong>{{ convertedCount }}</strong>
    </div>
    <div class="metric-tile muted">
      <span><History :size="18" /> {{ t('metric.historySuccess') }}</span><strong>{{ successRecords }}</strong>
    </div>
  </section>
</template>
