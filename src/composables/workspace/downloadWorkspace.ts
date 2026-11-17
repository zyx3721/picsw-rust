import { computed, ref, type Ref } from 'vue';
import { open } from '@tauri-apps/plugin-dialog';
import { createClientId } from './api';
import type {
  ConversionTask,
  DownloadDocument,
  DownloadImageFilter,
  DownloadImageItem,
  DownloadTaskImage,
  MarkdownImage,
  TaskDocumentProgress,
  TaskKind,
  TaskProgressStatus,
} from './types';
import { t } from '../../i18n';
import { localizeBackendText } from '../../i18n/errorText';

type WorkspaceRequest = <T>(path: string, options?: RequestInit) => Promise<T>;

const DOWNLOAD_WORKSPACE_STORAGE_KEY = 'picbed_download_workspace';
const LEGACY_DOWNLOAD_DIR_STORAGE_KEY = 'picbed_download_target_dir';

type DownloadTaskData = {
  task: ConversionTask;
  documents?: TaskDocumentProgress[];
  images?: DownloadTaskImage[];
  resumable?: boolean;
};

type PersistedDownloadWorkspace = {
  documents: DownloadDocument[];
  targetDir: string;
  activeTaskId?: number;
  savedAt: number;
};

type DownloadWorkspaceDeps = {
  request: WorkspaceRequest;
  showMessage: (text: string) => void;
  showError: (text: string) => void;
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

function decodeURIComponentSafe(value: string) {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

function urlFileName(url: string) {
  const cleanPath = url.split(/[?#]/)[0];
  const last = cleanPath.split('/').filter(Boolean).pop() || 'image';
  return decodeURIComponentSafe(last);
}

function taskFinished(task: ConversionTask) {
  return task.status === 'success' || task.status === 'failed' || task.status === 'interrupted' || task.status === 'cancelled';
}

function waitForNextPoll(ms: number) {
  return new Promise(resolve => window.setTimeout(resolve, ms));
}

export function useWorkspaceDownload({
  request,
  showMessage,
  showError,
  loading,
  startTaskProgress,
  updateTaskProgress,
  finishTaskProgress,
}: DownloadWorkspaceDeps) {
  const downloadDocuments = ref<DownloadDocument[]>([]);
  const downloadDragActive = ref(false);
  const downloadItems = ref<DownloadImageItem[]>([]);
  const downloadTargetDir = ref('');
  const downloadActive = ref(false);
  const downloadFilter = ref<DownloadImageFilter>('all');
  let activeDownloadTaskId = 0;
  let downloadPollVersion = 0;

  localStorage.removeItem(LEGACY_DOWNLOAD_DIR_STORAGE_KEY);

  const downloadTotalCount = computed(() => downloadItems.value.length);
  const downloadSuccessCount = computed(
    () => downloadItems.value.filter(item => item.status === 'success').length
  );
  const downloadFailedCount = computed(
    () => downloadItems.value.filter(item => item.status === 'failed').length
  );
  const downloadFilteredItems = computed(() =>
    downloadFilter.value === 'all'
      ? downloadItems.value
      : downloadItems.value.filter(item => item.status === downloadFilter.value)
  );
  const canDownloadBatch = computed(
    () =>
      !downloadActive.value &&
      downloadItems.value.length > 0 &&
      downloadTargetDir.value.trim().length > 0
  );

  function setDownloadFilter(filter: DownloadImageFilter) {
    downloadFilter.value = filter;
  }

  function downloadStatusLabel(status: DownloadDocument['status']) {
    return {
      ready: t('download.docStatusReady'),
      analyzed: t('download.docStatusAnalyzed'),
      success: t('download.docStatusSuccess'),
      partial: t('download.docStatusPartial'),
      failed: t('download.docStatusFailed'),
    }[status];
  }

  function readPersistedDownloadWorkspace() {
    try {
      const raw = localStorage.getItem(DOWNLOAD_WORKSPACE_STORAGE_KEY);
      if (!raw) return null;
      const state = JSON.parse(raw) as PersistedDownloadWorkspace;
      if (!Array.isArray(state.documents)) return null;
      return state;
    } catch {
      return null;
    }
  }

  function persistDownloadWorkspace(taskId = activeDownloadTaskId) {
    try {
      const state: PersistedDownloadWorkspace = {
        documents: downloadDocuments.value,
        targetDir: downloadTargetDir.value,
        activeTaskId: taskId || undefined,
        savedAt: Date.now(),
      };
      localStorage.setItem(DOWNLOAD_WORKSPACE_STORAGE_KEY, JSON.stringify(state));
    } catch {
      // 本地存储不可用时任务仍由后端继续执行
    }
  }

  function clearPersistedDownloadWorkspace() {
    localStorage.removeItem(DOWNLOAD_WORKSPACE_STORAGE_KEY);
  }

  function rebuildItems() {
    const items: DownloadImageItem[] = [];
    const seen = new Set<string>();
    for (const doc of downloadDocuments.value) {
      for (const image of doc.images) {
        const url = image.url.trim();
        if (!/^https?:\/\//i.test(url) || seen.has(url)) continue;
        seen.add(url);
        items.push({
          key: `${doc.id}-${url}`,
          docId: doc.id,
          name: urlFileName(url),
          url,
          picbed: image.picbed,
          status: 'pending',
          error: '',
        });
      }
    }
    downloadItems.value = items;
  }

  function applyTaskImages(images?: DownloadTaskImage[]) {
    if (!images?.length) return;
    const byUrl = new Map(images.map(image => [image.url, image]));
    for (const item of downloadItems.value) {
      const row = byUrl.get(item.url);
      if (!row) continue;
      if (row.status === 'success') {
        item.status = 'success';
        item.error = '';
        item.savedPath = row.saved_path;
      } else if (row.status === 'failed') {
        item.status = 'failed';
        item.error = row.error || t('msg.download.failed');
      } else if (row.status === 'cancelled') {
        item.status = 'cancelled';
        item.error = '';
      } else if (row.status === 'running') {
        item.status = 'downloading';
        item.error = '';
      } else {
        item.status = 'pending';
        item.error = '';
      }
    }
  }

  function applyTaskDocuments(documents?: TaskDocumentProgress[]) {
    if (!documents?.length) return;
    for (const [index, document] of downloadDocuments.value.entries()) {
      const progress = documents[index];
      if (!progress) continue;
      if (progress.status === 'success' || progress.status === 'partial' || progress.status === 'failed') {
        document.status = progress.status;
      }
    }
  }

  async function analyzeDownloadDocuments() {
    if (downloadDocuments.value.length === 0) {
      downloadItems.value = [];
      return;
    }
    loading.value = true;
    try {
      for (const doc of downloadDocuments.value) {
        const data = await request<{ images: MarkdownImage[]; total: number }>('/api/convert/analyze', {
          method: 'POST',
          body: JSON.stringify({ content: doc.content }),
        });
        doc.images = (data.images || []).filter(image => /^https?:\/\//i.test(image.url.trim()));
        doc.status = 'analyzed';
        doc.error = '';
      }
      rebuildItems();
    } catch (error) {
      showError(error instanceof Error ? error.message : t('msg.download.analyzeFailed'));
    } finally {
      loading.value = false;
    }
  }

  async function addDownloadDocuments(files: File[]) {
    const markdownFiles = files.filter(file => /\.md$/i.test(file.name) || file.type === 'text/markdown');
    if (markdownFiles.length === 0) return;
    loading.value = true;
    try {
      for (const file of markdownFiles) {
        const content = await file.text();
        const existing = downloadDocuments.value.findIndex(item => item.filename === file.name);
        const doc: DownloadDocument = {
          id: createClientId(),
          filename: file.name,
          content,
          images: [],
          status: 'ready',
          error: '',
        };
        if (existing >= 0) downloadDocuments.value.splice(existing, 1, doc);
        else downloadDocuments.value.push(doc);
      }
      await analyzeDownloadDocuments();
    } finally {
      loading.value = false;
    }
  }

  function handleDownloadDocumentFiles(event: Event) {
    const input = event.target as HTMLInputElement;
    const files = Array.from(input.files || []);
    input.value = '';
    void addDownloadDocuments(files);
  }

  function handleDownloadDocumentDrop(event: DragEvent) {
    downloadDragActive.value = false;
    const files = Array.from(event.dataTransfer?.files || []);
    void addDownloadDocuments(files);
  }

  function removeDownloadDocument(id: string) {
    downloadDocuments.value = downloadDocuments.value.filter(item => item.id !== id);
    rebuildItems();
  }

  function clearDownloadDocuments() {
    const count = downloadDocuments.value.length;
    if (!count) return;
    downloadDocuments.value = [];
    downloadItems.value = [];
    downloadFilter.value = 'all';
    showMessage(t('msg.download.cleared', { count }));
  }

  async function selectDownloadDir() {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === 'string' && dir) downloadTargetDir.value = dir;
  }

  function syncDownloadTaskProgress(task: ConversionTask, documents?: TaskDocumentProgress[]) {
    updateTaskProgress({
      current: Math.min(task.success + task.partial + task.failed, task.total),
      total: task.total,
      success: task.success,
      partial: task.partial,
      failed: task.failed,
      message: localizeBackendText(task.message) || t('msg.download.taskRunning'),
      documents,
    });
  }

  async function pollDownloadTask(taskId: number, initialDelay = 600) {
    const currentPollVersion = ++downloadPollVersion;
    activeDownloadTaskId = taskId;
    downloadActive.value = true;
    persistDownloadWorkspace(taskId);
    if (initialDelay > 0) await waitForNextPoll(initialDelay);

    while (currentPollVersion === downloadPollVersion) {
      const data = await request<DownloadTaskData>(`/api/convert/tasks/${taskId}`);
      if (currentPollVersion !== downloadPollVersion) return;
      syncDownloadTaskProgress(data.task, data.documents);
      applyTaskImages(data.images);

      if (taskFinished(data.task)) {
        activeDownloadTaskId = 0;
        downloadActive.value = false;
        applyTaskImages(data.images);
        applyTaskDocuments(data.documents);
        if (data.task.status === 'interrupted') {
          activeDownloadTaskId = taskId;
          persistDownloadWorkspace(taskId);
          const message = t('msg.download.progressInterrupted');
          finishTaskProgress({
            status: 'interrupted',
            message,
            detail: t('msg.taskInterruptedHint'),
            current: data.task.success + data.task.partial + data.task.failed,
            resumable: data.resumable === true,
          });
          showError(message);
        } else if (data.task.status === 'cancelled') {
          clearPersistedDownloadWorkspace();
          finishTaskProgress({
            status: 'cancelled',
            message: t('msg.taskCancelled'),
            current: data.task.success + data.task.partial + data.task.failed,
          });
          showMessage(t('msg.taskCancelled'));
        } else {
          clearPersistedDownloadWorkspace();
          const failed = downloadFailedCount.value;
          const message = failed > 0
            ? t('msg.download.donePartial', { success: downloadSuccessCount.value, failed })
            : t('msg.download.done', { success: downloadSuccessCount.value, dir: downloadTargetDir.value.trim() });
          finishTaskProgress({
            status: data.task.failed > 0 ? 'failed' : 'success',
            message: localizeBackendText(data.task.message) || message,
            detail: t('msg.download.taskDoneHint'),
          });
          if (failed > 0 || data.task.failed > 0) showError(message);
          else showMessage(message);
        }
        return;
      }

      await waitForNextPoll(1200);
    }
  }

  async function downloadAllImages() {
    if (!canDownloadBatch.value) return;
    const groups = downloadDocuments.value
      .map(document => ({
        document,
        urls: downloadItems.value.filter(item => item.docId === document.id).map(item => item.url),
      }))
      .filter(group => group.urls.length > 0);
    const totalImages = groups.reduce((sum, group) => sum + group.urls.length, 0);
    const progressDocuments: TaskDocumentProgress[] = groups.map((group, index) => ({
      id: index + 1,
      filename: group.document.filename,
      total_images: group.urls.length,
      processed_images: 0,
      status: 'queued',
      error: '',
    }));
    downloadFilter.value = 'all';
    downloadItems.value.forEach(item => {
      item.status = 'pending';
      item.error = '';
    });
    downloadActive.value = true;
    startTaskProgress({
      taskKind: 'download',
      title: t('msg.download.progressTitle'),
      message: t('msg.download.creating'),
      detail: t('msg.download.progressDetail', { documents: groups.length, images: totalImages }),
      total: groups.length,
    });
    updateTaskProgress({ documents: progressDocuments.map(document => ({ ...document })) });
    try {
      const created = await request<{ task: ConversionTask }>('/api/download/tasks', {
        method: 'POST',
        body: JSON.stringify({
          documents: groups.map(group => ({ filename: group.document.filename, urls: group.urls })),
          target_dir: downloadTargetDir.value.trim(),
        }),
      });
      const taskId = created.task.id;
      updateTaskProgress({ taskId, taskKind: 'download', message: localizeBackendText(created.task.message) || t('msg.download.taskQueued'), current: 0, success: 0, partial: 0, failed: 0 });
      await pollDownloadTask(taskId, 0);
    } catch (error) {
      downloadActive.value = false;
      const message = error instanceof Error ? error.message : t('msg.download.failed');
      finishTaskProgress({ status: 'failed', message: t('msg.download.taskFailed'), detail: message });
      showError(message);
    }
  }

  async function resumeDownloadTask(taskId: number) {
    loading.value = true;
    startTaskProgress({
      taskId,
      taskKind: 'download',
      title: t('msg.download.progressTitle'),
      message: t('msg.taskResuming'),
      total: Math.max(downloadDocuments.value.length, 1),
    });
    try {
      await request(`/api/convert/tasks/${taskId}/resume`, { method: 'POST' });
    } catch (error) {
      persistDownloadWorkspace(taskId);
      finishTaskProgress({
        status: 'interrupted',
        message: t('msg.taskResumeFailed'),
        detail: error instanceof Error ? error.message : t('msg.retryHint'),
        resumable: true,
      });
      showError(error instanceof Error ? error.message : t('msg.taskResumeFailed'));
      loading.value = false;
      return;
    }
    try {
      persistDownloadWorkspace(taskId);
      await pollDownloadTask(taskId, 0);
    } catch (error) {
      persistDownloadWorkspace(taskId);
      downloadActive.value = false;
      finishTaskProgress({
        status: 'failed',
        message: t('msg.download.taskFailed'),
        detail: error instanceof Error ? error.message : t('msg.retryHint'),
      });
      showError(error instanceof Error ? error.message : t('msg.download.taskFailed'));
    } finally {
      loading.value = false;
    }
  }

  async function cancelDownloadTask(taskId: number): Promise<boolean> {
    try {
      await request(`/api/convert/tasks/${taskId}/cancel`, { method: 'POST' });
      updateTaskProgress({ message: t('msg.taskCancelling') });
      return true;
    } catch (error) {
      showError(error instanceof Error ? error.message : t('msg.taskCancelFailed'));
      return false;
    }
  }

  function clearActiveDownloadTaskMarker() {
    activeDownloadTaskId = 0;
    clearPersistedDownloadWorkspace();
  }

  function markCancelledState() {
    downloadItems.value.forEach(item => {
      if (item.status === 'pending' || item.status === 'downloading') item.status = 'cancelled';
    });
  }

  function restoreDownloadWorkspace(): 'download' | null {
    const state = readPersistedDownloadWorkspace();
    if (!state?.activeTaskId) {
      clearPersistedDownloadWorkspace();
      return null;
    }
    downloadDocuments.value = state.documents;
    downloadTargetDir.value = state.targetDir || '';
    rebuildItems();
    startTaskProgress({
      taskId: state.activeTaskId,
      taskKind: 'download',
      title: t('msg.download.progressTitle'),
      message: t('msg.convert.progressRestoring'),
      detail: t('msg.convert.progressRestored', { count: downloadDocuments.value.length }),
      total: Math.max(downloadDocuments.value.length, 1),
    });
    void pollDownloadTask(state.activeTaskId, 0)
      .catch(error => {
        persistDownloadWorkspace(state.activeTaskId);
        downloadActive.value = false;
        finishTaskProgress({
          status: 'failed',
          message: t('msg.download.taskFailed'),
          detail: error instanceof Error ? error.message : t('msg.retryHint'),
        });
        showError(error instanceof Error ? error.message : t('msg.download.taskFailed'));
      })
      .finally(() => {
        loading.value = false;
      });
    return 'download';
  }

  return {
    downloadDocuments,
    downloadDragActive,
    downloadItems,
    downloadTargetDir,
    downloadActive,
    downloadTotalCount,
    downloadSuccessCount,
    downloadFailedCount,
    downloadFilter,
    downloadFilteredItems,
    canDownloadBatch,
    downloadStatusLabel,
    setDownloadFilter,
    handleDownloadDocumentFiles,
    handleDownloadDocumentDrop,
    removeDownloadDocument,
    clearDownloadDocuments,
    selectDownloadDir,
    analyzeDownloadDocuments,
    downloadAllImages,
    restoreDownloadWorkspace,
    resumeDownloadTask,
    cancelDownloadTask,
    clearActiveDownloadTaskMarker,
    markCancelledState,
  };
}
