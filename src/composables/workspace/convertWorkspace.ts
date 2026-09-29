import { computed, reactive, ref, type Ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';
import { createClientId } from './api';
import { isGithubImageURL, normalizeProxyURL, withGithubProxy } from './imageProxy';
import type { BatchFile, ConversionRecord, ConversionTask, MarkdownImage, PicbedConfig } from './types';

type WorkspaceRequest = <T>(path: string, options?: RequestInit) => Promise<T>;
const CONVERT_WORKSPACE_STORAGE_KEY = 'picbed_convert_workspace';

type ConvertTaskData = { task: ConversionTask; records: ConversionRecord[] };

type PersistedConvertWorkspace = {
  targetConfigId: number;
  batchFiles: BatchFile[];
  activeTaskId?: number;
  savedAt: number;
};

function taskFinished(task: ConversionTask) {
  return task.status === 'success' || task.status === 'failed';
}

function taskProgressValue(task: ConversionTask) {
  return Math.min(task.success + task.failed, task.total);
}

function waitForNextPoll(ms: number) {
  return new Promise(resolve => window.setTimeout(resolve, ms));
}

type ConvertWorkspaceDeps = {
  configs: Ref<PicbedConfig[]>;
  request: WorkspaceRequest;
  showMessage: (text: string) => void;
  showError: (text: string) => void;
  clearNotice: () => void;
  loadRecords: () => Promise<void>;
  typeLabel: (value: string) => string;
  loading: Ref<boolean>;
  startTaskProgress: (input: { title: string; message: string; total: number; detail?: string }) => void;
  updateTaskProgress: (input: { message?: string; detail?: string; current?: number; success?: number; failed?: number }) => void;
  finishTaskProgress: (input: { status: 'success' | 'failed'; message: string; detail?: string }) => void;
};

export function useWorkspaceConvert({
  configs,
  request,
  showMessage,
  showError,
  clearNotice,
  loadRecords,
  typeLabel,
  loading,
  startTaskProgress,
  updateTaskProgress,
  finishTaskProgress,
}: ConvertWorkspaceDeps) {
  const convertForm = reactive({ target_config_id: 0 });
  const pasteForm = reactive({ filename: 'pasted.md', content: '' });
  const batchFiles = ref<BatchFile[]>([]);
  const targetDropdownOpen = ref(false);
  const uploadDragActive = ref(false);
  const githubProxyDialogOpen = ref(false);
  const githubProxyEnabled = ref(true);
  const githubProxyURL = ref('https://gh-proxy.com/');
  let activeTaskId = 0;
  let pollVersion = 0;

  const targetConfigs = computed(() => configs.value);
  const defaultTarget = computed(() => targetConfigs.value.find(item => item.is_default) || targetConfigs.value[0]);
  const selectedTargetConfig = computed(() =>
    targetConfigs.value.find(item => item.id === convertForm.target_config_id)
  );
  const totalImages = computed(() => batchFiles.value.reduce((sum, item) => sum + item.images.length, 0));
  const convertedCount = computed(() => batchFiles.value.filter(item => item.status === 'success').length);
  const isBatchAnalyzed = computed(
    () => batchFiles.value.length > 0 && batchFiles.value.every(item => item.status === 'analyzed')
  );
  const canConvertBatch = computed(() => isBatchAnalyzed.value && totalImages.value > 0);
  const hasGithubImages = computed(() =>
    batchFiles.value.some(file =>
      file.status === 'analyzed' && file.images.some(image => image.picbed === 'github' || isGithubImageURL(image.url))
    )
  );

  function statusLabel(status: BatchFile['status']) {
    return { ready: '待识别', analyzed: '已识别', success: '已转换', failed: '失败' }[status];
  }
  function targetConfigLabel(config?: PicbedConfig) {
    return config ? `${config.config_name} · ${typeLabel(config.picbed_type)}` : '请选择';
  }
  function selectTargetConfig(id: number) {
    convertForm.target_config_id = id;
    targetDropdownOpen.value = false;
  }

  function readPersistedConvertWorkspace() {
    try {
      const raw = localStorage.getItem(CONVERT_WORKSPACE_STORAGE_KEY);
      if (!raw) return null;
      const state = JSON.parse(raw) as PersistedConvertWorkspace;
      if (!Array.isArray(state.batchFiles)) return null;
      return state;
    } catch {
      return null;
    }
  }
  function persistConvertWorkspace(taskId = activeTaskId) {
    try {
      const state: PersistedConvertWorkspace = {
        targetConfigId: convertForm.target_config_id,
        batchFiles: batchFiles.value,
        activeTaskId: taskId || undefined,
        savedAt: Date.now(),
      };
      localStorage.setItem(CONVERT_WORKSPACE_STORAGE_KEY, JSON.stringify(state));
    } catch {
      // Storage may be unavailable in private mode; conversion itself should continue.
    }
  }
  function clearPersistedConvertWorkspace() {
    localStorage.removeItem(CONVERT_WORKSPACE_STORAGE_KEY);
  }
  function stopConvertTaskPolling() {
    pollVersion += 1;
    activeTaskId = 0;
  }
  function resetConvertForm() {
    if (activeTaskId) return;
    clearNotice();
    stopConvertTaskPolling();
    convertForm.target_config_id = defaultTarget.value?.id || 0;
    targetDropdownOpen.value = false;
    pasteForm.filename = 'pasted.md';
    pasteForm.content = '';
    persistConvertWorkspace();
    batchFiles.value = [];
    closeGithubProxyDialog();
    clearPersistedConvertWorkspace();
  }
  function syncTaskProgress(task: ConversionTask) {
    updateTaskProgress({
      current: taskProgressValue(task),
      success: task.success,
      failed: task.failed,
      message: task.message || '转换任务执行中',
    });
  }
  function applyTaskRecords(data: ConvertTaskData) {
    const recordsByFilename = new Map<string, ConversionRecord[]>();
    for (const record of data.records) {
      const filename = record.original_filename || '';
      recordsByFilename.set(filename, [...(recordsByFilename.get(filename) || []), record]);
    }
    const canFallbackToPosition = data.records.length === batchFiles.value.length;

    for (const [index, file] of batchFiles.value.entries()) {
      const filenameRecords = recordsByFilename.get(file.filename) || [];
      const record = filenameRecords.shift() || (canFallbackToPosition ? data.records[index] : undefined);
      if (filenameRecords.length) {
        recordsByFilename.set(file.filename, filenameRecords);
      } else {
        recordsByFilename.delete(file.filename);
      }
      if (!record) {
        file.status = 'failed';
        file.convertedContent = '';
        file.changed = 0;
        file.error = '未找到转换结果';
        continue;
      }
      file.status = record.status === 'success' ? 'success' : 'failed';
      file.convertedContent = record.converted_content || '';
      file.changed = record.image_count || 0;
      file.error = record.error_message || '';
    }
  }
  async function addMarkdownFiles(fileList: FileList | File[]) {
    const files = Array.from(fileList).filter(file => file.name.toLowerCase().endsWith('.md'));
    if (!files.length) {
      showError('请上传或拖动 .md 文件');
      return;
    }
    const loaded = await Promise.all(
      files.map(async file => ({
        id: `${file.name}-${file.size}-${createClientId()}`,
        filename: file.name,
        content: await file.text(),
        images: [],
        convertedContent: '',
        changed: 0,
        status: 'ready' as const,
        error: '',
        previewOpen: false,
      }))
    );
    batchFiles.value = [...batchFiles.value, ...loaded];
    persistConvertWorkspace();
    showMessage(`已加入 ${loaded.length} 个 Markdown 文件`);
  }
  async function handleFiles(event: Event) {
    const input = event.target as HTMLInputElement;
    await addMarkdownFiles(input.files || []);
    input.value = '';
  }
  async function handleFileDrop(event: DragEvent) {
    uploadDragActive.value = false;
    await addMarkdownFiles(event.dataTransfer?.files || []);
  }
  function addPastedDocument() {
    if (!pasteForm.content.trim()) {
      showError('请先粘贴 Markdown 内容');
      return;
    }
    batchFiles.value.push({
      id: `paste-${createClientId()}`,
      filename: pasteForm.filename || 'pasted.md',
      content: pasteForm.content,
      images: [],
      convertedContent: '',
      changed: 0,
      status: 'ready',
      error: '',
      previewOpen: false,
    });
    pasteForm.content = '';
    persistConvertWorkspace();
    showMessage('已加入粘贴文档');
  }
  function removeBatchFile(id: string) {
    batchFiles.value = batchFiles.value.filter(item => item.id !== id);
    persistConvertWorkspace();
  }
  function closeGithubProxyDialog() {
    githubProxyDialogOpen.value = false;
    githubProxyEnabled.value = true;
    githubProxyURL.value = 'https://gh-proxy.com/';
  }
  function confirmGithubProxyConvert() {
    void runConvertBatch(githubProxyEnabled.value ? normalizeProxyURL(githubProxyURL.value) : '');
  }
  async function analyzeBatch() {
    if (!batchFiles.value.length) {
      showError('请先添加 Markdown 文件');
      return;
    }
    loading.value = true;
    try {
      for (const file of batchFiles.value) {
        const data = await request<{ images: MarkdownImage[]; total: number }>('/api/convert/analyze', {
          method: 'POST',
          body: JSON.stringify({ content: file.content }),
        });
        file.images = data.images;
        file.status = 'analyzed';
      }
      showMessage(`已完成识别，共 ${totalImages.value} 个图片地址`);
      persistConvertWorkspace();
    } catch (err) {
      showError(err instanceof Error ? err.message : '批量识别失败');
    } finally {
      loading.value = false;
    }
  }
  async function convertBatch() {
    if (!convertForm.target_config_id) {
      showError('请先选择目标图床配置');
      return;
    }
    if (!batchFiles.value.length) {
      showError('请先添加 Markdown 文件');
      return;
    }
    if (!isBatchAnalyzed.value) {
      showError('请先点击批量识别');
      return;
    }
    if (!totalImages.value) {
      showError('未识别到图片，无需转换');
      return;
    }
    if (hasGithubImages.value) {
      githubProxyDialogOpen.value = true;
      githubProxyEnabled.value = true;
      githubProxyURL.value = githubProxyURL.value.trim() || 'https://gh-proxy.com/';
      return;
    }
    await runConvertBatch('');
  }
  async function pollConvertTask(taskId: number, initialDelay = 600) {
    const currentPollVersion = ++pollVersion;
    activeTaskId = taskId;
    persistConvertWorkspace(taskId);
    if (initialDelay > 0) await waitForNextPoll(initialDelay);

    while (currentPollVersion === pollVersion) {
      const data = await request<ConvertTaskData>(`/api/convert/tasks/${taskId}`);
      if (currentPollVersion !== pollVersion) return;
      syncTaskProgress(data.task);

      if (taskFinished(data.task)) {
        activeTaskId = 0;
        applyTaskRecords(data);
        const finalStatus = data.task.failed > 0 ? 'failed' : 'success';
        finishTaskProgress({
          status: finalStatus,
          message: data.task.message || `批量转换完成，成功 ${data.task.success} 个，失败 ${data.task.failed} 个`,
          detail: '可以关闭此窗口并下载已转换的文档。',
        });
        showMessage(`批量转换完成，成功 ${convertedCount.value} 个文件`);
        persistConvertWorkspace(0);
        await loadRecords();
        return;
      }

      await waitForNextPoll(1200);
    }
  }
  function restoreConvertWorkspace() {
    const state = readPersistedConvertWorkspace();
    if (!state) return false;
    convertForm.target_config_id = state.targetConfigId || defaultTarget.value?.id || 0;
    batchFiles.value = state.batchFiles;
    if (!state.activeTaskId) return batchFiles.value.length > 0;

    loading.value = true;
    startTaskProgress({
      title: '批量转换处理中',
      message: '正在恢复后台任务进度',
      detail: `已恢复 ${batchFiles.value.length} 个文档，任务会继续同步进度。`,
      total: Math.max(batchFiles.value.length, 1),
    });
    void pollConvertTask(state.activeTaskId, 0)
      .catch(err => {
        activeTaskId = state.activeTaskId || 0;
        persistConvertWorkspace(activeTaskId);
        finishTaskProgress({
          status: 'failed',
          message: '任务进度同步失败',
          detail: err instanceof Error ? err.message : '请稍后重试',
        });
        showError(err instanceof Error ? err.message : '任务进度同步失败');
      })
      .finally(() => {
        loading.value = false;
      });
    return true;
  }
  async function runConvertBatch(githubProxyURLForConvert: string) {
    loading.value = true;
    githubProxyDialogOpen.value = false;
    startTaskProgress({
      title: '批量转换处理中',
      message: '正在创建转换任务',
      detail: `共 ${batchFiles.value.length} 个文档，${totalImages.value} 个图片地址。`,
      total: batchFiles.value.length,
    });
    try {
      const payloadFiles = batchFiles.value.map(file => ({
        target_config_id: convertForm.target_config_id,
        filename: file.filename,
        content: githubProxyURLForConvert ? withGithubProxy(file.content, githubProxyURLForConvert) : file.content,
      }));
      const created = await request<{ task: ConversionTask }>('/api/convert/tasks', {
        method: 'POST',
        body: JSON.stringify({ target_config_id: convertForm.target_config_id, files: payloadFiles }),
      });
      const taskId = created.task.id;
      activeTaskId = taskId;
      persistConvertWorkspace(taskId);
      updateTaskProgress({ message: created.task.message || '转换任务已加入队列', current: 0, success: 0, failed: 0 });
      await pollConvertTask(taskId);
    } catch (err) {
      persistConvertWorkspace(activeTaskId);
      finishTaskProgress({
        status: 'failed',
        message: activeTaskId ? '任务进度同步失败' : '批量转换失败',
        detail: err instanceof Error ? err.message : '批量转换失败，请稍后重试',
      });
      showError(err instanceof Error ? err.message : '批量转换失败');
      await loadRecords();
    } finally {
      loading.value = false;
    }
  }
  async function downloadFile(file: BatchFile) {
    if (!file.convertedContent) return;
    const targetPath = await save({
      defaultPath: file.filename || 'converted.md',
      filters: [{ name: 'Markdown', extensions: ['md'] }],
    });
    if (!targetPath) return;
    await invoke('write_text_file', { path: targetPath, content: file.convertedContent });
    showMessage('转换后的文档已保存');
  }
  async function downloadAll() {
    const targets = batchFiles.value.filter(file => file.convertedContent);
    if (!targets.length) return;
    const directory = await open({ directory: true, title: '选择保存目录', multiple: false });
    if (typeof directory !== 'string' || !directory) return;
    for (const file of targets) {
      await invoke('write_text_file', { path: `${directory}/${file.filename || 'converted.md'}`, content: file.convertedContent });
    }
    showMessage(`已保存 ${targets.length} 个转换后的文档`);
  }
  function togglePreview(file: BatchFile) {
    file.previewOpen = !file.previewOpen;
  }
  function changedLines(file: BatchFile) {
    if (!file.convertedContent) return [];
    const before = file.content.split('\n');
    const after = file.convertedContent.split('\n');
    const rows = [] as Array<{ line: number; before: string; after: string }>;
    const max = Math.max(before.length, after.length);
    for (let index = 0; index < max; index += 1) {
      if ((before[index] || '') !== (after[index] || '')) rows.push({ line: index + 1, before: before[index] || '', after: after[index] || '' });
    }
    return rows.slice(0, 20);
  }
  return {
    convertForm,
    pasteForm,
    batchFiles,
    targetDropdownOpen,
    uploadDragActive,
    githubProxyDialogOpen,
    githubProxyEnabled,
    githubProxyURL,
    targetConfigs,
    defaultTarget,
    selectedTargetConfig,
    totalImages,
    convertedCount,
    canConvertBatch,
    hasGithubImages,
    statusLabel,
    targetConfigLabel,
    selectTargetConfig,
    restoreConvertWorkspace,
    stopConvertTaskPolling,
    resetConvertForm,
    handleFiles,
    handleFileDrop,
    addPastedDocument,
    removeBatchFile,
    analyzeBatch,
    convertBatch,
    closeGithubProxyDialog,
    confirmGithubProxyConvert,
    downloadFile,
    downloadAll,
    togglePreview,
    changedLines,
  };
}
