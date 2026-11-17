import { computed, ref, type Ref } from 'vue';
import { open, save } from '@tauri-apps/plugin-dialog';
import { createClientId } from './api';
import type {
  ConversionRecord,
  ConversionTask,
  LocalDocument,
  LocalImageFile,
  LocalImageFilter,
  MarkdownImage,
  PicbedConfig,
  ScannedLocalImage,
  TaskDocumentProgress,
  TaskKind,
  TaskProgressStatus,
} from './types';
import { t } from '../../i18n';
import { localizeBackendText } from '../../i18n/errorText';

type WorkspaceRequest = <T>(path: string, options?: RequestInit) => Promise<T>;
const LOCAL_UPLOAD_WORKSPACE_STORAGE_KEY = 'picbed_local_upload_workspace';

type LocalTaskData = { task: ConversionTask; records: ConversionRecord[]; documents?: TaskDocumentProgress[]; resumable?: boolean };

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
  startTaskProgress: (input: { title: string; message: string; total: number; detail?: string; taskId?: number; taskKind?: TaskKind }) => void;
  updateTaskProgress: (input: {
    message?: string;
    detail?: string;
    current?: number;
    total?: number;
    success?: number;
    partial?: number;
    failed?: number;
    documents?: TaskDocumentProgress[];
    taskId?: number;
    taskKind?: TaskKind;
    resumable?: boolean;
  }) => void;
  finishTaskProgress: (input: { status: Exclude<TaskProgressStatus, 'idle' | 'running'>; message: string; detail?: string; current?: number; resumable?: boolean }) => void;
};

const imageExtensions = new Set(['.png', '.jpg', '.jpeg', '.gif', '.webp', '.bmp', '.svg', '.avif']);

function taskFinished(task: ConversionTask) {
  return task.status === 'success' || task.status === 'failed' || task.status === 'interrupted' || task.status === 'cancelled';
}

function taskProgressValue(task: ConversionTask) {
  return Math.min(task.success + task.partial + task.failed, task.total);
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
  const localImageFilter = ref<LocalImageFilter>('all');
  let activeLocalTaskId = 0;
  let localPollVersion = 0;

  const localTargetConfigs = computed(() => configs.value);
  const localDefaultTarget = computed(() => localTargetConfigs.value.find(item => item.is_default) || localTargetConfigs.value[0]);
  const selectedLocalTargetConfig = computed(() =>
    localTargetConfigs.value.find(item => item.id === localTargetConfigId.value)
  );
  const localMatchedImages = computed(() => {
    const matchedKeys = new Set<string>();
    for (const document of localDocuments.value) {
      for (const reference of document.references) {
        const image = findLocalImage(reference.url);
        if (image) matchedKeys.add(image.key);
      }
    }
    return localImages.value.filter(image => matchedKeys.has(image.key));
  });
  const localMissingReferences = computed(() => {
    const references: string[] = [];
    for (const document of localDocuments.value) {
      for (const path of document.missing) {
        if (!references.includes(path)) references.push(path);
      }
    }
    return references;
  });
  const localMatchedCount = computed(() => localMatchedImages.value.length);
  const localMissingCount = computed(() => localMissingReferences.value.length);
  const localFilteredImages = computed(() =>
    localImageFilter.value === 'matched' ? localMatchedImages.value : localImages.value
  );
  const localConvertedCount = computed(() => localDocuments.value.filter(item => item.status === 'success').length);
  const canUploadLocalBatch = computed(
    () => localDocuments.value.length > 0 && localMatchedCount.value > 0 && localMissingCount.value === 0
  );

  function localTargetConfigLabel(config?: PicbedConfig) {
    return config ? `${config.config_name} · ${typeLabel(config.picbed_type)}` : t('common.pleaseSelect');
  }
  function selectLocalTargetConfig(id: number) {
    localTargetConfigId.value = id;
    localTargetDropdownOpen.value = false;
  }
  function setLocalImageFilter(filter: LocalImageFilter) {
    localImageFilter.value = filter;
  }
  function localStatusLabel(status: LocalDocument['status']) {
    return { ready: t('msg.local.statusReady'), analyzed: t('msg.local.statusAnalyzed'), success: t('msg.local.statusUploaded'), partial: t('msg.local.statusPartial'), failed: t('msg.statusFailed') }[status];
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
    localImageFilter.value = 'all';
    clearPersistedLocalUploadWorkspace();
  }
  async function addLocalDocuments(fileList: FileList | File[]) {
    const candidates = Array.from(fileList).filter(file => file.name.toLowerCase().endsWith('.md'));
    if (!candidates.length) {
      showError(t('msg.needMdFiles'));
      return;
    }
    const existing = new Set(localDocuments.value.map(document => document.filename.trim().toLowerCase()));
    const files = candidates.filter(file => {
      const key = file.name.trim().toLowerCase();
      if (existing.has(key)) return false;
      existing.add(key);
      return true;
    });
    const skipped = candidates.length - files.length;
    if (!files.length) {
      showMessage(t('msg.local.duplicateDocsSkipped', { count: skipped }));
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
    if (skipped > 0) showMessage(t('msg.local.addedDocsSkipped', { added: loaded.length, skipped }));
    else showMessage(t('msg.local.addedDocs', { count: loaded.length }));
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
    const candidates = Array.from(fileList).filter(isImageFile);
    if (!candidates.length) {
      showError(t('msg.local.needImages'));
      return;
    }
    const existing = new Set(
      localImages.value
        .filter(image => image.file)
        .map(image => `${image.file!.name}-${image.file!.size}-${image.file!.lastModified}`)
    );
    const files = candidates.filter(file => {
      const key = `${file.name}-${file.size}-${file.lastModified}`;
      if (existing.has(key)) return false;
      existing.add(key);
      return true;
    });
    const skipped = candidates.length - files.length;
    if (!files.length) {
      showMessage(t('msg.local.duplicateImagesSkipped', { count: skipped }));
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
    if (skipped > 0) showMessage(t('msg.local.addedImagesSkipped', { added: loaded.length, skipped }));
    else showMessage(t('msg.local.addedImages', { count: loaded.length }));
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
  async function scanLocalImageDirectory(): Promise<ScannedLocalImage[] | null> {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir !== 'string' || !dir) return null;
    loading.value = true;
    try {
      const data = await request<{ images: Array<{ name: string; path: string; relative_path: string }>; truncated?: boolean }>(
        '/api/local/scan-images',
        { method: 'POST', body: JSON.stringify({ path: dir }) }
      );
      if (data.truncated) showMessage(t('msg.local.scanTruncated'));
      return (data.images || []).map(image => ({
        name: image.name,
        path: image.path,
        relativePath: image.relative_path || image.name,
      }));
    } catch (error) {
      showError(error instanceof Error ? error.message : t('msg.local.scanFailed'));
      return null;
    } finally {
      loading.value = false;
    }
  }
  function addScannedLocalImages(images: ScannedLocalImage[]) {
    const existing = new Set(localImages.value.map(image => image.sourcePath || image.path));
    const added = images
      .filter(image => !existing.has(image.path))
      .map(image => ({
        key: `image_${createClientId()}`,
        name: image.name,
        path: image.relativePath,
        sourcePath: image.path,
      }));
    const skipped = images.length - added.length;
    if (!added.length) {
      showMessage(t('msg.local.duplicateImagesSkipped', { count: skipped }));
      return;
    }
    localImages.value = [...localImages.value, ...added];
    analyzeLocalMatches();
    if (skipped > 0) showMessage(t('msg.local.addedImagesSkipped', { added: added.length, skipped }));
    else showMessage(t('msg.local.addedImages', { count: added.length }));
  }
  function removeLocalDocument(id: string) {
    localDocuments.value = localDocuments.value.filter(item => item.id !== id);
    persistLocalUploadWorkspace();
  }
  function removeLocalImage(key: string) {
    localImages.value = localImages.value.filter(item => item.key !== key);
    analyzeLocalMatches();
  }
  function clearLocalDocuments() {
    const count = localDocuments.value.length;
    if (!count) return;
    localDocuments.value = [];
    persistLocalUploadWorkspace();
    showMessage(t('msg.local.clearedDocs', { count }));
  }
  function clearLocalImages() {
    const count = localImages.value.length;
    if (!count) return;
    localImages.value = [];
    localImageFilter.value = 'all';
    analyzeLocalMatches();
    showMessage(t('msg.local.clearedImages', { count }));
  }
  async function analyzeLocalBatch() {
    if (!localDocuments.value.length) {
      showError(t('msg.local.needDocs'));
      return;
    }
    analyzeLocalMatches();
    persistLocalUploadWorkspace();
    showMessage(t('msg.local.matchedCount', { count: localMatchedCount.value }));
  }
  function syncLocalTaskProgress(task: ConversionTask, documents?: TaskDocumentProgress[]) {
    updateTaskProgress({
      current: taskProgressValue(task),
      total: task.total,
      success: task.success,
      partial: task.partial,
      failed: task.failed,
      message: localizeBackendText(task.message) || t('msg.local.taskRunning'),
      documents,
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
        document.error = t('msg.local.noResult');
        document.details = [];
        continue;
      }
      document.status = record.status === 'success' ? 'success' : record.status === 'partial' ? 'partial' : 'failed';
      document.convertedContent = record.converted_content || '';
      document.changed = record.image_count || 0;
      document.error = record.error_message || '';
      document.details = record.details || [];
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
      syncLocalTaskProgress(data.task, data.documents);

      if (taskFinished(data.task)) {
        activeLocalTaskId = 0;
        applyLocalTaskRecords(data);
        if (data.task.status === 'interrupted') {
          activeLocalTaskId = taskId;
          persistLocalUploadWorkspace(taskId);
          const message = t('msg.local.progressInterrupted');
          finishTaskProgress({
            status: 'interrupted',
            message,
            detail: t('msg.taskInterruptedHint'),
            current: data.task.success + data.task.partial + data.task.failed,
            resumable: data.resumable === true,
          });
          showError(message);
        } else if (data.task.status === 'cancelled') {
          persistLocalUploadWorkspace(0);
          finishTaskProgress({
            status: 'cancelled',
            message: t('msg.taskCancelled'),
            current: data.task.success + data.task.partial + data.task.failed,
          });
          showMessage(t('msg.taskCancelled'));
        } else {
          persistLocalUploadWorkspace(0);
          const finalStatus = data.task.failed > 0 ? 'failed' : 'success';
          const message = localizeBackendText(data.task.message) || t('msg.local.taskDone', { success: data.task.success, failed: data.task.failed });
          finishTaskProgress({
            status: finalStatus,
            message,
            detail: t('msg.local.taskDoneHint'),
          });
          if (finalStatus === 'success') {
            if (data.task.partial > 0) showMessage(t('msg.local.batchDonePartial', { success: localConvertedCount.value, partial: data.task.partial }));
            else showMessage(t('msg.local.batchDone', { count: localConvertedCount.value }));
          } else {
            showError(message);
          }
        }
        await loadRecords();
        return;
      }

      await waitForNextPoll(1200);
    }
  }
  async function uploadLocalBatch() {
    if (!localTargetConfigId.value) {
      showError(t('msg.needTarget'));
      return;
    }
    if (!localDocuments.value.length) {
      showError(t('msg.local.needDocs'));
      return;
    }
    analyzeLocalMatches();
    if (localMissingCount.value > 0) {
      showError(t('msg.local.hasMissing'));
      return;
    }
    if (!localMatchedCount.value) {
      showError(t('msg.local.noMatched'));
      return;
    }
    loading.value = true;
    startTaskProgress({
      taskKind: 'localUpload',
      title: t('msg.local.progressTitle'),
      message: t('msg.local.creating'),
      detail: t('msg.local.progressDetail', { documents: localDocuments.value.length, images: localMatchedCount.value }),
      total: localDocuments.value.length,
    });
    try {
      const manifest = buildLocalTaskManifest();
      const formData = new FormData();
      formData.append('manifest', JSON.stringify({ target_config_id: localTargetConfigId.value, documents: manifest.documents }));
      const pathImages = manifest.images
        .filter(image => !image.file && image.sourcePath)
        .map(image => ({ key: image.key, name: image.name, source_path: image.sourcePath }));
      for (const image of manifest.images) {
        if (image.file) formData.append(image.key, image.file, image.name);
      }
      if (pathImages.length) formData.append('path_images', JSON.stringify(pathImages));
      const created = await request<{ task: ConversionTask }>('/api/convert/local-tasks', {
        method: 'POST',
        body: formData,
      });
      const taskId = created.task.id;
      activeLocalTaskId = taskId;
      persistLocalUploadWorkspace(taskId);
      localImages.value = [];
      localImageFilter.value = 'all';
      updateTaskProgress({ taskId, taskKind: 'localUpload', message: localizeBackendText(created.task.message) || t('msg.local.taskQueued'), current: 0, success: 0, failed: 0 });
      await pollLocalUploadTask(taskId);
    } catch (err) {
      persistLocalUploadWorkspace(activeLocalTaskId);
      finishTaskProgress({
        status: 'failed',
        message: activeLocalTaskId ? t('msg.local.progressSyncFailed') : t('msg.local.uploadFailed'),
        detail: err instanceof Error ? err.message : t('msg.local.uploadFailedHint'),
      });
      showError(err instanceof Error ? err.message : t('msg.local.uploadFailed'));
      await loadRecords();
    } finally {
      loading.value = false;
    }
  }
  async function resumeLocalUploadTask(taskId: number) {
    loading.value = true;
    startTaskProgress({
      taskId,
      taskKind: 'localUpload',
      title: t('msg.local.progressTitle'),
      message: t('msg.taskResuming'),
      total: Math.max(localDocuments.value.length, 1),
    });
    try {
      await request(`/api/convert/tasks/${taskId}/resume`, { method: 'POST' });
    } catch (err) {
      persistLocalUploadWorkspace(taskId);
      finishTaskProgress({
        status: 'interrupted',
        message: t('msg.taskResumeFailed'),
        detail: err instanceof Error ? err.message : t('msg.retryHint'),
        resumable: true,
      });
      showError(err instanceof Error ? err.message : t('msg.taskResumeFailed'));
      loading.value = false;
      return;
    }
    try {
      persistLocalUploadWorkspace(taskId);
      await pollLocalUploadTask(taskId, 0);
    } catch (err) {
      persistLocalUploadWorkspace(taskId);
      finishTaskProgress({
        status: 'failed',
        message: t('msg.local.progressSyncFailed'),
        detail: err instanceof Error ? err.message : t('msg.retryHint'),
      });
      showError(err instanceof Error ? err.message : t('msg.local.progressSyncFailed'));
    } finally {
      loading.value = false;
    }
  }
  async function cancelLocalUploadTask(taskId: number): Promise<boolean> {
    try {
      await request(`/api/convert/tasks/${taskId}/cancel`, { method: 'POST' });
      updateTaskProgress({ message: t('msg.taskCancelling') });
      return true;
    } catch (err) {
      showError(err instanceof Error ? err.message : t('msg.taskCancelFailed'));
      return false;
    }
  }
  function clearActiveLocalUploadTaskMarker() {
    activeLocalTaskId = 0;
    persistLocalUploadWorkspace(0);
  }
  async function downloadLocalFile(item: LocalDocument) {
    if (!item.convertedContent) return;
    try {
      const targetPath = await save({
        defaultPath: item.filename || 'converted.md',
        filters: [{ name: 'Markdown', extensions: ['md'] }],
      });
      if (!targetPath) return;
      await request('/api/files/write', {
        method: 'POST',
        body: JSON.stringify({ path: targetPath, content: item.convertedContent }),
      });
      showMessage(t('msg.local.saved'));
    } catch (err) {
      showError(err instanceof Error ? err.message : t('records.saveFailed'));
    }
  }
  async function downloadAllLocalFiles() {
    const targets = localDocuments.value.filter(document => document.convertedContent);
    if (!targets.length) return;
    try {
      const directory = await open({ directory: true, title: t('msg.chooseSaveDir'), multiple: false });
      if (typeof directory !== 'string' || !directory) return;
      for (const document of targets) {
        await request('/api/files/write', {
          method: 'POST',
          body: JSON.stringify({ path: `${directory}/${document.filename || 'converted.md'}`, content: document.convertedContent }),
        });
      }
      showMessage(t('msg.local.savedCount', { count: targets.length }));
    } catch (err) {
      showError(err instanceof Error ? err.message : t('records.saveFailed'));
    }
  }
  function restoreLocalUploadWorkspace(): 'localUpload' | null {
    const state = readPersistedLocalUploadWorkspace();
    if (!state) return null;
    localTargetConfigId.value = state.targetConfigId || localDefaultTarget.value?.id || 0;
    localDocuments.value = state.localDocuments;
    localImages.value = [];
    analyzeLocalMatches();
    if (!state.activeTaskId) return null;

    loading.value = true;
    startTaskProgress({
      taskId: state.activeTaskId,
      taskKind: 'localUpload',
      title: t('msg.local.progressTitle'),
      message: t('msg.convert.progressRestoring'),
      detail: t('msg.convert.progressRestored', { count: localDocuments.value.length }),
      total: Math.max(localDocuments.value.length, 1),
    });
    void pollLocalUploadTask(state.activeTaskId, 0)
      .catch(err => {
        activeLocalTaskId = state.activeTaskId || 0;
        persistLocalUploadWorkspace(activeLocalTaskId);
        finishTaskProgress({
          status: 'failed',
          message: t('msg.local.progressSyncFailed'),
          detail: err instanceof Error ? err.message : t('msg.retryHint'),
        });
        showError(err instanceof Error ? err.message : t('msg.local.progressSyncFailed'));
      })
      .finally(() => {
        loading.value = false;
      });
    return 'localUpload';
  }
  function analyzeLocalMatches() {
    for (const document of localDocuments.value) {
      const references = extractMarkdownImages(document.content).filter(image => isLocalImageReference(image.url));
      const uniqueSources: string[] = [];
      const seen = new Set<string>();
      for (const reference of references) {
        const source = reference.url.trim();
        if (seen.has(source)) continue;
        seen.add(source);
        uniqueSources.push(source);
      }
      const missing: string[] = [];
      let matched = 0;
      for (const source of uniqueSources) {
        if (findLocalImage(source)) matched += 1;
        else missing.push(source);
      }
      document.references = references;
      document.matched = matched;
      document.missing = missing;
      document.status = document.status === 'success' || document.status === 'partial' || document.status === 'failed' ? document.status : 'analyzed';
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
    localImageFilter,
    localFilteredImages,
    localMatchedImages,
    localMissingReferences,
    localTargetConfigs,
    selectedLocalTargetConfig,
    localMatchedCount,
    localMissingCount,
    localConvertedCount,
    canUploadLocalBatch,
    localTargetConfigLabel,
    selectLocalTargetConfig,
    setLocalImageFilter,
    localStatusLabel,
    resetLocalUploadForm,
    handleLocalDocumentFiles,
    handleLocalDocumentDrop,
    handleLocalImageFiles,
    handleLocalImageDrop,
    scanLocalImageDirectory,
    addScannedLocalImages,
    removeLocalDocument,
    removeLocalImage,
    clearLocalDocuments,
    clearLocalImages,
    analyzeLocalBatch,
    uploadLocalBatch,
    restoreLocalUploadWorkspace,
    resumeLocalUploadTask,
    cancelLocalUploadTask,
    clearActiveLocalUploadTaskMarker,
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
