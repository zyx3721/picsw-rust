import { computed, ref, watch, type Ref } from 'vue';
import { open } from '@tauri-apps/plugin-dialog';
import { Channel } from '@tauri-apps/api/core';
import { createClientId } from './api';
import type {
  DownloadDocument,
  DownloadImageFilter,
  DownloadImageItem,
  DownloadImageResult,
  DownloadProgressEvent,
  MarkdownImage,
  TaskDocumentProgress,
} from './types';
import { t } from '../../i18n';

type WorkspaceRequest = <T>(path: string, options?: RequestInit) => Promise<T>;

const DOWNLOAD_DIR_STORAGE_KEY = 'picbed_download_target_dir';

type DownloadWorkspaceDeps = {
  request: WorkspaceRequest;
  showMessage: (text: string) => void;
  showError: (text: string) => void;
  loading: Ref<boolean>;
  startTaskProgress: (input: { title: string; message: string; total: number; detail?: string }) => void;
  updateTaskProgress: (input: {
    message?: string;
    detail?: string;
    current?: number;
    success?: number;
    failed?: number;
    documents?: TaskDocumentProgress[];
  }) => void;
  finishTaskProgress: (input: { status: 'success' | 'failed'; message: string; detail?: string }) => void;
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
  const downloadTargetDir = ref(localStorage.getItem(DOWNLOAD_DIR_STORAGE_KEY) || '');
  const downloadActive = ref(false);
  const downloadFilter = ref<DownloadImageFilter>('all');

  watch(downloadTargetDir, value => localStorage.setItem(DOWNLOAD_DIR_STORAGE_KEY, value));

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
      item.status = 'downloading';
      item.error = '';
    });
    downloadActive.value = true;
    startTaskProgress({
      title: t('msg.download.progressTitle'),
      message: t('msg.download.creating'),
      detail: t('msg.download.progressDetail', { documents: groups.length, images: totalImages }),
      total: groups.length,
    });
    updateTaskProgress({ documents: progressDocuments.map(document => ({ ...document })) });
    try {
      const channel = new Channel<DownloadProgressEvent>();
      channel.onmessage = progress => {
        const document = progressDocuments[progress.document_index];
        if (!document) return;
        document.processed_images = progress.processed;
        document.total_images = progress.total;
        document.status = progress.status;
        document.error = progress.error;
        const success = progressDocuments.filter(item => item.status === 'success').length;
        const failed = progressDocuments.filter(item => item.status === 'failed').length;
        updateTaskProgress({
          message: t('msg.download.taskRunning'),
          current: success + failed,
          success,
          failed,
          documents: progressDocuments.map(item => ({ ...item })),
        });
      };
      const results = await request<DownloadImageResult[]>('/api/download/images', {
        method: 'POST',
        body: JSON.stringify({
          documents: groups.map(group => ({ filename: group.document.filename, urls: group.urls })),
          target_dir: downloadTargetDir.value.trim(),
        }),
        channel,
      } as RequestInit);
      const resultsByUrl = new Map(results.map(result => [result.url, result]));
      downloadItems.value.forEach(item => {
        const result = resultsByUrl.get(item.url);
        if (!result) {
          item.status = 'failed';
          item.error = t('msg.download.failed');
          return;
        }
        item.status = result.status === 'success' ? 'success' : 'failed';
        item.error = result.error || '';
        item.savedPath = result.saved_path || '';
      });
      const failed = results.filter(result => result.status !== 'success').length;
      const message = failed === 0
        ? t('msg.download.done', { success: results.length, dir: downloadTargetDir.value.trim() })
        : t('msg.download.donePartial', { success: results.length - failed, failed });
      finishTaskProgress({
        status: failed === 0 ? 'success' : 'failed',
        message,
        detail: t('msg.download.taskDoneHint'),
      });
      if (failed === 0) showMessage(message);
      else showError(message);
    } catch (error) {
      downloadItems.value.forEach(item => {
        if (item.status === 'downloading') item.status = 'failed';
      });
      const message = error instanceof Error ? error.message : t('msg.download.failed');
      finishTaskProgress({ status: 'failed', message: t('msg.download.taskFailed'), detail: message });
      showError(message);
    } finally {
      downloadActive.value = false;
    }
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
    setDownloadFilter,
    handleDownloadDocumentFiles,
    handleDownloadDocumentDrop,
    removeDownloadDocument,
    clearDownloadDocuments,
    selectDownloadDir,
    analyzeDownloadDocuments,
    downloadAllImages,
  };
}
