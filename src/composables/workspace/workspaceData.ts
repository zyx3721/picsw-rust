import type { Ref } from 'vue';
import type { ConversionRecord, PicbedConfig, PicbedTypeDef } from './types';

type WorkspaceRequest = <T>(path: string, options?: RequestInit) => Promise<T>;

type WorkspaceDataDeps = {
  request: WorkspaceRequest;
  typeDefs: Ref<PicbedTypeDef[]>;
  configs: Ref<PicbedConfig[]>;
  records: Ref<ConversionRecord[]>;
  getTargetConfigId: () => number;
  setTargetConfigId: (id: number) => void;
  getDefaultTarget: () => PicbedConfig | undefined;
  mergeTypeDefs: (types: PicbedTypeDef[]) => PicbedTypeDef[];
};

export function useWorkspaceData({
  request,
  typeDefs,
  configs,
  records,
  getTargetConfigId,
  setTargetConfigId,
  getDefaultTarget,
  mergeTypeDefs,
}: WorkspaceDataDeps) {
  async function loadWorkspaceData() {
    await Promise.all([loadTypes(), loadConfigs(), loadRecords()]);
  }

  async function loadTypes() {
    const data = await request<{ types: PicbedTypeDef[] }>('/api/picbed/types');
    typeDefs.value = mergeTypeDefs(data.types);
  }

  async function loadConfigs() {
    const data = await request<{ configs: PicbedConfig[] }>('/api/picbed/configs');
    configs.value = data.configs;
    const defaultConfig = getDefaultTarget();
    const currentTargetId = getTargetConfigId();
    if (defaultConfig) setTargetConfigId(defaultConfig.id);
    else if (!configs.value.some(item => item.id === currentTargetId)) setTargetConfigId(0);
  }

  async function loadRecords() {
    const data = await request<{ records: ConversionRecord[] }>('/api/convert/records');
    records.value = data.records;
  }

  return { loadWorkspaceData, loadTypes, loadConfigs, loadRecords };
}
