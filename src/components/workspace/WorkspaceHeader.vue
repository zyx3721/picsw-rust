<script setup lang="ts">
import { ImageUp, Moon, Sun } from 'lucide-vue-next';
import { useTheme } from '../../composables/useTheme';

const { theme, toggleTheme } = useTheme();
</script>

<template>
  <header class="topbar">
    <div class="brand-line">
      <div class="brand-mark"><ImageUp :size="24" /></div>
      <div>
        <p class="eyebrow">PicBed Switcher</p>
        <h1>图床转站助手</h1>
      </div>
    </div>
    <div class="topbar-actions">
      <button
        class="ghost icon-only topbar-action"
        type="button"
        :title="theme === 'dark' ? '切换到浅色' : '切换到深色'"
        :aria-label="theme === 'dark' ? '切换到浅色' : '切换到深色'"
        @click="toggleTheme"
      >
        <Sun v-if="theme === 'dark'" :size="18" />
        <Moon v-else :size="18" />
      </button>
    </div>
  </header>
</template>
