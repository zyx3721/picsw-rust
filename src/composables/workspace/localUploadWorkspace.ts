import { computed, ref, type Ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';
import { createClientId } from './api';
import type { ConversionRecord, ConversionTask, LocalDocument, LocalImageFile, MarkdownImage, PicbedConfig } from './types';

type WorkspaceRequest = <T>(path: string, options?: RequestInit) => Promise<T>;
const LOCAL_UPLOAD_WORKSPACE_STORAGE_KEY = 'picbed_local_upload_workspace';

type LocalTaskData = { task: ConversionTask; records: ConversionRecord[] };

type PersistedLocalUploadWorkspace = {
  targetConfigId: number;
  localDocuments: LocalDocument[];
  activeTaskId?: number;
  savedAt: number;
};

type LocalUploadWorkspaceDeps = {
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

const imageExtensions = new Set(['.png', '.jpg', '.jpeg', '.gif', '.webp', '.bmp', '.svg', '.avif']);

function taskFinished(task: ConversionTask) {
  return task.status === 'success' || task.status === 'failed';
}

function taskProgressValue(task: ConversionTask) {
  return Math.min(task.success + task.failed, task.total);
}

function waitForNextPoll(ms: number) {
  return new Promise(resolve => window.setTimeout(resolve, ms));
}

export function useWorkspaceLocalUpload({
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
}: LocalUploadWorkspaceDeps) {
  const localTargetConfigId = ref(0);
  const localTargetDropdownOpen = ref(false);
  const localDocumentDragActive = ref(false);
  const localImageDragActive = ref(false);
  const localDocuments = ref<LocalDocument[]>([]);
  const localImages = ref<LocalImageFile[]>([]);
  let activeLocalTaskId = 0;
  let localPollVersion = 0;

  const localTargetConfigs = computed(() => configs.value);
  const localDefaultTarget = computed(() => localTargetConfigs.value.find(item => item.is_default) || localTargetConfigs.value[0]);
  const selectedLocalTargetConfig = computed(() =>
    localTargetConfigs.value.find(item => item.id === localTargetConfigId.value)
  );
  const localMatchedCount = computed(() => localDocuments.value.reduce((sum, item) => sum + item.matched, 0));
  const localMissingCount = computed(() => localDocuments.value.reduce((sum, item) => sum + item.missing.length, 0));
  const localConvertedCount = computed(() => localDocuments.value.filter(item => item.status === 'success').length);
  const canUploadLocalBatch = computed(
    () => localDocuments.value.length > 0 && localMatchedCount.value > 0 && localMissingCount.value === 0
  );

  function localTargetConfigLabel(config?: PicbedConfig) {
    return config ? `${config.config_name} · ${typeLabel(config.picbed_type)}` : '请选择';
  }
  function selectLocalTargetConfig(id: number) {
    localTargetConfigId.value = id;
    localTargetDropdownOpen.value = false;
  }
  function localStatusLabel(status: LocalDocument['status']) {
    return { ready: '待匹配', analyzed: '已匹配', success: '已上传', failed: '失败' }[status];
  }
  function readPersistedLocalUploadWorkspace() {
    try {
      const raw = localStorage.getItem(LOCAL_UPLOAD_WORKSPACE_STORAGE_KEY);
      if (!raw) return null;
      const state = JSON.parse(raw) as PersistedLocalUploadWorkspace;
      if (!Array.isArray(state.localDocuments)) return null;
      return state;
    } catch {
      return null;
    }
  }
  function persistLocalUploadWorkspace(taskId = activeLocalTaskId) {
    try {
      const state: PersistedLocalUploadWorkspace = {
        targetConfigId: localTargetConfigId.value,
        localDocuments: localDocuments.value,
        activeTaskId: taskId || undefined,
        savedAt: Date.now(),
      };
      localStorage.setItem(LOCAL_UPLOAD_WORKSPACE_STORAGE_KEY, JSON.stringify(state));
    } catch {
      // Browser storage may be unavailable; the backend task still continues.
    }
  }
  function clearPersistedLocalUploadWorkspace() {
    localStorage.removeItem(LOCAL_UPLOAD_WORKSPACE_STORAGE_KEY);
  }
  function stopLocalUploadTaskPolling() {
    localPollVersion += 1;
    activeLocalTaskId = 0;
  }
  function resetLocalUploadForm() {
    if (activeLocalTaskId) return;
    clearNotice();
    stopLocalUploadTaskPolling();
    localTargetConfigId.value = localDefaultTarget.value?.id || 0;
    localTargetDropdownOpen.value = false;
    localDocumentDragActive.value = false;
    localImageDragActive.value = false;
    localDocuments.value = [];
    localImages.value = [];
    clearPersistedLocalUploadWorkspace();
  }
  async function addLocalDocuments(fileList: FileList | File[]) {
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
        references: [] as MarkdownImage[],
        matched: 0,
        missing: [],
        convertedContent: '',
        changed: 0,
        status: 'ready' as const,
        error: '',
      }))
    );
    localDocuments.value = [...localDocuments.value, ...loaded];
    analyzeLocalMatches();
    persistLocalUploadWorkspace();
    showMessage(`已加入 ${loaded.length} 个 Markdown 文档`);
  }
  async function handleLocalDocumentFiles(event: Event) {
    const input = event.target as HTMLInputElement;
    await addLocalDocuments(input.files || []);
    input.value = '';
  }
  async function handleLocalDocumentDrop(event: DragEvent) {
    localDocumentDragActive.value = false;
    await addLocalDocuments(event.dataTransfer?.files || []);
  }
  function addLocalImages(fileList: FileList | File[]) {
    const files = Array.from(fileList).filter(isImageFile);
    if (!files.length) {
      showError('请上传本地图片文件或图片目录');
      return;
    }
    const loaded = files.map(file => ({
      key: `image_${createClientId()}`,
      name: file.name,
      path: filePath(file),
      file,
    }));
    localImages.value = [...localImages.value, ...loaded];
    analyzeLocalMatches();
    showMessage(`已加入 ${loaded.length} 张本地图片`);
  }
  function handleLocalImageFiles(event: Event) {
    const input = event.target as HTMLInputElement;
    addLocalImages(input.files || []);
    input.value = '';
  }
  function handleLocalImageDrop(event: DragEvent) {
    localImageDragActive.value = false;
    addLocalImages(event.dataTransfer?.files || []);
  }
  function removeLocalDocument(id: string) {
    localDocuments.value = localDocuments.value.filter(item => item.id !== id);
    persistLocalUploadWorkspace();
  }
  function removeLocalImage(key: string) {
    localImages.value = localImages.value.filter(item => item.key !== key);
    analyzeLocalMatches();
  }
  async function analyzeLocalBatch() {
    if (!localDocuments.value.length) {
      showError('请先添加 Markdown 文档');
      return;
    }
    analyzeLocalMatches();
    persistLocalUploadWorkspace();
    showMessage(`已匹配 ${localMatchedCount.value} 张本地图片`);
  }
  function syncLocalTaskProgress(task: ConversionTask) {
    updateTaskProgress({
      current: taskProgressValue(task),
      success: task.success,
      failed: task.failed,
      message: task.message || '本地上传任务执行中',
    });
  }
  function applyLocalTaskRecords(data: LocalTaskData) {
    const recordsByFilename = new Map<string, ConversionRecord[]>();
    for (const record of data.records) {
      const filename = record.original_filename || '';
      recordsByFilename.set(filename, [...(recordsByFilename.get(filename) || []), record]);
    }
    const canFallbackToPosition = data.records.length === localDocuments.value.length;

    for (const [index, document] of localDocuments.value.entries()) {
      const filenameRecords = recordsByFilename.get(document.filename) || [];
      const record = filenameRecords.shift() || (canFallbackToPosition ? data.records[index] : undefined);
      if (filenameRecords.length) recordsByFilename.set(document.filename, filenameRecords);
      else recordsByFilename.delete(document.filename);

      if (!record) {
        document.status = 'failed';
        document.convertedContent = '';
        document.changed = 0;
        document.error = '未找到上传替换结果';
        continue;
      }
      document.status = record.status === 'success' ? 'success' : 'failed';
      document.convertedContent = record.converted_content || '';
      document.changed = record.image_count || 0;
      document.error = record.error_message || '';
    }
  }
  async function pollLocalUploadTask(taskId: number, initialDelay = 600) {
    const currentPollVersion = ++localPollVersion;
    activeLocalTaskId = taskId;
    persistLocalUploadWorkspace(taskId);
    if (initialDelay > 0) await waitForNextPoll(initialDelay);

    while (currentPollVersion === localPollVersion) {
      const data = await request<LocalTaskData>(`/api/convert/tasks/${taskId}`);
      if (currentPollVersion !== localPollVersion) return;
      syncLocalTaskProgress(data.task);

      if (taskFinished(data.task)) {
        activeLocalTaskId = 0;
        applyLocalTaskRecords(data);
        const finalStatus = data.task.failed > 0 ? 'failed' : 'success';
        finishTaskProgress({
          status: finalStatus,
          message: data.task.message || `本地图片上传完成，成功 ${data.task.success} 个，失败 ${data.task.failed} 个`,
          detail: '可以关闭此窗口并下载替换后的文档。',
        });
        showMessage(`本地图片上传完成，成功 ${localConvertedCount.value} 个文档`);
        persistLocalUploadWorkspace(0);
        await loadRecords();
        return;
      }

      await waitForNextPoll(1200);
    }
  }
  async function uploadLocalBatch() {
    if (!localTargetConfigId.value) {
      showError('请先选择目标图床配置');
      return;
    }
    if (!localDocuments.value.length) {
      showError('请先添加 Markdown 文档');
      return;
    }
    analyzeLocalMatches();
    if (localMissingCount.value > 0) {
      showError('仍有本地图片未匹配，请补充图片文件或目录');
      return;
    }
    if (!localMatchedCount.value) {
      showError('未匹配到本地图片，无需上传');
      return;
    }
    loading.value = true;
    startTaskProgress({
      title: '本地上传替换中',
      message: '正在创建本地上传任务',
      detail: `共 ${localDocuments.value.length} 个文档，${localMatchedCount.value} 张本地图片。`,
      total: localDocuments.value.length,
    });
    try {
      const manifest = buildLocalTaskManifest();
      const formData = new FormData();
      formData.append('manifest', JSON.stringify({ target_config_id: localTargetConfigId.value, documents: manifest.documents }));
      for (const image of manifest.images) formData.append(image.key, image.file, image.name);
      const created = await request<{ task: ConversionTask }>('/api/convert/local-tasks', {
        method: 'POST',
        body: formData,
      });
      const taskId = created.task.id;
      activeLocalTaskId = taskId;
      persistLocalUploadWorkspace(taskId);
      localImages.value = [];
      updateTaskProgress({ message: created.task.message || '本地上传任务已加入队列', current: 0, success: 0, failed: 0 });
      await pollLocalUploadTask(taskId);
    } catch (err) {
      persistLocalUploadWorkspace(activeLocalTaskId);
      finishTaskProgress({
        status: 'failed',
        message: activeLocalTaskId ? '本地上传任务进度同步失败' : '本地图片上传失败',
        detail: err instanceof Error ? err.message : '本地图片上传失败，请稍后重试',
      });
      showError(err instanceof Error ? err.message : '本地图片上传失败');
      await loadRecords();
    } finally {
      loading.value = false;
    }
  }
  async function downloadLocalFile(item: LocalDocument) {
    if (!item.convertedContent) return;
    const targetPath = await save({
      defaultPath: item.filename || 'converted.md',
      filters: [{ name: 'Markdown', extensions: ['md'] }],
    });
    if (!targetPath) return;
    await invoke('write_text_file', { path: targetPath, content: item.convertedContent });
    showMessage('替换后的文档已保存');
  }
  async function downloadAllLocalFiles() {
    const targets = localDocuments.value.filter(document => document.convertedContent);
    if (!targets.length) return;
    const directory = await open({ directory: true, title: '选择保存目录', multiple: false });
    if (typeof directory !== 'string' || !directory) return;
    for (const document of targets) {
      await invoke('write_text_file', { path: `${directory}/${document.filename || 'converted.md'}`, content: document.convertedContent });
    }
    showMessage(`已保存 ${targets.length} 个替换后的文档`);
  }
  function restoreLocalUploadWorkspace() {
    const state = readPersistedLocalUploadWorkspace();
    if (!state) return false;
    localTargetConfigId.value = state.targetConfigId || localDefaultTarget.value?.id || 0;
    localDocuments.value = state.localDocuments;
    localImages.value = [];
    if (!state.activeTaskId) return localDocuments.value.length > 0;

    loading.value = true;
    startTaskProgress({
      title: '本地上传替换中',
      message: '正在恢复后台任务进度',
      detail: `已恢复 ${localDocuments.value.length} 个文档，任务会继续同步进度。`,
      total: Math.max(localDocuments.value.length, 1),
    });
    void pollLocalUploadTask(state.activeTaskId, 0)
      .catch(err => {
        activeLocalTaskId = state.activeTaskId || 0;
        persistLocalUploadWorkspace(activeLocalTaskId);
        finishTaskProgress({
          status: 'failed',
          message: '本地上传任务进度同步失败',
          detail: err instanceof Error ? err.message : '请稍后重试',
        });
        showError(err instanceof Error ? err.message : '本地上传任务进度同步失败');
      })
      .finally(() => {
        loading.value = false;
      });
    return true;
  }
  function analyzeLocalMatches() {
    for (const document of localDocuments.value) {
      const references = extractMarkdownImages(document.content).filter(image => isLocalImageReference(image.url));
      const missing: string[] = [];
      let matched = 0;
      for (const reference of references) {
        if (findLocalImage(reference.url)) matched += 1;
        else missing.push(reference.url);
      }
      document.references = references;
      document.matched = matched;
      document.missing = [...new Set(missing)];
      document.status = document.status === 'success' || document.status === 'failed' ? document.status : 'analyzed';
      document.error = '';
    }
  }
  function buildSingleLocalManifest(document: LocalDocument) {
    const usedImages = new Map<string, LocalImageFile>();
    const item = {
      filename: document.filename,
      content: document.content,
      images: document.references.flatMap(reference => {
        const image = findLocalImage(reference.url);
        if (!image) return [];
        usedImages.set(image.key, image);
        return [{ source: reference.url, file_key: image.key }];
      }),
    };
    return { document: item, images: Array.from(usedImages.values()) };
  }
  function buildLocalTaskManifest() {
    const documents = [] as Array<{ filename: string; content: string; images: Array<{ source: string; file_key: string }> }>;
    const usedImages = new Map<string, LocalImageFile>();
    for (const document of localDocuments.value) {
      const manifest = buildSingleLocalManifest(document);
      documents.push(manifest.document);
      for (const image of manifest.images) usedImages.set(image.key, image);
    }
    return { documents, images: Array.from(usedImages.values()) };
  }
  function findLocalImage(source: string) {
    const normalizedSource = normalizeLocalPath(source);
    const sourceBase = baseName(normalizedSource);
    return (
      localImages.value.find(image => normalizeLocalPath(image.path) === normalizedSource) ||
      localImages.value.find(image => normalizeLocalPath(image.path).endsWith('/' + normalizedSource)) ||
      localImages.value.find(image => baseName(image.path).toLowerCase() === sourceBase.toLowerCase())
    );
  }

  return {
    localTargetConfigId,
    localTargetDropdownOpen,
    localDocumentDragActive,
    localImageDragActive,
    localDocuments,
    localImages,
    localTargetConfigs,
    selectedLocalTargetConfig,
    localMatchedCount,
    localMissingCount,
    localConvertedCount,
    canUploadLocalBatch,
    localTargetConfigLabel,
    selectLocalTargetConfig,
    localStatusLabel,
    resetLocalUploadForm,
    handleLocalDocumentFiles,
    handleLocalDocumentDrop,
    handleLocalImageFiles,
    handleLocalImageDrop,
    removeLocalDocument,
    removeLocalImage,
    analyzeLocalBatch,
    uploadLocalBatch,
    restoreLocalUploadWorkspace,
    stopLocalUploadTaskPolling,
    downloadLocalFile,
    downloadAllLocalFiles,
  };
}

function extractMarkdownImages(content: string): MarkdownImage[] {
  const images: MarkdownImage[] = [];
  const pattern = /!\[([^\]]*)\]\(([^\s)]+)(?:\s+"[^"]*")?\)|<img[^>]+src=["']([^"']+)["'][^>]*>/gi;
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(content))) {
    const url = match[2] || match[3] || '';
    images.push({ raw: match[0], url, alt: match[1] || '', picbed: isLocalImageReference(url) ? 'local' : 'other' });
  }
  return images;
}

function isImageFile(file: File) {
  if (file.type.startsWith('image/')) return true;
  const name = file.name.toLowerCase();
  return Array.from(imageExtensions).some(ext => name.endsWith(ext));
}

function isLocalImageReference(raw: string) {
  const value = raw.trim().toLowerCase();
  return Boolean(value) && !value.startsWith('http://') && !value.startsWith('https://') && !value.startsWith('data:');
}

function filePath(file: File) {
  const entryPath = 'webkitRelativePath' in file ? String(file.webkitRelativePath || '') : '';
  return entryPath || file.name;
}

function normalizeLocalPath(raw: string) {
  let value = decodeURIComponent(raw.trim());
  value = value.replace(/^file:\/\/\/?/i, '');
  value = value.replace(/\\/g, '/');
  while (value.startsWith('./')) value = value.slice(2);
  return value.replace(/^\/+|\/+$/g, '').toLowerCase();
}

function baseName(raw: string) {
  const normalized = normalizeLocalPath(raw);
  const parts = normalized.split('/').filter(Boolean);
  return parts[parts.length - 1] || normalized;
}
