import type { Ref } from 'vue';
import type { PicbedConfig } from './types';
import { t } from '../../i18n';

type WorkspaceRequest = <T>(path: string, options?: RequestInit) => Promise<T>;

type ConfigActionsDeps = {
  request: WorkspaceRequest;
  configForm: {
    id: number;
    picbed_type: string;
    config_name: string;
    values: Record<string, string>;
    is_default: boolean;
  };
  selectedFields: Ref<Array<{ key: string }>>;
  deleteTarget: Ref<PicbedConfig | null>;
  loading: Ref<boolean>;
  validateConfigForm: () => boolean;
  resetConfigForm: () => void;
  loadConfigs: () => Promise<void>;
  showMessage: (text: string) => void;
  showError: (text: string) => void;
};

export function useWorkspaceConfigActions({
  request,
  configForm,
  selectedFields,
  deleteTarget,
  loading,
  validateConfigForm,
  resetConfigForm,
  loadConfigs,
  showMessage,
  showError,
}: ConfigActionsDeps) {
  async function saveConfig() {
    if (!validateConfigForm()) return;
    loading.value = true;
    const payload = {
      picbed_type: configForm.picbed_type,
      config_name: configForm.config_name,
      is_default: configForm.is_default,
      config: selectedFields.value.reduce((values, field) => {
        const value = configForm.values[field.key];
        if (typeof value === 'string') values[field.key] = value;
        return values;
      }, {} as Record<string, string>),
    };
    try {
      if (configForm.id) {
        await request(`/api/picbed/configs/${configForm.id}`, { method: 'PUT', body: JSON.stringify(payload) });
      } else {
        await request('/api/picbed/configs', { method: 'POST', body: JSON.stringify(payload) });
      }
      showMessage(configForm.id ? t('msg.config.updated') : t('msg.config.saved'));
      resetConfigForm();
      await loadConfigs();
    } catch (err) {
      showError(err instanceof Error ? err.message : t('msg.config.saveFailed'));
    } finally {
      loading.value = false;
    }
  }
  function requestDeleteConfig(config: PicbedConfig) {
    deleteTarget.value = config;
  }
  function cancelDeleteConfig() {
    deleteTarget.value = null;
  }
  async function confirmDeleteConfig() {
    if (!deleteTarget.value) return;
    const id = deleteTarget.value.id;
    try {
      await request(`/api/picbed/configs/${id}`, { method: 'DELETE' });
      showMessage(t('msg.config.deleted'));
      cancelDeleteConfig();
      await loadConfigs();
    } catch (err) {
      showError(err instanceof Error ? err.message : t('msg.config.deleteFailed'));
    }
  }
  async function setDefault(id: number) {
    try {
      await request(`/api/picbed/configs/${id}/default`, { method: 'PUT' });
      showMessage(t('msg.config.defaultUpdated'));
      await loadConfigs();
    } catch (err) {
      showError(err instanceof Error ? err.message : t('msg.config.defaultFailed'));
    }
  }

  async function testConfig(config?: PicbedConfig) {
    loading.value = true;
    try {
      if (config) {
        const data = await request<{ message: string }>(`/api/picbed/configs/${config.id}/test`, { method: 'POST' });
        showMessage(data.message || t('msg.config.testPassed'));
        return;
      }
      if (!validateConfigForm()) return;
      const payload = {
        picbed_type: configForm.picbed_type,
        config_name: configForm.config_name || t('msg.config.tempName'),
        is_default: configForm.is_default,
        config: selectedFields.value.reduce((values, field) => {
          const value = configForm.values[field.key];
          if (typeof value === 'string') values[field.key] = value;
          return values;
        }, {} as Record<string, string>),
      };
      const data = await request<{ message: string }>('/api/picbed/configs/test', { method: 'POST', body: JSON.stringify(payload) });
      showMessage(data.message || t('msg.config.testPassed'));
    } catch (err) {
      showError(err instanceof Error ? err.message : t('msg.config.testFailed'));
    } finally {
      loading.value = false;
    }
  }

  return { saveConfig, requestDeleteConfig, cancelDeleteConfig, confirmDeleteConfig, setDefault, testConfig };
}
