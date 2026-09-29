import { computed, ref, watch, type Ref } from 'vue';
import { open } from '@tauri-apps/plugin-dialog';
import { createClientId } from './api';
import type { DownloadDocument, DownloadImageItem, DownloadImageResult, MarkdownImage } from './types';

type WorkspaceRequest = <T>(path: string, options?: RequestInit) => Promise<T>;

const DOWNLOAD_DIR_STORAGE_KEY = 'picbed_download_target_dir';

type DownloadWorkspaceDeps = {
  request: WorkspaceRequest;
  showMessage: (text: string) => void;
  showError: (text: string) => void;
  loading: Ref<boolean>;
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

export function useWorkspaceDownload({ request, showMessage, showError, loading }: DownloadWorkspaceDeps) {
  const downloadDocuments = ref<DownloadDocument[]>([]);
  const downloadDragActive = ref(false);
  const downloadItems = ref<DownloadImageItem[]>([]);
  const downloadTargetDir = ref(localStorage.getItem(DOWNLOAD_DIR_STORAGE_KEY) || '');
  const downloadActive = ref(false);

  watch(downloadTargetDir, value => localStorage.setItem(DOWNLOAD_DIR_STORAGE_KEY, value));

  const downloadTotalCount = computed(() => downloadItems.value.length);
  const downloadSuccessCount = computed(
    () => downloadItems.value.filter(item => item.status === 'success').length
  );
  const downloadFailedCount = computed(
    () => downloadItems.value.filter(item => item.status === 'failed').length
  );
  const canDownloadBatch = computed(
    () =>
      !downloadActive.value &&
      downloadItems.value.length > 0 &&
      downloadTargetDir.value.trim().length > 0
  );

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
      showError(error instanceof Error ? error.message : '分析网络图片失败');
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

  async function selectDownloadDir() {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === 'string' && dir) downloadTargetDir.value = dir;
  }

  async function downloadAllImages() {
    if (!canDownloadBatch.value) return;
    const urls = downloadItems.value.map(item => item.url);
    downloadItems.value.forEach(item => {
      item.status = 'downloading';
      item.error = '';
    });
    downloadActive.value = true;
    try {
      const results = await request<DownloadImageResult[]>('/api/download/images', {
        method: 'POST',
        body: JSON.stringify({ urls, target_dir: downloadTargetDir.value.trim() }),
      });
      results.forEach((result, index) => {
        const item = downloadItems.value[index];
        if (!item) return;
        item.status = result.status === 'success' ? 'success' : 'failed';
        item.error = result.error || '';
        item.savedPath = result.saved_path || '';
      });
      const failed = results.filter(result => result.status !== 'success').length;
      const message = failed === 0
        ? `下载完成：成功 ${results.length} 张，已保存到 ${downloadTargetDir.value.trim()}`
        : `下载完成：成功 ${results.length - failed} 张，失败 ${failed} 张`;
      if (failed === 0) showMessage(message);
      else showError(message);
    } catch (error) {
      downloadItems.value.forEach(item => {
        if (item.status === 'downloading') item.status = 'failed';
      });
      showError(error instanceof Error ? error.message : '下载图片失败');
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
    canDownloadBatch,
    handleDownloadDocumentFiles,
    handleDownloadDocumentDrop,
    removeDownloadDocument,
    selectDownloadDir,
    analyzeDownloadDocuments,
    downloadAllImages,
  };
}
