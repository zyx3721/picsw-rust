<script setup lang="ts">
import { Database, FileSearch, History, Wand2 } from 'lucide-vue-next';
import { useWorkspaceContext } from '../../composables/useWorkspaceContext';

const { configs, totalImages, convertedCount, successRecords } = useWorkspaceContext();
</script>

<template>
  <section class="overview">
    <div class="metric-tile">
      <span><Database :size="18" /> 图床配置</span><strong>{{ configs.length }}</strong>
    </div>
    <div class="metric-tile accent">
      <span><FileSearch :size="18" /> 队列图片</span><strong>{{ totalImages }}</strong>
    </div>
    <div class="metric-tile">
      <span><Wand2 :size="18" /> 队列成功</span><strong>{{ convertedCount }}</strong>
    </div>
    <div class="metric-tile muted">
      <span><History :size="18" /> 历史成功</span><strong>{{ successRecords }}</strong>
    </div>
  </section>
</template>
