import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { createCloseBehavior } from './workspace/closeBehavior';
import { useWorkspaceConfigActions } from './workspace/configActions';
import { useWorkspaceConfigForm } from './workspace/configForm';
import { useWorkspaceCloudSync } from './workspace/cloudSyncWorkspace';
import { useWorkspaceConvert } from './workspace/convertWorkspace';
import { useWorkspaceDownload } from './workspace/downloadWorkspace';
import { useWorkspaceLocalUpload } from './workspace/localUploadWorkspace';
import { useWorkspaceNotices } from './workspace/notices';
import { createWorkspaceRequest } from './workspace/request';
import { useTaskProgress } from './workspace/taskProgress';
import { createUpdateChecker } from './workspace/updateChecker';
import { useWorkspaceData } from './workspace/workspaceData';
import type { ConversionRecord, PicbedConfig, WorkspaceTab } from './workspace/types';
import { t } from '../i18n';

export function usePicbedWorkspace() {
  const activeTab = ref<WorkspaceTab>('convert');
  const { toast, toastKind, toastNoticeKey, showMessage, showError, clearNotice, clearToastTimer } =
    useWorkspaceNotices();
  const loading = ref(false);
  const { taskProgress, startTaskProgress, updateTaskProgress, finishTaskProgress, closeTaskProgress } = useTaskProgress();

  const configs = ref<PicbedConfig[]>([]);
  const {
    typeDefs,
    configForm,
    configErrors,
    secretVisibility,
    supportedTypes,
    selectedFields,
    typeLabel,
    typeDescription,
    fieldLabel,
    fieldPlaceholder,
    secretFieldVisible,
    toggleSecretField,
    handleConfigTypeChange,
    resetConfigForm,
    editConfig,
    validateConfigForm,
    mergeTypeDefs,
  } = useWorkspaceConfigForm({ activeTab, configs, showError, clearNotice });
  const records = ref<ConversionRecord[]>([]);
  const recordDetail = ref<ConversionRecord | null>(null);
  const recordDetailOpen = ref(false);
  const deleteTarget = ref<PicbedConfig | null>(null);
  const configTypeDropdownOpen = ref(false);

  const isAuthed = computed(() => true);
  const successRecords = computed(() => records.value.filter(item => item.status === 'success').length);

  function closeDropdowns() {
    targetDropdownOpen.value = false;
    localTargetDropdownOpen.value = false;
    configTypeDropdownOpen.value = false;
  }
  function handleGlobalPointerDown(event: PointerEvent) {
    const target = event.target;
    if (!(target instanceof Element)) {
      closeDropdowns();
      return;
    }
    if (!target.closest('.custom-select')) closeDropdowns();
  }
  function selectConfigType(value: string) {
    configForm.picbed_type = value;
    configTypeDropdownOpen.value = false;
    handleConfigTypeChange();
  }

  const request = createWorkspaceRequest();
  let reloadRecords: () => Promise<void> = async () => {};
  const {
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
    resetConvertForm,
    handleFiles,
    handleFileDrop,
    addPastedDocument,
    removeBatchFile,
    clearBatchFiles,
    analyzeBatch,
    convertBatch,
    closeGithubProxyDialog,
    confirmGithubProxyConvert,
    downloadFile,
    downloadAll,
    restoreConvertWorkspace,
    resumeConvertTask,
    cancelConvertTask,
    clearActiveConvertTaskMarker,
    stopConvertTaskPolling,
    togglePreview,
    changedLines,
  } = useWorkspaceConvert({
    configs,
    request,
    showMessage,
    showError,
    clearNotice,
    loadRecords: () => reloadRecords(),
    typeLabel,
    loading,
    startTaskProgress,
    updateTaskProgress,
    finishTaskProgress,
  });
  const {
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
  } = useWorkspaceLocalUpload({
    configs,
    request,
    showMessage,
    showError,
    clearNotice,
    loadRecords: () => reloadRecords(),
    typeLabel,
    loading,
    startTaskProgress,
    updateTaskProgress,
    finishTaskProgress,
  });
  const {
    downloadDocuments,
    downloadDragActive,
    downloadItems,
    downloadTargetDir,
    downloadActive,
    downloadTotalCount,
    downloadSuccessCount,
    downloadFailedCount,
    downloadSkippedCount,
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
    markCancelledState: markDownloadCancelledState,
  } = useWorkspaceDownload({
    request,
    showMessage,
    showError,
    loading,
    startTaskProgress,
    updateTaskProgress,
    finishTaskProgress,
  });
  const { loadWorkspaceData, loadConfigs, loadRecords } = useWorkspaceData({
    request,
    typeDefs,
    configs,
    records,
    getTargetConfigId: () => convertForm.target_config_id,
    setTargetConfigId: id => {
      convertForm.target_config_id = id;
    },
    getLocalTargetConfigId: () => localTargetConfigId.value,
    setLocalTargetConfigId: id => {
      localTargetConfigId.value = id;
    },
    getDefaultTarget: () => defaultTarget.value,
    mergeTypeDefs,
  });
  reloadRecords = loadRecords;
  const { saveConfig, requestDeleteConfig, cancelDeleteConfig, confirmDeleteConfig, setDefault, testConfig } = useWorkspaceConfigActions({
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
  });
  const {
    syncAccount,
    secretBackend,
    syncPassword,
    syncPatInput,
    syncRemoteState,
    syncBusy,
    syncConnected,
    syncTokenInvalid,
    syncLastError,
    localConfigCount,
    syncStatusBadge,
    remoteVersionBadge,
    remoteVersionBadgeStale,
    lastSyncText,
    canSyncNow,
    syncNow,
    syncBanner,
    overrideCloudWithLocal,
    pushEmptyVaultLocal,
    resolveConflictWithCloudPassword,
    restoreRemoteVersion,
    forcePushLocal,
    passwordStatus,
    passwordBackend,
    lockSyncPassword,
    passwordModalOpen,
    passwordModalMode,
    openPasswordModal,
    closePasswordModal,
    cloudPwdModalOpen,
    openCloudPwdModal,
    closeCloudPwdModal,
    submitPasswordModal,
    unlockWithPassword,
    githubConnectOpen,
    githubConnectPreparing,
    deviceFlowUserCode,
    deviceFlowStatus,
    deviceFlowError,
    deviceFlowAvailable,
    patMode,
    autoSyncEnabled,
    autoSyncDisplayChecked,
    autoSyncToggleEnabled,
    autoSyncToggleTitle,
    setAutoSyncEnabled,
    openGithubConnect,
    revalidateConnection,
    cancelGithubConnect,
    openVerificationPage,
    copyUserCode,
    connectGithub,
    disconnectGithub,
    syncHistoryOpen,
    syncHistoryLoading,
    syncHistory,
    selectedRevisionSha,
    revisionPreview,
    revisionPreviewLoading,
    revisionError,
    restoringRevision,
    toggleSyncHistory,
    selectRevision,
    restoreRevision,
  } = useWorkspaceCloudSync({
    request,
    configs,
    showMessage,
    showError,
    reloadConfigs: () => loadConfigs(),
  });

  const {
    updateStatus,
    updateVersion,
    updateProgress,
    updateNotes,
    lastCheckAt,
    showUpToDate,
    autoCheckUpdate,
    setAutoCheckUpdate,
    checkUpdate,
    startUpdate,
    applyUpdate,
    openUpdatePackage,
  } = createUpdateChecker({
    request,
    showMessage,
    showError,
  });

  const { closeAction, setCloseAction } = createCloseBehavior();

  function clearWorkspaceDrafts() {
    resetConfigForm();
    resetConvertForm();
    resetLocalUploadForm();
    configTypeDropdownOpen.value = false;
    deleteTarget.value = null;
  }
  function setActiveTab(tab: WorkspaceTab) {
    if (activeTab.value === 'configs' || tab === 'configs') resetConfigForm();
    activeTab.value = tab;
  }
  async function openRecordDetail(record: ConversionRecord) {
    loading.value = true;
    try {
      const data = await request<{ record: ConversionRecord }>(`/api/convert/records/${record.id}`);
      recordDetail.value = data.record;
      recordDetailOpen.value = true;
    } catch (err) {
      showError(err instanceof Error ? err.message : t('msg.records.detailFailed'));
    } finally {
      loading.value = false;
    }
  }
  function closeRecordDetail() {
    recordDetailOpen.value = false;
    recordDetail.value = null;
  }
  async function deleteRecords(ids: number[]) {
    const uniqueIds = Array.from(new Set(ids.filter(id => id > 0)));
    if (uniqueIds.length === 0) return;
    loading.value = true;
    try {
      await request<{ message: string }>('/api/convert/records', {
        method: 'DELETE',
        body: JSON.stringify({ ids: uniqueIds }),
      });
      if (recordDetail.value && uniqueIds.includes(recordDetail.value.id)) closeRecordDetail();
      records.value = records.value.filter(record => !uniqueIds.includes(record.id));
      showMessage(t('msg.records.deleted', { count: uniqueIds.length }));
      await loadRecords();
    } catch (err) {
      showError(err instanceof Error ? err.message : t('msg.records.deleteFailed'));
    } finally {
      loading.value = false;
    }
  }
  async function resumeTaskProgress() {
    const taskId = taskProgress.taskId;
    if (!taskId) return;
    if (taskProgress.taskKind === 'convert') await resumeConvertTask(taskId);
    else if (taskProgress.taskKind === 'localUpload') await resumeLocalUploadTask(taskId);
    else if (taskProgress.taskKind === 'download') await resumeDownloadTask(taskId);
  }
  async function cancelTaskProgress() {
    const taskId = taskProgress.taskId;
    if (!taskId) return;
    const wasInterrupted = taskProgress.status === 'interrupted';
    let cancelled = false;
    if (taskProgress.taskKind === 'convert') cancelled = await cancelConvertTask(taskId);
    else if (taskProgress.taskKind === 'localUpload') cancelled = await cancelLocalUploadTask(taskId);
    else if (taskProgress.taskKind === 'download') cancelled = await cancelDownloadTask(taskId);
    if (!cancelled || !wasInterrupted) return;
    if (taskProgress.taskKind === 'convert') clearActiveConvertTaskMarker();
    else if (taskProgress.taskKind === 'localUpload') clearActiveLocalUploadTaskMarker();
    else clearActiveDownloadTaskMarker();
    if (taskProgress.taskKind === 'download') markDownloadCancelledState();
    updateTaskProgress({
      documents: taskProgress.documents.map(document =>
        document.status === 'success' || document.status === 'partial' || document.status === 'failed'
          ? document
          : { ...document, status: 'cancelled' as const, error: '' }
      ),
    });
    finishTaskProgress({
      status: 'cancelled',
      message: t('msg.taskCancelled'),
      current: taskProgress.success + taskProgress.partial + taskProgress.failed,
    });
    showMessage(t('msg.taskCancelled'));
  }
  onMounted(() => {
    document.addEventListener('pointerdown', handleGlobalPointerDown);
    void loadWorkspaceData()
      .then(() => {
        const restoredConvert = restoreConvertWorkspace();
        const restoredLocal = restoreLocalUploadWorkspace();
        const restoredDownload = restoreDownloadWorkspace();
        const restored = restoredConvert || restoredLocal || restoredDownload;
        if (restored) activeTab.value = restored;
      })
      .catch(err => {
        showError(err instanceof Error ? err.message : t('msg.workspace.initFailed'));
      });
  });
  onBeforeUnmount(() => {
    clearToastTimer();
    stopConvertTaskPolling();
    stopLocalUploadTaskPolling();
    document.removeEventListener('pointerdown', handleGlobalPointerDown);
  });

  return {
    activeTab,
    toast,
    toastKind,
    toastNoticeKey,
    request,
    loading,
    taskProgress,
    closeTaskProgress,
    resumeTaskProgress,
    cancelTaskProgress,
    showMessage,
    showError,
    configForm,
    configErrors,
    convertForm,
    pasteForm,
    configs,
    records,
    recordDetail,
    recordDetailOpen,
    batchFiles,
    deleteTarget,
    targetDropdownOpen,
    configTypeDropdownOpen,
    uploadDragActive,
    githubProxyDialogOpen,
    githubProxyEnabled,
    githubProxyURL,
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
    downloadDocuments,
    downloadDragActive,
    downloadItems,
    downloadTargetDir,
    downloadActive,
    downloadTotalCount,
    downloadSuccessCount,
    downloadFailedCount,
    downloadSkippedCount,
    downloadFilter,
    downloadFilteredItems,
    canDownloadBatch,
    downloadStatusLabel,
    setDownloadFilter,
    syncAccount,
    secretBackend,
    syncPassword,
    syncRemoteState,
    syncBusy,
    syncConnected,
    syncTokenInvalid,
    syncLastError,
    localConfigCount,
    syncStatusBadge,
    remoteVersionBadge,
    remoteVersionBadgeStale,
    lastSyncText,
    canSyncNow,
    syncNow,
    syncBanner,
    overrideCloudWithLocal,
    pushEmptyVaultLocal,
    resolveConflictWithCloudPassword,
    restoreRemoteVersion,
    forcePushLocal,
    syncPatInput,
    passwordStatus,
    passwordBackend,
    lockSyncPassword,
    passwordModalOpen,
    passwordModalMode,
    openPasswordModal,
    closePasswordModal,
    cloudPwdModalOpen,
    openCloudPwdModal,
    closeCloudPwdModal,
    submitPasswordModal,
    unlockWithPassword,
    githubConnectOpen,
    githubConnectPreparing,
    deviceFlowUserCode,
    deviceFlowStatus,
    deviceFlowError,
    deviceFlowAvailable,
    patMode,
    autoSyncEnabled,
    autoSyncDisplayChecked,
    autoSyncToggleEnabled,
    autoSyncToggleTitle,
    setAutoSyncEnabled,
    openGithubConnect,
    revalidateConnection,
    cancelGithubConnect,
    openVerificationPage,
    copyUserCode,
    connectGithub,
    disconnectGithub,
    syncHistoryOpen,
    syncHistoryLoading,
    syncHistory,
    selectedRevisionSha,
    revisionPreview,
    revisionPreviewLoading,
    revisionError,
    restoringRevision,
    toggleSyncHistory,
    selectRevision,
    restoreRevision,
    updateStatus,
    updateVersion,
    updateProgress,
    updateNotes,
    lastCheckAt,
    showUpToDate,
    autoCheckUpdate,
    setAutoCheckUpdate,
    checkUpdate,
    startUpdate,
    applyUpdate,
    openUpdatePackage,
    closeAction,
    setCloseAction,
    secretVisibility,
    isAuthed,
    supportedTypes,
    selectedFields,
    targetConfigs,
    selectedTargetConfig,
    totalImages,
    convertedCount,
    canConvertBatch,
    hasGithubImages,
    localTargetConfigs,
    selectedLocalTargetConfig,
    localMatchedCount,
    localMissingCount,
    localConvertedCount,
    canUploadLocalBatch,
    successRecords,
    typeLabel,
    typeDescription,
    fieldLabel,
    fieldPlaceholder,
    secretFieldVisible,
    toggleSecretField,
    statusLabel,
    targetConfigLabel,
    localTargetConfigLabel,
    selectTargetConfig,
    selectLocalTargetConfig,
    setLocalImageFilter,
    selectConfigType,
    handleConfigTypeChange,
    resetConfigForm,
    resetConvertForm,
    resetLocalUploadForm,
    clearWorkspaceDrafts,
    setActiveTab,
    editConfig,
    saveConfig,
    testConfig,
    requestDeleteConfig,
    cancelDeleteConfig,
    confirmDeleteConfig,
    setDefault,
    handleFiles,
    handleFileDrop,
    addPastedDocument,
    removeBatchFile,
    clearBatchFiles,
    localStatusLabel,
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
    analyzeBatch,
    convertBatch,
    analyzeLocalBatch,
    uploadLocalBatch,
    handleDownloadDocumentFiles,
    handleDownloadDocumentDrop,
    removeDownloadDocument,
    clearDownloadDocuments,
    selectDownloadDir,
    analyzeDownloadDocuments,
    downloadAllImages,
    closeGithubProxyDialog,
    confirmGithubProxyConvert,
    togglePreview,
    changedLines,
    downloadFile,
    downloadAll,
    downloadLocalFile,
    downloadAllLocalFiles,
    loadRecords,
    openRecordDetail,
    closeRecordDetail,
    deleteRecords,
  };
}
