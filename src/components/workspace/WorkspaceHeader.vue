<script setup lang="ts">
import { ImageUp, Languages, Moon, Sun } from 'lucide-vue-next';
import { useTheme } from '../../composables/useTheme';
import { t, useI18n } from '../../i18n';

const { theme, toggleTheme } = useTheme();
const { locale, toggleLocale } = useI18n();
</script>

<template>
  <header class="topbar">
    <div class="brand-line">
      <div class="brand-mark"><ImageUp :size="24" /></div>
      <div>
        <p class="eyebrow">PicBed Switcher</p>
        <h1>{{ t('app.title') }}</h1>
      </div>
    </div>
    <div class="topbar-actions">
      <button
        class="ghost icon-only topbar-action"
        type="button"
        :title="theme === 'dark' ? t('header.themeToLight') : t('header.themeToDark')"
        :aria-label="theme === 'dark' ? t('header.themeToLight') : t('header.themeToDark')"
        @click="toggleTheme"
      >
        <Sun v-if="theme === 'dark'" :size="18" />
        <Moon v-else :size="18" />
      </button>
      <button
        class="ghost topbar-action lang"
        type="button"
        :title="locale === 'zh' ? t('header.langToEn') : t('header.langToZh')"
        :aria-label="locale === 'zh' ? t('header.langToEn') : t('header.langToZh')"
        @click="toggleLocale"
      >
        <Languages :size="17" />
        <span>{{ locale === 'zh' ? 'EN' : '中' }}</span>
      </button>
    </div>
  </header>
</template>
