import type { Ref } from 'vue';
import type { PicbedConfig } from './types';

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
      showMessage(configForm.id ? '配置已更新' : '配置已保存');
      resetConfigForm();
      await loadConfigs();
    } catch (err) {
      showError(err instanceof Error ? err.message : '保存配置失败');
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
      showMessage('配置已删除');
      cancelDeleteConfig();
      await loadConfigs();
    } catch (err) {
      showError(err instanceof Error ? err.message : '删除配置失败');
    }
  }
  async function setDefault(id: number) {
    try {
      await request(`/api/picbed/configs/${id}/default`, { method: 'PUT' });
      showMessage('默认配置已更新');
      await loadConfigs();
    } catch (err) {
      showError(err instanceof Error ? err.message : '设置默认失败');
    }
  }

  async function testConfig(config?: PicbedConfig) {
    loading.value = true;
    try {
      if (config) {
        const data = await request<{ message: string }>(`/api/picbed/configs/${config.id}/test`, { method: 'POST' });
        showMessage(data.message || '配置测试通过');
        return;
      }
      if (!validateConfigForm()) return;
      const payload = {
        picbed_type: configForm.picbed_type,
        config_name: configForm.config_name || '临时测试配置',
        is_default: configForm.is_default,
        config: selectedFields.value.reduce((values, field) => {
          const value = configForm.values[field.key];
          if (typeof value === 'string') values[field.key] = value;
          return values;
        }, {} as Record<string, string>),
      };
      const data = await request<{ message: string }>('/api/picbed/configs/test', { method: 'POST', body: JSON.stringify(payload) });
      showMessage(data.message || '配置测试通过');
    } catch (err) {
      showError(err instanceof Error ? err.message : '配置测试失败');
    } finally {
      loading.value = false;
    }
  }

  return { saveConfig, requestDeleteConfig, cancelDeleteConfig, confirmDeleteConfig, setDefault, testConfig };
}
