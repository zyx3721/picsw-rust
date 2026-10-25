import { ref } from 'vue';

export type ThemeMode = 'light' | 'dark';

const THEME_STORAGE_KEY = 'picbed_theme';

function readStoredTheme(): ThemeMode {
  return localStorage.getItem(THEME_STORAGE_KEY) === 'dark' ? 'dark' : 'light';
}

const theme = ref<ThemeMode>(readStoredTheme());

/// 把当前主题写入 <html> 根元素 class，驱动 style.css 中的深色变量生效
function applyTheme(mode: ThemeMode) {
  document.documentElement.classList.toggle('dark', mode === 'dark');
}

/// 深浅色主题：模块级单例状态，切换即更新根元素并持久化偏好
export function useTheme() {
  applyTheme(theme.value);
  function setTheme(mode: ThemeMode) {
    theme.value = mode;
    localStorage.setItem(THEME_STORAGE_KEY, mode);
    applyTheme(mode);
  }
  function toggleTheme() {
    setTheme(theme.value === 'dark' ? 'light' : 'dark');
  }
  return { theme, setTheme, toggleTheme };
}
