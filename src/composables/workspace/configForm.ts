import { computed, reactive, ref, type Ref } from 'vue';
import { defaultConfigValues, fallbackTypes, zhFieldLabels, zhTypeLabels } from './constants';
import type { ConfigField, PicbedConfig, PicbedTypeDef, WorkspaceTab } from './types';

type ConfigFormDeps = {
  activeTab: Ref<WorkspaceTab>;
  configs: Ref<PicbedConfig[]>;
  showError: (text: string) => void;
  clearNotice: () => void;
};

export function useWorkspaceConfigForm({ activeTab, configs, showError, clearNotice }: ConfigFormDeps) {
  const typeDefs = ref<PicbedTypeDef[]>(fallbackTypes);
  const configForm = reactive({
    id: 0,
    picbed_type: 'github',
    config_name: '',
    values: defaultConfigValues('github'),
    is_default: false,
  });
  const configErrors = reactive({ config_name: '', fields: {} as Record<string, string> });
  const secretVisibility = reactive({} as Record<string, boolean>);

  const supportedTypes = computed(() => typeDefs.value.filter(item => item.value !== 'unknown'));
  const selectedType = computed(
    () => typeDefs.value.find(item => item.value === configForm.picbed_type) || supportedTypes.value[0]
  );
  const selectedFields = computed(() =>
    selectedType.value?.fields?.length
      ? selectedType.value.fields
      : fallbackTypes.find(item => item.value === configForm.picbed_type)?.fields || []
  );

  function typeLabel(value: string) {
    return zhTypeLabels[value] || typeDefs.value.find(item => item.value === value)?.label || value;
  }
  function fieldLabel(field: ConfigField) {
    if (configForm.picbed_type === 'qiniu' && field.key === 'custom_domain') {
      return '自定义域名 / CDN测试域名';
    }
    if (configForm.picbed_type === 'upyun' && field.key === 'bucket') {
      return '服务名';
    }
    if (configForm.picbed_type === 'upyun' && field.key === 'custom_domain') {
      return '加速域名 / 测试域名';
    }
    if (configForm.picbed_type === 'minio' && field.key === 'endpoint') {
      return 'Endpoint';
    }
    return zhFieldLabels[field.key] || field.label;
  }
  function fieldPlaceholder(field: ConfigField) {
    return field.placeholder || fieldLabel(field);
  }
  function secretFieldVisible(fieldKey: string) {
    return Boolean(secretVisibility[fieldKey]);
  }
  function toggleSecretField(fieldKey: string) {
    secretVisibility[fieldKey] = !secretVisibility[fieldKey];
  }
  function clearSecretVisibility() {
    Object.keys(secretVisibility).forEach(key => delete secretVisibility[key]);
  }

  function mergeTypeDefs(types: PicbedTypeDef[]) {
    return types.map(item => ({
      ...item,
      label: typeLabel(item.value),
      fields: item.fields?.length ? item.fields : fallbackTypes.find(fallback => fallback.value === item.value)?.fields || [],
    }));
  }

  function clearConfigErrors() {
    configErrors.config_name = '';
    configErrors.fields = {};
  }
  function handleConfigTypeChange() {
    configForm.values = defaultConfigValues(configForm.picbed_type);
    clearSecretVisibility();
    clearConfigErrors();
  }

  function resetConfigForm() {
    clearNotice();
    Object.assign(configForm, {
      id: 0,
      picbed_type: 'github',
      config_name: '',
      values: defaultConfigValues('github'),
      is_default: false,
    });
    clearSecretVisibility();
    clearConfigErrors();
  }
  function editableConfigValues(config: PicbedConfig) {
    const values = defaultConfigValues(config.picbed_type);
    Object.entries(config.config || {}).forEach(([key, value]) => {
      if (key !== 'masked' && typeof value === 'string') values[key] = value;
    });
    return values;
  }
  function fillConfigForm(config: PicbedConfig) {
    Object.assign(configForm, {
      id: config.id,
      picbed_type: config.picbed_type,
      config_name: config.config_name,
      values: editableConfigValues(config),
      is_default: config.is_default,
    });
    clearSecretVisibility();
    clearConfigErrors();
    activeTab.value = 'configs';
  }
  function editConfig(config: PicbedConfig) {
    fillConfigForm(config);
  }
  function validateConfigForm() {
    clearConfigErrors();
    const missing: string[] = [];
    const invalid: string[] = [];
    if (!configForm.config_name.trim()) {
      configErrors.config_name = 'invalid';
      missing.push('配置名称');
    } else if (configs.value.some(item => item.id !== configForm.id && item.config_name === configForm.config_name.trim())) {
      configErrors.config_name = 'invalid';
      invalid.push('配置名称不能重复');
    }
    selectedFields.value.forEach(field => {
      if (field.required && !String(configForm.values[field.key] || '').trim()) {
        configErrors.fields[field.key] = 'invalid';
        missing.push(fieldLabel(field));
      }
    });
    if (missing.length || invalid.length) {
      const parts = [];
      if (missing.length) parts.push(`${missing.join('、')}不能为空`);
      if (invalid.length) parts.push(invalid.join('、'));
      showError(parts.join('；'));
      return false;
    }
    return true;
  }

  return {
    typeDefs,
    configForm,
    configErrors,
    secretVisibility,
    supportedTypes,
    selectedFields,
    typeLabel,
    fieldLabel,
    fieldPlaceholder,
    secretFieldVisible,
    toggleSecretField,
    clearSecretVisibility,
    handleConfigTypeChange,
    resetConfigForm,
    editConfig,
    validateConfigForm,
    mergeTypeDefs,
  };
}
