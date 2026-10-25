/// 界面语言模块：模块级单例状态，t() 按当前语言取词并支持 {name} 插值
import { ref } from 'vue';
import { zh } from './zh';
import { en } from './en';

export type Locale = 'zh' | 'en';

const LOCALE_STORAGE_KEY = 'picbed_locale';
const dictionaries: Record<Locale, Record<string, string>> = { zh, en };

function readStoredLocale(): Locale {
  return localStorage.getItem(LOCALE_STORAGE_KEY) === 'en' ? 'en' : 'zh';
}

const locale = ref<Locale>(readStoredLocale());

/// 把当前语言同步到 <html lang>，供无障碍朗读与字体回退使用
function applyDocumentLang(value: Locale) {
  document.documentElement.lang = value === 'zh' ? 'zh-CN' : 'en';
}

/// 缺词时回退键名，便于发现遗漏；未注册键（如后端新增图床类型）由调用方自行兜底
export function t(key: string, params?: Record<string, string | number>): string {
  const dict = dictionaries[locale.value] as Record<string, string>;
  let text = dict[key] ?? key;
  if (params) {
    for (const [name, value] of Object.entries(params)) {
      text = text.replaceAll(`{${name}}`, String(value));
    }
  }
  return text;
}

/// 判断当前语言是否存在该词条（用于动态键的回退判断）
export function hasMessage(key: string): boolean {
  return key in (dictionaries[locale.value] as Record<string, string>);
}

/// 切换界面语言并持久化偏好
export function setLocale(value: Locale) {
  locale.value = value;
  localStorage.setItem(LOCALE_STORAGE_KEY, value);
  applyDocumentLang(value);
}

export function useI18n() {
  applyDocumentLang(locale.value);
  function toggleLocale() {
    setLocale(locale.value === 'zh' ? 'en' : 'zh');
  }
  return { locale, setLocale, toggleLocale, t };
}
