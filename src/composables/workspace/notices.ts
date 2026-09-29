import { ref, watch } from 'vue';

export type WorkspaceToastKind = 'success' | 'error';

export function useWorkspaceNotices() {
  const toast = ref('');
  const toastKind = ref<WorkspaceToastKind>('success');
  const toastNoticeKey = ref(0);
  let toastTimer: ReturnType<typeof window.setTimeout> | undefined;

  function clearToastTimer() {
    if (!toastTimer) return;
    window.clearTimeout(toastTimer);
    toastTimer = undefined;
  }

  function showToast(text: string, kind: WorkspaceToastKind = 'success') {
    clearToastTimer();
    toast.value = text;
    toastKind.value = kind;
    toastNoticeKey.value += 1;
    toastTimer = window.setTimeout(() => {
      toast.value = '';
      toastTimer = undefined;
    }, 5000);
  }

  function showMessage(text: string) {
    showToast(text, 'success');
  }

  function showError(text: string) {
    showToast(text, 'error');
  }

  function clearNotice() {
    clearToastTimer();
    toast.value = '';
  }

  watch(toast, value => {
    if (!value) clearToastTimer();
  });

  return {
    toast,
    toastKind,
    toastNoticeKey,
    showMessage,
    showError,
    showToast,
    clearNotice,
    clearToastTimer,
  };
}
